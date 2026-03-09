use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{
    error::SenateSimError,
    model::{legislative::PolicyDomain, normalized_public_signal_record::NormalizedPublicSignalRecord},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SenatorObjectSignalLink {
    pub senator_id: String,
    pub object_id: String,
    pub attention_score: f32,
    pub public_association_score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicSignalSummary {
    pub snapshot_date: NaiveDate,
    pub object_attention: HashMap<String, f32>,
    pub senator_attention: HashMap<String, f32>,
    pub domain_attention: HashMap<PolicyDomain, f32>,
    pub senator_object_link_strength: Vec<SenatorObjectSignalLink>,
    pub notes: Vec<String>,
}

impl PublicSignalSummary {
    pub fn validate(&self) -> Result<(), SenateSimError> {
        for (field, values) in [
            ("public_signal_summary.object_attention", self.object_attention.values().collect::<Vec<_>>()),
            ("public_signal_summary.senator_attention", self.senator_attention.values().collect::<Vec<_>>()),
            ("public_signal_summary.domain_attention", self.domain_attention.values().collect::<Vec<_>>()),
        ] {
            for value in values {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(SenateSimError::Validation {
                        field,
                        message: "must be between 0 and 1".to_string(),
                    });
                }
            }
        }
        for link in &self.senator_object_link_strength {
            for (field, value) in [
                ("public_signal_summary.senator_object_link_strength.attention_score", link.attention_score),
                (
                    "public_signal_summary.senator_object_link_strength.public_association_score",
                    link.public_association_score,
                ),
            ] {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(SenateSimError::Validation {
                        field,
                        message: "must be between 0 and 1".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

pub fn build_public_signal_summary(
    snapshot_date: NaiveDate,
    records: &[NormalizedPublicSignalRecord],
) -> PublicSignalSummary {
    let mut object_attention = HashMap::new();
    let mut senator_attention = HashMap::new();
    let mut domain_attention = HashMap::new();
    let mut domain_by_senator = HashMap::new();
    let mut domain_by_object = HashMap::new();

    for record in records {
        if let Some(object_id) = &record.linked_object_id {
            object_attention
                .entry(object_id.clone())
                .and_modify(|value: &mut f32| *value = value.max(record.attention_score))
                .or_insert(record.attention_score);
            if let Some(domain) = &record.policy_domain {
                domain_by_object.insert(object_id.clone(), domain.clone());
            }
        }
        if let Some(senator_id) = &record.linked_senator_id {
            senator_attention
                .entry(senator_id.clone())
                .and_modify(|value: &mut f32| *value = value.max(record.attention_score))
                .or_insert(record.attention_score);
            if let Some(domain) = &record.policy_domain {
                domain_by_senator.insert(senator_id.clone(), domain.clone());
            }
        }
        if let Some(domain) = &record.policy_domain {
            domain_attention
                .entry(domain.clone())
                .and_modify(|value: &mut f32| *value = value.max(record.attention_score))
                .or_insert(record.attention_score);
        }
    }

    let mut links = Vec::new();
    for (senator_id, senator_score) in &senator_attention {
        for (object_id, object_score) in &object_attention {
            let same_domain = domain_by_senator
                .get(senator_id)
                .zip(domain_by_object.get(object_id))
                .map(|(left, right)| left == right)
                .unwrap_or(false);
            if same_domain {
                links.push(SenatorObjectSignalLink {
                    senator_id: senator_id.clone(),
                    object_id: object_id.clone(),
                    attention_score: ((*senator_score + *object_score) / 2.0).clamp(0.0, 1.0),
                    public_association_score: ((*senator_score * 0.55) + (*object_score * 0.45))
                        .clamp(0.0, 1.0),
                });
            }
        }
    }

    let notes = if records.is_empty() {
        vec!["no public-signal records available for this snapshot".to_string()]
    } else {
        vec![format!(
            "aggregated {} normalized public-signal records",
            records.len()
        )]
    };

    PublicSignalSummary {
        snapshot_date,
        object_attention,
        senator_attention,
        domain_attention,
        senator_object_link_strength: links,
        notes,
    }
}
