use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{
        identity::{Party, SenateClass},
        legislative::{BudgetaryImpact, LegislativeObjectType, PolicyDomain},
        legislative_context::{Chamber, ProceduralStage},
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedSenatorRecord {
    pub senator_id: String,
    pub full_name: String,
    pub party: Party,
    pub state: String,
    pub class: SenateClass,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub source_member_id: String,
    pub as_of_date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedLegislativeRecord {
    pub object_id: String,
    pub title: String,
    pub summary: String,
    pub object_type: LegislativeObjectType,
    pub policy_domain: PolicyDomain,
    pub sponsor: Option<String>,
    pub introduced_date: NaiveDate,
    pub latest_status_text: Option<String>,
    pub current_stage: ProceduralStage,
    pub origin_chamber: Chamber,
    pub budgetary_impact: BudgetaryImpact,
    pub salience: f32,
    pub controversy: f32,
    pub as_of_date: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizedActionCategory {
    Introduced,
    Referred,
    Reported,
    MotionToProceed,
    Debate,
    Amendment,
    Cloture,
    Passage,
    Stall,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedActionRecord {
    pub action_id: String,
    pub object_id: String,
    pub action_date: NaiveDate,
    pub chamber: Chamber,
    pub action_text: String,
    pub category: NormalizedActionCategory,
    pub as_of_date: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VotePosition {
    Yea,
    Nay,
    Present,
    NotVoting,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteCategory {
    Passage,
    Cloture,
    MotionToProceed,
    Amendment,
    Nomination,
    Procedural,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProceduralKind {
    Cloture,
    MotionToProceed,
    AmendmentProcess,
    Table,
    Recommit,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedVoteRecord {
    pub vote_id: String,
    pub vote_date: NaiveDate,
    pub senator_id: String,
    pub senator_name: String,
    pub object_id: Option<String>,
    pub vote_category: VoteCategory,
    pub vote_position: VotePosition,
    pub party_at_time: Party,
    pub policy_domain: Option<PolicyDomain>,
    pub is_procedural: bool,
    pub procedural_kind: Option<ProceduralKind>,
    pub as_of_date: NaiveDate,
}

impl NormalizedSenatorRecord {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.senator_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_senator_record.senator_id",
                message: "must not be empty".to_string(),
            });
        }

        if self.full_name.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_senator_record.full_name",
                message: "must not be empty".to_string(),
            });
        }

        if self.state.len() != 2 || !self.state.chars().all(|value| value.is_ascii_uppercase()) {
            return Err(SenateSimError::Validation {
                field: "normalized_senator_record.state",
                message: "must be a 2-letter uppercase code".to_string(),
            });
        }

        Ok(())
    }
}

impl NormalizedLegislativeRecord {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.object_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_legislative_record.object_id",
                message: "must not be empty".to_string(),
            });
        }

        if self.title.trim().is_empty() || self.summary.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_legislative_record.title",
                message: "title and summary must not be empty".to_string(),
            });
        }

        for (field, value) in [
            ("normalized_legislative_record.salience", self.salience),
            (
                "normalized_legislative_record.controversy",
                self.controversy,
            ),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(SenateSimError::Validation {
                    field,
                    message: "must be between 0 and 1".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl NormalizedActionRecord {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.action_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_action_record.action_id",
                message: "must not be empty".to_string(),
            });
        }

        if self.object_id.trim().is_empty() || self.action_text.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_action_record.object_id",
                message: "object_id and action_text must not be empty".to_string(),
            });
        }

        Ok(())
    }
}

impl NormalizedVoteRecord {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        if self.vote_id.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_vote_record.vote_id",
                message: "must not be empty".to_string(),
            });
        }
        if self.senator_id.trim().is_empty() || self.senator_name.trim().is_empty() {
            return Err(SenateSimError::Validation {
                field: "normalized_vote_record.senator_id",
                message: "senator_id and senator_name must not be empty".to_string(),
            });
        }
        Ok(())
    }
}
