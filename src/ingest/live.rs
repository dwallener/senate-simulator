use std::{fs, path::Path};

use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};

use crate::{
    error::SenateSimError,
    model::raw_records::{RawActionRecord, RawLegislativeRecord, RawRosterRecord, RawVoteRecord},
};

use super::{
    actions::parse_raw_action_record_value,
    config::IngestionConfig,
    congress_api::CongressApiClient,
    legislation::parse_raw_legislative_record_value,
    roster::parse_raw_roster_record_value,
    senate_votes::{
        SenateVoteClient, parse_vote_index, parse_vote_summary_to_raw, validate_xml_response,
    },
    sources::{fetched_at_for, raw_storage_dir, read_string, write_json_value, write_string},
};

pub struct LiveIngestedRecords {
    pub roster: Vec<RawRosterRecord>,
    pub legislation: Vec<RawLegislativeRecord>,
    pub actions: Vec<RawActionRecord>,
    pub votes: Vec<RawVoteRecord>,
}

pub fn ingest_live_records(config: &IngestionConfig) -> Result<LiveIngestedRecords, SenateSimError> {
    let api_key = config
        .congress_api_key
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| SenateSimError::Validation {
            field: "ingest.congress_api_key",
            message: "live ingestion requires API_KEY_DATA_GOV".to_string(),
        })?;
    let congress = infer_congress_number(config.run_date);
    let session = infer_session_number(config.run_date);
    let raw_dir = raw_storage_dir(&config.output_root, config.run_date);
    fs::create_dir_all(&raw_dir).map_err(|source| SenateSimError::Io {
        path: raw_dir.clone(),
        source,
    })?;

    let congress_client = CongressApiClient::new(api_key)?;
    let senate_vote_client = SenateVoteClient::new()?;

    let members_payload = load_or_fetch_json(
        config,
        &raw_dir.join("congress_members.json"),
        || congress_client.fetch_members(),
    )?;
    let bills_payload = load_or_fetch_json(
        config,
        &raw_dir.join("congress_bills.json"),
        || congress_client.fetch_bills(congress),
    )?;

    let bill_entries = extract_congress_items(&bills_payload, &["bills", "bill"]);
    let mut raw_roster = build_roster_records(config.run_date, &members_payload)?;
    let mut raw_legislation = build_legislative_records(config.run_date, &bill_entries)?;

    let mut aggregated_actions = Vec::new();
    for bill in &bill_entries {
        if let Some((bill_type, bill_number)) = parse_bill_identity(bill) {
            let action_payload = load_or_fetch_json(
                config,
                &raw_dir.join(format!("congress_actions_{bill_type}_{bill_number}.json")),
                || congress_client.fetch_bill_actions(congress, &bill_type, &bill_number),
            )?;
            let action_entries = extract_congress_items(&action_payload, &["actions", "action"]);
            aggregated_actions.extend(action_entries);
        }
    }
    write_json_value(
        &raw_dir.join("congress_actions.json"),
        &Value::Array(aggregated_actions.clone()),
    )?;
    let raw_actions = build_action_records(config.run_date, &aggregated_actions)?;

    let (vote_index_xml, _) = load_or_fetch_text(
        config,
        &raw_dir.join("senate_vote_index.xml"),
        || senate_vote_client.fetch_vote_index(congress, session),
    )?;
    let vote_index = parse_vote_index(&vote_index_xml, congress, session)?;
    let mut raw_votes = Vec::new();
    for reference in vote_index
        .into_iter()
        .filter(|reference| reference.vote_date <= config.run_date)
        .take(25)
    {
        let file_name = format!("senate_vote_{congress}_{session}_{:05}.xml", reference.vote_number);
        let (vote_xml, source_url) = load_or_fetch_text(config, &raw_dir.join(&file_name), || {
            senate_vote_client.fetch_vote_summary(congress, session, reference.vote_number)
        })?;
        raw_votes.extend(parse_vote_summary_to_raw(
            &vote_xml,
            config.run_date,
            &source_url,
            &format!("senate_vote_{congress}_{session}_{:05}", reference.vote_number),
        )?);
    }

    raw_roster.sort_by(|a, b| a.source_member_id.cmp(&b.source_member_id));
    raw_legislation.sort_by(|a, b| a.source_object_id.cmp(&b.source_object_id));
    raw_votes.sort_by(|a, b| {
        a.source_vote_id
            .cmp(&b.source_vote_id)
            .then_with(|| a.senator_id.cmp(&b.senator_id))
    });

    let reconciled_votes = reconcile_vote_member_ids(raw_roster.as_slice(), raw_votes);
    Ok(LiveIngestedRecords {
        roster: raw_roster,
        legislation: raw_legislation,
        actions: raw_actions,
        votes: reconciled_votes,
    })
}

