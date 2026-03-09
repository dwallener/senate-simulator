use std::{thread, time::Duration};

use chrono::NaiveDate;
use quick_xml::de::from_str;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    error::SenateSimError,
    model::{
        identity::Party,
        legislative::PolicyDomain,
        raw_records::RawVoteRecord,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenateVoteReference {
    pub congress: u32,
    pub session: u32,
    pub vote_number: u32,
    pub vote_date: NaiveDate,
}

pub struct SenateVoteClient {
    client: Client,
    pacing_delay: Duration,
}

impl SenateVoteClient {
    pub fn new() -> Result<Self, SenateSimError> {
        let client = Client::builder().build().map_err(SenateSimError::HttpClient)?;
        Ok(Self {
            client,
            pacing_delay: Duration::from_millis(150),
        })
    }

    pub fn fetch_vote_index(
        &self,
        congress: u32,
        session: u32,
    ) -> Result<(String, String), SenateSimError> {
        thread::sleep(self.pacing_delay);
        let url = format!(
            "https://www.senate.gov/legislative/LIS/roll_call_lists/vote_menu_{congress}_{session}.xml"
        );
        let response = self
            .client
            .get(&url)
            .send()
            .map_err(SenateSimError::HttpClient)?;
        if !response.status().is_success() {
            return Err(SenateSimError::HttpStatus {
                url,
                status: response.status(),
            });
        }
        let body = response.text().map_err(SenateSimError::HttpClient)?;
        ensure_xml_response(&url, &body)?;
        Ok((body, url))
    }

    pub fn fetch_vote_summary(
        &self,
        congress: u32,
        session: u32,
        vote_number: u32,
    ) -> Result<(String, String), SenateSimError> {
        thread::sleep(self.pacing_delay);
        let url = format!(
            "https://www.senate.gov/legislative/LIS/roll_call_votes/vote{congress}{session}/vote_{congress}_{session}_{vote_number:05}.xml"
        );
        let response = self
            .client
            .get(&url)
            .send()
            .map_err(SenateSimError::HttpClient)?;
        if !response.status().is_success() {
            return Err(SenateSimError::HttpStatus {
                url,
                status: response.status(),
            });
        }
        let body = response.text().map_err(SenateSimError::HttpClient)?;
        ensure_xml_response(&url, &body)?;
        Ok((body, url))
    }
}

pub fn parse_vote_index(
    xml: &str,
    congress: u32,
    session: u32,
) -> Result<Vec<SenateVoteReference>, SenateSimError> {
    let payload: VoteMenuXml = from_str(xml).map_err(SenateSimError::Xml)?;
    let congress_year = payload.congress_year.unwrap_or_default();
    let mut entries = Vec::new();
    for vote in payload.votes.items {
        let vote_number = vote
            .vote_number
            .trim()
            .parse::<u32>()
            .map_err(|_| SenateSimError::Validation {
                field: "senate_vote_index.vote_number",
                message: format!("invalid vote number {}", vote.vote_number),
            })?;
        let vote_date = parse_vote_index_date(&vote.vote_date, congress_year)?;
        entries.push(SenateVoteReference {
            congress,
            session,
            vote_number,
            vote_date,
        });
    }
    Ok(entries)
}

fn parse_vote_index_date(value: &str, congress_year: i32) -> Result<NaiveDate, SenateSimError> {
    let trimmed = value.trim();
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%d-%b-%Y") {
        return Ok(date);
    }
    if let Ok(partial) = NaiveDate::parse_from_str(&format!("{trimmed}-{congress_year}"), "%d-%b-%Y") {
        return Ok(partial);
    }
    parse_vote_date(trimmed)
}

pub fn parse_vote_summary_to_raw(
    xml: &str,
    run_date: NaiveDate,
    source_url: &str,
    source_identifier: &str,
) -> Result<Vec<RawVoteRecord>, SenateSimError> {
    let payload: VoteSummaryXml = from_str(xml).map_err(SenateSimError::Xml)?;
    let vote_date = parse_vote_date(payload.vote_date.as_deref().unwrap_or(""))?;
    let title = payload
        .vote_title
        .clone()
        .or(payload.vote_question.clone())
        .unwrap_or_else(|| "Senate vote".to_string());
    let document = payload.document.unwrap_or_default();
    let object_id = build_object_id(document.document_type.as_deref(), document.document_number.as_deref());
    let vote_category = infer_vote_category(&title);
    let procedural_kind = infer_procedural_kind(&title);
    let is_procedural = procedural_kind.is_some()
        || matches!(
            vote_category.as_str(),
            "Cloture" | "MotionToProceed" | "Procedural"
        );
    let policy_domain = infer_policy_domain_from_text(&title);
    let fetched_at = chrono::Utc::now();

    let mut records = Vec::new();
    for member in payload.members.members {
        let senator_id = member
            .lis_member_id
            .clone()
            .or(member.member_id.clone())
            .unwrap_or_else(|| synthesize_member_id(&member));
        let senator_name = build_member_name(&member);
        records.push(RawVoteRecord {
            source_vote_id: source_identifier.to_string(),
            vote_date,
            senator_id,
            senator_name,
            object_id: object_id.clone(),
            vote_category: vote_category.clone(),
            vote_position: member
                .vote_cast
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
            party_at_time: parse_party(member.party.as_deref().unwrap_or("Unknown")),
            policy_domain: policy_domain.clone(),
            is_procedural,
            procedural_kind: procedural_kind.clone(),
            as_of_date: run_date,
            source_name: "senate_votes".to_string(),
            source_identifier: source_identifier.to_string(),
            source_url: Some(source_url.to_string()),
            fetched_at,
            raw_payload: json!({
                "vote_title": title,
                "document_type": document.document_type,
                "document_number": document.document_number,
                "member": member,
            }),
        });
    }
    Ok(records)
}

