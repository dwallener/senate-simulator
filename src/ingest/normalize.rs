use chrono::{Datelike, NaiveDate};

use crate::{
    error::SenateSimError,
    model::{
        identity::{Party, SenateClass},
        legislative::{BudgetaryImpact, LegislativeObjectType, PolicyDomain},
        legislative_context::{Chamber, ProceduralStage},
        normalized_records::{
            NormalizedActionCategory, NormalizedActionRecord, NormalizedLegislativeRecord,
            NormalizedSenatorRecord, NormalizedVoteRecord, ProceduralKind, VoteCategory,
            VotePosition,
        },
        raw_records::{RawActionRecord, RawLegislativeRecord, RawRosterRecord, RawVoteRecord},
    },
};

pub fn normalize_roster(
    raw_records: &[RawRosterRecord],
) -> Result<Vec<NormalizedSenatorRecord>, SenateSimError> {
    raw_records
        .iter()
        .enumerate()
        .map(|(index, record)| normalize_roster_record(record, index))
        .collect()
}

pub fn normalize_legislation(
    raw_records: &[RawLegislativeRecord],
) -> Result<Vec<NormalizedLegislativeRecord>, SenateSimError> {
    raw_records
        .iter()
        .map(normalize_legislative_record)
        .collect()
}

pub fn normalize_actions(
    raw_records: &[RawActionRecord],
) -> Result<Vec<NormalizedActionRecord>, SenateSimError> {
    raw_records.iter().map(normalize_action_record).collect()
}

pub fn normalize_votes(
    raw_records: &[RawVoteRecord],
) -> Result<Vec<NormalizedVoteRecord>, SenateSimError> {
    raw_records.iter().map(normalize_vote_record).collect()
}

fn normalize_roster_record(
    record: &RawRosterRecord,
    index: usize,
) -> Result<NormalizedSenatorRecord, SenateSimError> {
    let start_date = record.start_date.unwrap_or(record.as_of_date);
    let class = match index % 3 {
        0 => SenateClass::I,
        1 => SenateClass::II,
        _ => SenateClass::III,
    };
    let normalized = NormalizedSenatorRecord {
        senator_id: format!("real_{}", sanitize_identifier(&record.source_member_id)),
        full_name: record.name.trim().to_string(),
        party: parse_party(&record.party),
        state: record.state.trim().to_uppercase(),
        class,
        start_date,
        end_date: record.end_date,
        source_member_id: record.source_member_id.clone(),
        as_of_date: record.as_of_date,
    };
    normalized.validate()?;
    Ok(normalized)
}

fn normalize_legislative_record(
    record: &RawLegislativeRecord,
) -> Result<NormalizedLegislativeRecord, SenateSimError> {
    let summary = record
        .summary
        .clone()
        .unwrap_or_else(|| "Summary unavailable in source payload".to_string());
    let normalized = NormalizedLegislativeRecord {
        object_id: sanitize_identifier(&record.source_object_id),
        title: record.title.trim().to_string(),
        summary,
        object_type: infer_object_type(
            &record.source_object_id,
            record.latest_status_text.as_deref(),
        ),
        policy_domain: infer_policy_domain(&record.title, record.summary.as_deref()),
        sponsor: record.sponsor.clone(),
        introduced_date: record.introduced_date.unwrap_or(record.as_of_date),
        latest_status_text: record.latest_status_text.clone(),
        current_stage: infer_stage(record.latest_status_text.as_deref()),
        origin_chamber: Chamber::Senate,
        budgetary_impact: infer_budgetary_impact(&record.title, record.summary.as_deref()),
        salience: infer_salience(&record.title, record.summary.as_deref()),
        controversy: infer_controversy(&record.title, record.summary.as_deref()),
        as_of_date: record.as_of_date,
    };
    normalized.validate()?;
    Ok(normalized)
}

fn normalize_action_record(
    record: &RawActionRecord,
) -> Result<NormalizedActionRecord, SenateSimError> {
    let normalized = NormalizedActionRecord {
        action_id: sanitize_identifier(&record.source_action_id),
        object_id: sanitize_identifier(&record.object_id),
        action_date: record.action_date,
        chamber: record.chamber,
        action_text: record.action_text.clone(),
        category: infer_action_category(record.action_type.as_deref(), &record.action_text),
        as_of_date: record.as_of_date,
    };
    normalized.validate()?;
    Ok(normalized)
}

fn normalize_vote_record(record: &RawVoteRecord) -> Result<NormalizedVoteRecord, SenateSimError> {
    let normalized = NormalizedVoteRecord {
        vote_id: sanitize_identifier(&record.source_vote_id),
        vote_date: record.vote_date,
        senator_id: format!("real_{}", sanitize_identifier(&record.senator_id)),
        senator_name: record.senator_name.clone(),
        object_id: record.object_id.as_ref().map(|value| sanitize_identifier(value)),
        vote_category: parse_vote_category(&record.vote_category),
        vote_position: parse_vote_position(&record.vote_position),
        party_at_time: record.party_at_time.clone(),
        policy_domain: record.policy_domain.clone(),
        is_procedural: record.is_procedural,
        procedural_kind: record
            .procedural_kind
            .as_deref()
            .map(parse_procedural_kind),
        as_of_date: record.as_of_date,
    };
    normalized.validate()?;
    Ok(normalized)
}