fn load_or_fetch_json<F>(
    config: &IngestionConfig,
    path: &Path,
    fetcher: F,
) -> Result<Value, SenateSimError>
where
    F: FnOnce() -> Result<(Value, super::congress_api::RateLimitStatus, String), SenateSimError>,
{
    if config.use_cached_raw_if_present && path.exists() {
        let text = read_string(path)?;
        return serde_json::from_str(&text).map_err(|source| SenateSimError::Json {
            path: path.to_path_buf(),
            source,
        });
    }

    let (payload, _rate_limit, endpoint_url) = fetcher()?;
    let wrapped = json!({
        "endpoint_url": endpoint_url,
        "fetched_at": fetched_at_for(config.run_date)?,
        "payload": payload,
    });
    write_json_value(path, &wrapped)?;
    Ok(wrapped)
}

fn load_or_fetch_text<F>(
    config: &IngestionConfig,
    path: &Path,
    fetcher: F,
) -> Result<(String, String), SenateSimError>
where
    F: FnOnce() -> Result<(String, String), SenateSimError>,
{
    if config.use_cached_raw_if_present && path.exists() {
        let contents = read_string(path)?;
        validate_xml_response(&path.to_string_lossy(), &contents)?;
        return Ok((contents, path.to_string_lossy().to_string()));
    }

    let (payload, source_url) = fetcher()?;
    validate_xml_response(&source_url, &payload)?;
    write_string(path, &payload)?;
    Ok((payload, source_url))
}