fn parse_vote_date(value: &str) -> Result<NaiveDate, SenateSimError> {
    for format in [
        "%B %d, %Y",
        "%B %e, %Y",
        "%B %d, %Y, %I:%M %p",
        "%B %e, %Y, %I:%M %p",
        "%Y-%m-%d",
        "%d-%b-%Y",
        "%d-%b",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(value.trim(), format) {
            return Ok(date);
        }
    }

    let date_prefix = value.split(',').take(2).collect::<Vec<_>>().join(",");
    for format in ["%B %d,%Y", "%B %e,%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(date_prefix.trim(), format) {
            return Ok(date);
        }
    }

    Err(SenateSimError::Validation {
        field: "senate_vote_summary.vote_date",
        message: format!("invalid vote date {value}"),
    })
}

fn ensure_xml_response(url: &str, body: &str) -> Result<(), SenateSimError> {
    let trimmed = body.trim_start();
    if trimmed.starts_with("<?xml") || trimmed.starts_with('<') && !trimmed.starts_with("<!DOCTYPE html") && !trimmed.starts_with("<html") {
        return Ok(());
    }
    Err(SenateSimError::UnexpectedResponseFormat {
        url: url.to_string(),
        expected: "XML",
        body_prefix: trimmed.chars().take(80).collect(),
    })
}

pub fn validate_xml_response(url: &str, body: &str) -> Result<(), SenateSimError> {
    ensure_xml_response(url, body)
}

fn build_object_id(document_type: Option<&str>, document_number: Option<&str>) -> Option<String> {
    let doc_type = document_type?.trim().to_ascii_lowercase();
    let doc_number = document_number?.trim().to_ascii_lowercase();
    if doc_type.is_empty() || doc_number.is_empty() {
        return None;
    }
    Some(format!("{doc_type}{doc_number}"))
}

fn infer_vote_category(title: &str) -> String {
    let lowered = title.to_ascii_lowercase();
    if lowered.contains("cloture") {
        "Cloture".to_string()
    } else if lowered.contains("motion to proceed") {
        "MotionToProceed".to_string()
    } else if lowered.contains("amendment") {
        "Amendment".to_string()
    } else if lowered.contains("nomination") {
        "Nomination".to_string()
    } else if lowered.contains("passage")
        || lowered.contains("on passage")
        || lowered.contains("passed")
    {
        "Passage".to_string()
    } else if lowered.contains("motion") || lowered.contains("table") {
        "Procedural".to_string()
    } else {
        "Other".to_string()
    }
}

fn infer_procedural_kind(title: &str) -> Option<String> {
    let lowered = title.to_ascii_lowercase();
    if lowered.contains("cloture") {
        Some("Cloture".to_string())
    } else if lowered.contains("motion to proceed") {
        Some("MotionToProceed".to_string())
    } else if lowered.contains("amendment") {
        Some("AmendmentProcess".to_string())
    } else if lowered.contains("table") {
        Some("Table".to_string())
    } else if lowered.contains("recommit") {
        Some("Recommit".to_string())
    } else {
        None
    }
}

fn infer_policy_domain_from_text(title: &str) -> Option<PolicyDomain> {
    let lowered = title.to_ascii_lowercase();
    if lowered.contains("defense") {
        Some(PolicyDomain::Defense)
    } else if lowered.contains("budget") || lowered.contains("tax") {
        Some(PolicyDomain::BudgetTax)
    } else if lowered.contains("health") {
        Some(PolicyDomain::Healthcare)
    } else if lowered.contains("immigration") || lowered.contains("border") {
        Some(PolicyDomain::Immigration)
    } else if lowered.contains("energy") || lowered.contains("climate") {
        Some(PolicyDomain::EnergyClimate)
    } else if lowered.contains("judiciary") || lowered.contains("judge") {
        Some(PolicyDomain::Judiciary)
    } else if lowered.contains("technology") || lowered.contains("privacy") {
        Some(PolicyDomain::Technology)
    } else if lowered.contains("foreign") {
        Some(PolicyDomain::ForeignPolicy)
    } else if lowered.contains("labor") || lowered.contains("worker") {
        Some(PolicyDomain::Labor)
    } else if lowered.contains("education") || lowered.contains("school") {
        Some(PolicyDomain::Education)
    } else {
        None
    }
}