fn parse_party(value: &str) -> Party {
    if value.eq_ignore_ascii_case("D")
        || value.eq_ignore_ascii_case("Democrat")
        || value.eq_ignore_ascii_case("Democratic")
    {
        Party::Democrat
    } else if value.eq_ignore_ascii_case("R") || value.eq_ignore_ascii_case("Republican") {
        Party::Republican
    } else if value.eq_ignore_ascii_case("I") || value.eq_ignore_ascii_case("Independent") {
        Party::Independent
    } else {
        Party::Other(value.trim().to_string())
    }
}

fn sanitize_identifier(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn infer_object_type(id: &str, status_text: Option<&str>) -> LegislativeObjectType {
    let lowered_id = id.to_ascii_lowercase();
    let status = status_text.unwrap_or_default().to_ascii_lowercase();
    if lowered_id.starts_with('s') || lowered_id.starts_with("hr") || status.contains("bill") {
        LegislativeObjectType::Bill
    } else if status.contains("amendment") {
        LegislativeObjectType::Amendment
    } else if status.contains("cloture") {
        LegislativeObjectType::ClotureVote
    } else {
        LegislativeObjectType::Other("unknown".to_string())
    }
}

fn infer_policy_domain(title: &str, summary: Option<&str>) -> PolicyDomain {
    let haystack = format!(
        "{} {}",
        title.to_ascii_lowercase(),
        summary.unwrap_or_default().to_ascii_lowercase()
    );
    if haystack.contains("energy")
        || haystack.contains("climate")
        || haystack.contains("permitting")
    {
        PolicyDomain::EnergyClimate
    } else if haystack.contains("health") {
        PolicyDomain::Healthcare
    } else if haystack.contains("border") || haystack.contains("immigration") {
        PolicyDomain::Immigration
    } else if haystack.contains("defense") || haystack.contains("armed forces") {
        PolicyDomain::Defense
    } else if haystack.contains("privacy")
        || haystack.contains("technology")
        || haystack.contains("ai")
    {
        PolicyDomain::Technology
    } else if haystack.contains("tax")
        || haystack.contains("budget")
        || haystack.contains("appropriation")
    {
        PolicyDomain::BudgetTax
    } else if haystack.contains("labor")
        || haystack.contains("union")
        || haystack.contains("worker")
    {
        PolicyDomain::Labor
    } else if haystack.contains("court")
        || haystack.contains("judge")
        || haystack.contains("judiciary")
    {
        PolicyDomain::Judiciary
    } else {
        PolicyDomain::Other("general".to_string())
    }
}

fn infer_stage(status_text: Option<&str>) -> ProceduralStage {
    let text = status_text.unwrap_or_default().to_ascii_lowercase();
    if text.contains("cloture vote") {
        ProceduralStage::ClotureVote
    } else if text.contains("cloture filed") {
        ProceduralStage::ClotureFiled
    } else if text.contains("amendment") {
        ProceduralStage::AmendmentPending
    } else if text.contains("debate") {
        ProceduralStage::Debate
    } else if text.contains("motion to proceed") {
        ProceduralStage::MotionToProceed
    } else if text.contains("calendar") {
        ProceduralStage::OnCalendar
    } else if text.contains("reported") {
        ProceduralStage::Reported
    } else if text.contains("committee") || text.contains("referred") {
        ProceduralStage::InCommittee
    } else if text.contains("stalled") {
        ProceduralStage::Stalled
    } else {
        ProceduralStage::Introduced
    }
}

fn infer_budgetary_impact(title: &str, summary: Option<&str>) -> BudgetaryImpact {
    let haystack = format!(
        "{} {}",
        title.to_ascii_lowercase(),
        summary.unwrap_or_default().to_ascii_lowercase()
    );
    if haystack.contains("billion")
        || haystack.contains("appropriation")
        || haystack.contains("funding")
    {
        BudgetaryImpact::High
    } else if haystack.contains("grant") || haystack.contains("program") {
        BudgetaryImpact::Moderate
    } else {
        BudgetaryImpact::Unknown
    }
}

fn infer_salience(title: &str, summary: Option<&str>) -> f32 {
    let haystack = format!(
        "{} {}",
        title.to_ascii_lowercase(),
        summary.unwrap_or_default().to_ascii_lowercase()
    );
    let mut score: f32 = 0.45;
    for keyword in [
        "security", "energy", "health", "climate", "budget", "border",
    ] {
        if haystack.contains(keyword) {
            score += 0.08;
        }
    }
    score.clamp(0.0, 1.0)
}

fn infer_controversy(title: &str, summary: Option<&str>) -> f32 {
    let haystack = format!(
        "{} {}",
        title.to_ascii_lowercase(),
        summary.unwrap_or_default().to_ascii_lowercase()
    );
    let mut score: f32 = 0.30;
    for keyword in [
        "controversial",
        "border",
        "filibuster",
        "climate",
        "privacy",
    ] {
        if haystack.contains(keyword) {
            score += 0.10;
        }
    }
    score.clamp(0.0, 1.0)
}

fn infer_action_category(action_type: Option<&str>, action_text: &str) -> NormalizedActionCategory {
    let haystack = format!(
        "{} {}",
        action_type.unwrap_or_default().to_ascii_lowercase(),
        action_text.to_ascii_lowercase()
    );
    if haystack.contains("introduced") {
        NormalizedActionCategory::Introduced
    } else if haystack.contains("referred") || haystack.contains("committee") {
        NormalizedActionCategory::Referred
    } else if haystack.contains("reported") {
        NormalizedActionCategory::Reported
    } else if haystack.contains("motion to proceed") {
        NormalizedActionCategory::MotionToProceed
    } else if haystack.contains("debate") {
        NormalizedActionCategory::Debate
    } else if haystack.contains("amendment") {
        NormalizedActionCategory::Amendment
    } else if haystack.contains("cloture") {
        NormalizedActionCategory::Cloture
    } else if haystack.contains("pass") || haystack.contains("agreed to") {
        NormalizedActionCategory::Passage
    } else if haystack.contains("stalled") || haystack.contains("block") {
        NormalizedActionCategory::Stall
    } else {
        NormalizedActionCategory::Other
    }
}

fn parse_vote_position(value: &str) -> VotePosition {
    if value.eq_ignore_ascii_case("yea") || value.eq_ignore_ascii_case("yes") {
        VotePosition::Yea
    } else if value.eq_ignore_ascii_case("nay") || value.eq_ignore_ascii_case("no") {
        VotePosition::Nay
    } else if value.eq_ignore_ascii_case("present") {
        VotePosition::Present
    } else if value.eq_ignore_ascii_case("notvoting")
        || value.eq_ignore_ascii_case("not_voting")
        || value.eq_ignore_ascii_case("absent")
    {
        VotePosition::NotVoting
    } else {
        VotePosition::Unknown
    }
}

fn parse_vote_category(value: &str) -> VoteCategory {
    if value.eq_ignore_ascii_case("passage") {
        VoteCategory::Passage
    } else if value.eq_ignore_ascii_case("cloture") {
        VoteCategory::Cloture
    } else if value.eq_ignore_ascii_case("motiontoproceed")
        || value.eq_ignore_ascii_case("motion_to_proceed")
    {
        VoteCategory::MotionToProceed
    } else if value.eq_ignore_ascii_case("amendment") {
        VoteCategory::Amendment
    } else if value.eq_ignore_ascii_case("nomination") {
        VoteCategory::Nomination
    } else if value.eq_ignore_ascii_case("procedural") {
        VoteCategory::Procedural
    } else {
        VoteCategory::Other
    }
}

fn parse_procedural_kind(value: &str) -> ProceduralKind {
    if value.eq_ignore_ascii_case("cloture") {
        ProceduralKind::Cloture
    } else if value.eq_ignore_ascii_case("motiontoproceed")
        || value.eq_ignore_ascii_case("motion_to_proceed")
    {
        ProceduralKind::MotionToProceed
    } else if value.eq_ignore_ascii_case("amendmentprocess")
        || value.eq_ignore_ascii_case("amendment_process")
    {
        ProceduralKind::AmendmentProcess
    } else if value.eq_ignore_ascii_case("table") {
        ProceduralKind::Table
    } else if value.eq_ignore_ascii_case("recommit") {
        ProceduralKind::Recommit
    } else {
        ProceduralKind::Other
    }
}

pub(crate) fn default_context_for_stage(
    snapshot_date: NaiveDate,
    stage: ProceduralStage,
) -> crate::model::legislative_context::LegislativeContext {
    crate::model::legislative_context::LegislativeContext {
        congress_number: 119,
        session: if snapshot_date.month() < 7 {
            crate::model::legislative_context::CongressionalSession::First
        } else {
            crate::model::legislative_context::CongressionalSession::Second
        },
        current_chamber: Chamber::Senate,
        procedural_stage: stage,
        majority_party: Party::Democrat,
        minority_party: Party::Republican,
        president_party: Party::Democrat,
        days_until_election: Some(300),
        days_until_deadline: Some(21),
        under_unanimous_consent: false,
        under_reconciliation: false,
        leadership_priority: 0.62,
        media_attention: 0.55,
    }
}