fn build_roster_records(
    run_date: NaiveDate,
    members_payload: &Value,
) -> Result<Vec<RawRosterRecord>, SenateSimError> {
    let fetched_at = fetched_at_for(run_date)?;
    extract_congress_items(members_payload, &["members", "member"])
        .into_iter()
        .filter(|member| senate_term(member).is_some())
        .map(|member| {
            let senate_term = senate_term(&member);
            let entry = json!({
                "source_member_id": member
                    .get("bioguideId")
                    .or_else(|| member.get("memberId"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_member"),
                "name": member
                    .get("directOrderName")
                    .or_else(|| member.get("name"))
                    .or_else(|| member.get("fullName"))
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown Member"),
                "party": member_party(&member),
                "state": member_state(&member),
                "start_date": senate_term
                    .and_then(|term| term.get("startYear"))
                    .and_then(year_to_string_date),
                "end_date": Value::Null,
                "source_name": "congress_api",
                "source_identifier": member
                    .get("bioguideId")
                    .or_else(|| member.get("memberId"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_member"),
                "source_url": member.get("url").cloned().unwrap_or(Value::Null),
            });
            parse_raw_roster_record_value(entry, run_date, fetched_at)
        })
        .collect()
}

fn build_legislative_records(
    run_date: NaiveDate,
    bill_entries: &[Value],
) -> Result<Vec<RawLegislativeRecord>, SenateSimError> {
    let fetched_at = fetched_at_for(run_date)?;
    bill_entries
        .iter()
        .map(|bill| {
            let bill_type = bill
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("bill")
                .to_ascii_lowercase();
            let bill_number = bill
                .get("number")
                .or_else(|| bill.get("billNumber"))
                .and_then(Value::as_str)
                .unwrap_or("0");
            let entry = json!({
                "source_object_id": format!("{bill_type}{bill_number}"),
                "title": bill
                    .get("title")
                    .or_else(|| bill.get("latestTitle"))
                    .and_then(Value::as_str)
                    .unwrap_or("Untitled legislation"),
                "summary": bill
                    .get("summary")
                    .and_then(Value::as_str)
                    .or_else(|| bill.pointer("/summaries/0/text").and_then(Value::as_str)),
                "introduced_date": bill.get("introducedDate").cloned().unwrap_or(Value::Null),
                "sponsor": bill
                    .pointer("/sponsors/0/fullName")
                    .or_else(|| bill.pointer("/sponsors/0/name"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "latest_status_text": bill
                    .get("latestAction")
                    .and_then(|action| action.get("text"))
                    .cloned()
                    .or_else(|| bill.get("updateDateIncludingText").cloned())
                    .unwrap_or(Value::Null),
                "source_name": "congress_api",
                "source_identifier": format!("{bill_type}{bill_number}"),
                "source_url": bill.get("url").cloned().unwrap_or(Value::Null),
            });
            parse_raw_legislative_record_value(entry, run_date, fetched_at)
        })
        .collect()
}

fn build_action_records(
    run_date: NaiveDate,
    actions: &[Value],
) -> Result<Vec<RawActionRecord>, SenateSimError> {
    let fetched_at = fetched_at_for(run_date)?;
    actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let action_date = action
                .get("actionDate")
                .and_then(Value::as_str)
                .unwrap_or("1900-01-01");
            let object_id = action
                .get("actionCode")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("action_{index}"));
            let legislation_id = action
                .get("relatedObjectId")
                .or_else(|| action.get("billId"))
                .and_then(Value::as_str)
                .unwrap_or(&object_id);
            let entry = json!({
                "source_action_id": action.get("actionCode").cloned().unwrap_or_else(|| json!(format!("action_{index}"))),
                "object_id": legislation_id,
                "action_date": action_date,
                "action_text": action.get("text").and_then(Value::as_str).unwrap_or("Congress action"),
                "action_type": action.get("type").cloned().unwrap_or(Value::Null),
                "chamber": action.get("actionCode").and_then(Value::as_str).map(|code| {
                    if code.starts_with('H') { "House" } else { "Senate" }
                }).unwrap_or("Senate"),
                "source_name": "congress_api",
                "source_identifier": action.get("actionCode").cloned().unwrap_or_else(|| json!(format!("action_{index}"))),
                "source_url": action.get("url").cloned().unwrap_or(Value::Null),
            });
            parse_raw_action_record_value(entry, run_date, fetched_at)
        })
        .collect()
}

pub fn reconcile_vote_member_ids(
    roster_records: &[RawRosterRecord],
    vote_records: Vec<RawVoteRecord>,
) -> Vec<RawVoteRecord> {
    vote_records
        .into_iter()
        .map(|mut vote| {
            if let Some(roster) = roster_records.iter().find(|roster| {
                roster.source_member_id.eq_ignore_ascii_case(&vote.senator_id)
                    || (canonical_name_parts(&roster.name) == canonical_name_parts(&vote.senator_name)
                        && roster.state.eq_ignore_ascii_case(
                            vote.raw_payload
                                .get("member")
                                .and_then(|member| member.get("state"))
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                        ))
                    || (roster
                        .name
                        .split(',')
                        .next()
                        .map(str::trim)
                        .unwrap_or("")
                        .eq_ignore_ascii_case(
                            vote.raw_payload
                                .get("member")
                                .and_then(|member| member.get("last_name"))
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                        )
                        && roster.state.eq_ignore_ascii_case(
                            vote.raw_payload
                                .get("member")
                                .and_then(|member| member.get("state"))
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                        ))
            }) {
                vote.senator_id = roster.source_member_id.clone();
            }
            vote
        })
        .collect()
}

pub fn infer_congress_number(run_date: NaiveDate) -> u32 {
    let year = run_date.year();
    let congress_start_year = if year % 2 == 0 { year - 1 } else { year };
    (((congress_start_year - 1789) / 2) + 1) as u32
}

pub fn infer_session_number(run_date: NaiveDate) -> u32 {
    if run_date.year() % 2 == 0 { 2 } else { 1 }
}

fn extract_congress_items(payload: &Value, keys: &[&str]) -> Vec<Value> {
    let mut current = payload.get("payload").unwrap_or(payload);
    for key in keys {
        if let Some(next) = current.get(*key) {
            current = next;
        } else {
            break;
        }
    }
    current
        .as_array()
        .cloned()
        .or_else(|| current.get("item").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

fn parse_bill_identity(bill: &Value) -> Option<(String, String)> {
    Some((
        bill.get("type")?.as_str()?.to_ascii_lowercase(),
        bill.get("number")
            .or_else(|| bill.get("billNumber"))?
            .as_str()?
            .to_string(),
    ))
}

fn member_party(member: &Value) -> String {
    member
        .pointer("/terms/item/0/party")
        .or_else(|| member.pointer("/terms/item/0/partyName"))
        .or_else(|| member.get("partyName"))
        .or_else(|| member.get("party"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_string()
}

fn member_state(member: &Value) -> String {
    let state = member
        .pointer("/terms/item/0/stateCode")
        .or_else(|| member.pointer("/terms/item/0/state"))
        .or_else(|| member.get("state"))
        .or_else(|| member.get("stateCode"))
        .and_then(Value::as_str)
        .unwrap_or("XX")
        .to_string();

    if state.len() == 2 {
        state.to_ascii_uppercase()
    } else {
        state_name_to_code(&state).unwrap_or_else(|| "XX".to_string())
    }
}

fn state_name_to_code(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let code = match normalized.as_str() {
        "alabama" => "AL",
        "alaska" => "AK",
        "arizona" => "AZ",
        "arkansas" => "AR",
        "california" => "CA",
        "colorado" => "CO",
        "connecticut" => "CT",
        "delaware" => "DE",
        "florida" => "FL",
        "georgia" => "GA",
        "hawaii" => "HI",
        "idaho" => "ID",
        "illinois" => "IL",
        "indiana" => "IN",
        "iowa" => "IA",
        "kansas" => "KS",
        "kentucky" => "KY",
        "louisiana" => "LA",
        "maine" => "ME",
        "maryland" => "MD",
        "massachusetts" => "MA",
        "michigan" => "MI",
        "minnesota" => "MN",
        "mississippi" => "MS",
        "missouri" => "MO",
        "montana" => "MT",
        "nebraska" => "NE",
        "nevada" => "NV",
        "new hampshire" => "NH",
        "new jersey" => "NJ",
        "new mexico" => "NM",
        "new york" => "NY",
        "north carolina" => "NC",
        "north dakota" => "ND",
        "ohio" => "OH",
        "oklahoma" => "OK",
        "oregon" => "OR",
        "pennsylvania" => "PA",
        "rhode island" => "RI",
        "south carolina" => "SC",
        "south dakota" => "SD",
        "tennessee" => "TN",
        "texas" => "TX",
        "utah" => "UT",
        "vermont" => "VT",
        "virginia" => "VA",
        "washington" => "WA",
        "west virginia" => "WV",
        "wisconsin" => "WI",
        "wyoming" => "WY",
        "district of columbia" => "DC",
        _ => return None,
    };
    Some(code.to_string())
}

fn senate_term(member: &Value) -> Option<&Value> {
    member
        .get("terms")
        .and_then(|terms| terms.get("item"))
        .and_then(Value::as_array)
        .and_then(|terms| {
            terms.iter().rev().find(|term| {
                term.get("chamber")
                    .and_then(Value::as_str)
                    .map(|value| value.eq_ignore_ascii_case("Senate"))
                    .unwrap_or(false)
            })
        })
}

fn year_to_string_date(value: &Value) -> Option<String> {
    value
        .as_i64()
        .map(|year| format!("{year}-01-03"))
        .or_else(|| value.as_str().map(|year| format!("{year}-01-03")))
}

fn canonical_name_parts(value: &str) -> Vec<String> {
    let mut parts = value
        .replace(',', " ")
        .to_ascii_lowercase()
        .split_whitespace()
        .map(|part| {
            part.chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts.sort();
    parts
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use crate::model::{identity::Party, legislative_context::Chamber, raw_records::RawVoteRecord};

    use super::{
        build_action_records, build_legislative_records, build_roster_records, extract_congress_items,
        infer_congress_number, infer_session_number, reconcile_vote_member_ids,
    };

    #[test]
    fn builds_congress_records_from_saved_payloads() {
        let run_date = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        let members = json!({
            "payload": {
                "members": [
                    {
                        "bioguideId": "A0001",
                        "directOrderName": "Adams, Alex",
                        "partyName": "Democratic",
                        "state": "Washington",
                        "terms": { "item": [{ "chamber": "Senate", "startYear": 2025 }] }
                    }
                ]
            }
        });
        let bills = json!({
            "payload": {
                "bills": [
                    {
                        "type": "s",
                        "number": "2100",
                        "title": "Clean Energy Permitting Reform Act",
                        "summary": "Permitting reform for transmission.",
                        "introducedDate": "2026-03-01",
                        "latestAction": { "text": "Cloture filed in Senate." },
                        "url": "https://example.test/bill"
                    }
                ]
            }
        });
        let actions = vec![json!({
            "actionCode": "S001",
            "billId": "s2100",
            "actionDate": "2026-03-05",
            "text": "Cloture filed in Senate.",
            "type": "floor"
        })];

        let roster = build_roster_records(run_date, &members).unwrap();
        let legislation = build_legislative_records(run_date, &extract_congress_items(&bills, &["bills"])).unwrap();
        let action_records = build_action_records(run_date, &actions).unwrap();
        assert_eq!(roster.len(), 1);
        assert_eq!(roster[0].party, "Democratic");
        assert_eq!(roster[0].state, "WA");
        assert_eq!(roster[0].start_date, Some(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap()));
        assert_eq!(legislation[0].source_object_id, "s2100");
        assert_eq!(action_records[0].chamber, Chamber::Senate);
    }

    #[test]
    fn reconciles_member_ids_between_roster_and_votes() {
        let roster = vec![crate::model::raw_records::RawRosterRecord {
            source_member_id: "A0001".to_string(),
            name: "Adams, Alex".to_string(),
            party: "D".to_string(),
            state: "WA".to_string(),
            chamber: Chamber::Senate,
            start_date: None,
            end_date: None,
            as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            source_name: "congress_api".to_string(),
            source_identifier: "A0001".to_string(),
            source_url: None,
            fetched_at: chrono::Utc::now(),
            raw_payload: json!({}),
        }];
        let votes = vec![RawVoteRecord {
            source_vote_id: "vote_1".to_string(),
            vote_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            senator_id: "alex_adams_wa_d".to_string(),
            senator_name: "Alex Adams".to_string(),
            object_id: Some("s2100".to_string()),
            vote_category: "Cloture".to_string(),
            vote_position: "Yea".to_string(),
            party_at_time: Party::Democrat,
            policy_domain: None,
            is_procedural: true,
            procedural_kind: Some("Cloture".to_string()),
            as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            source_name: "senate_votes".to_string(),
            source_identifier: "vote_1".to_string(),
            source_url: None,
            fetched_at: chrono::Utc::now(),
            raw_payload: json!({"member": {"state": "WA"}}),
        }];

        let reconciled = reconcile_vote_member_ids(&roster, votes);
        assert_eq!(reconciled[0].senator_id, "A0001");
    }

    #[test]
    fn infers_congress_and_session_from_date() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        assert_eq!(infer_congress_number(date), 119);
        assert_eq!(infer_session_number(date), 2);
    }
}