fn parse_party(value: &str) -> Party {
    if value.eq_ignore_ascii_case("D") || value.eq_ignore_ascii_case("Democrat") {
        Party::Democrat
    } else if value.eq_ignore_ascii_case("R") || value.eq_ignore_ascii_case("Republican") {
        Party::Republican
    } else if value.eq_ignore_ascii_case("I") || value.eq_ignore_ascii_case("Independent") {
        Party::Independent
    } else {
        Party::Other(value.to_string())
    }
}

fn synthesize_member_id(member: &VoteMemberXml) -> String {
    format!(
        "{}_{}_{}",
        member
            .last_name
            .as_deref()
            .unwrap_or("member")
            .to_ascii_lowercase(),
        member.state.as_deref().unwrap_or("xx").to_ascii_lowercase(),
        member.party.as_deref().unwrap_or("u").to_ascii_lowercase()
    )
}

fn build_member_name(member: &VoteMemberXml) -> String {
    match (member.first_name.as_deref(), member.last_name.as_deref()) {
        (Some(first), Some(last)) => format!("{first} {last}"),
        (_, Some(last)) => last.to_string(),
        _ => member
            .member_full
            .clone()
            .unwrap_or_else(|| "Unknown Senator".to_string()),
    }
}

#[derive(Debug, Deserialize)]
struct VoteMenuXml {
    congress_year: Option<i32>,
    #[serde(default)]
    votes: VoteMenuEntriesXml,
}

#[derive(Debug, Deserialize, Default)]
struct VoteMenuEntriesXml {
    #[serde(rename = "vote", default)]
    items: Vec<VoteMenuEntryXml>,
}

#[derive(Debug, Deserialize)]
struct VoteMenuEntryXml {
    vote_number: String,
    vote_date: String,
}

#[derive(Debug, Deserialize, Default)]
struct VoteSummaryXml {
    vote_date: Option<String>,
    vote_question: Option<String>,
    vote_title: Option<String>,
    #[serde(default)]
    document: Option<VoteDocumentXml>,
    members: VoteMembersXml,
}

#[derive(Debug, Deserialize, Default)]
struct VoteDocumentXml {
    document_type: Option<String>,
    document_number: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct VoteMembersXml {
    #[serde(rename = "member", default)]
    members: Vec<VoteMemberXml>,
}

#[derive(Debug, Deserialize, Default, Serialize)]
struct VoteMemberXml {
    lis_member_id: Option<String>,
    member_id: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    member_full: Option<String>,
    state: Option<String>,
    party: Option<String>,
    vote_cast: Option<String>,
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{parse_vote_index, parse_vote_summary_to_raw};

    #[test]
    fn parses_vote_index_fixture() {
        let xml = r#"
            <vote_summary>
              <congress_year>2026</congress_year>
              <votes>
                <vote>
                  <vote_number>1</vote_number>
                  <vote_date>15-Jan</vote_date>
                </vote>
                <vote>
                  <vote_number>2</vote_number>
                  <vote_date>20-Jan</vote_date>
                </vote>
              </votes>
            </vote_summary>
        "#;
        let entries = parse_vote_index(xml, 119, 1).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].vote_number, 1);
        assert_eq!(entries[1].vote_date, NaiveDate::from_ymd_opt(2026, 1, 20).unwrap());
    }

    #[test]
    fn parses_vote_summary_into_raw_records() {
        let xml = r#"
            <roll_call_vote>
              <vote_date>January 15, 2026</vote_date>
              <vote_question>On the Cloture Motion S.2100</vote_question>
              <vote_title>On the Cloture Motion S.2100</vote_title>
              <document>
                <document_type>s</document_type>
                <document_number>2100</document_number>
              </document>
              <members>
                <member>
                  <lis_member_id>A0001</lis_member_id>
                  <first_name>Alex</first_name>
                  <last_name>Adams</last_name>
                  <state>WA</state>
                  <party>D</party>
                  <vote_cast>Yea</vote_cast>
                </member>
                <member>
                  <lis_member_id>B0002</lis_member_id>
                  <first_name>Blair</first_name>
                  <last_name>Baker</last_name>
                  <state>TX</state>
                  <party>R</party>
                  <vote_cast>Nay</vote_cast>
                </member>
              </members>
            </roll_call_vote>
        "#;
        let records = parse_vote_summary_to_raw(
            xml,
            NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            "https://example.test/vote.xml",
            "senate_vote_119_1_00001",
        )
        .unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].object_id.as_deref(), Some("s2100"));
        assert_eq!(records[0].vote_category, "Cloture");
        assert_eq!(records[0].procedural_kind.as_deref(), Some("Cloture"));
    }
}
