use crate::{
    error::SenateSimError,
    model::{
        dynamic_state::PublicPosition,
        identity::Party,
        legislative::{BudgetaryImpact, LegislativeObject, PolicyDomain},
        legislative_context::{LegislativeContext, ProceduralStage},
        senator::Senator,
        senator_stance::{ProceduralPosture, SenatorStance, StanceLabel},
    },
};

#[derive(Debug, Clone, Copy)]
struct DerivedScores {
    substantive_support: f32,
    procedural_support: f32,
    public_support: f32,
    negotiability: f32,
    rigidity: f32,
    defection_probability: f32,
    absence_probability: f32,
}

pub fn derive_stance(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
) -> Result<SenatorStance, SenateSimError> {
    senator.validate()?;
    legislative_object.validate()?;
    context.validate()?;

    let substantive_support = derive_substantive_support(senator, legislative_object);
    let negotiability =
        derive_negotiability(senator, legislative_object, context, substantive_support);
    let rigidity = derive_rigidity(senator, context, negotiability, substantive_support);
    let procedural_support =
        derive_procedural_support(senator, legislative_object, context, substantive_support);
    let public_support = derive_public_support(
        senator,
        legislative_object,
        context,
        substantive_support,
        negotiability,
    );
    let defection_probability =
        derive_defection_probability(senator, legislative_object, context, substantive_support);
    let absence_probability = derive_absence_probability(senator, context);

    let scores = DerivedScores {
        substantive_support,
        procedural_support,
        public_support,
        negotiability,
        rigidity,
        defection_probability,
        absence_probability,
    };

    let stance = SenatorStance {
        senator_id: senator.identity.senator_id.clone(),
        object_id: legislative_object.object_id.clone(),
        context_id: Some(build_context_id(context)),
        substantive_support,
        procedural_support,
        public_support,
        negotiability,
        rigidity,
        defection_probability,
        absence_probability,
        stance_label: derive_stance_label(substantive_support),
        procedural_posture: derive_procedural_posture(
            procedural_support,
            &context.procedural_stage,
        ),
        public_position: derive_public_position(public_support),
        top_factors: build_top_factors(senator, legislative_object, context, scores),
    };

    stance.validate()?;
    Ok(stance)
}

pub fn normalize_signed_score(x: f32) -> f32 {
    clamp_unit((x + 1.0) / 2.0)
}

pub fn clamp_unit(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }

    x.clamp(0.0, 1.0)
}

pub fn base_issue_alignment(senator: &Senator, domain: &PolicyDomain) -> f32 {
    let score = match domain {
        PolicyDomain::Defense => senator.issue_preferences.defense,
        PolicyDomain::BudgetTax => senator.issue_preferences.tax_spending,
        PolicyDomain::Healthcare => senator.issue_preferences.healthcare,
        PolicyDomain::Immigration => senator.issue_preferences.immigration,
        PolicyDomain::EnergyClimate => senator.issue_preferences.energy_climate,
        PolicyDomain::Judiciary => senator.issue_preferences.judiciary,
        PolicyDomain::Technology => senator.issue_preferences.tech_privacy,
        PolicyDomain::ForeignPolicy => senator.issue_preferences.foreign_policy,
        PolicyDomain::Labor => senator.issue_preferences.labor,
        PolicyDomain::Education => {
            (senator.issue_preferences.healthcare + senator.issue_preferences.labor) / 2.0
        }
        PolicyDomain::Other(_) => senator.structural.ideology_score * 0.5,
    };

    normalize_signed_score(score)
}

fn derive_substantive_support(senator: &Senator, legislative_object: &LegislativeObject) -> f32 {
    let base_alignment = base_issue_alignment(senator, &legislative_object.policy_domain);
    let dynamic_anchor = senator.dynamic_state.current_substantive_support;
    let ideology_anchor = normalize_signed_score(senator.structural.ideology_score);

    // Salient objects should pull the score further toward the senator's underlying issue view.
    let salience_amplifier = (legislative_object.salience - 0.5) * 0.20;
    let controversy_penalty =
        legislative_object.controversy * (1.0 - senator.structural.bipartisanship_baseline) * 0.18;
    let budget_penalty = if matches!(legislative_object.budgetary_impact, BudgetaryImpact::High) {
        (1.0 - normalize_signed_score(senator.issue_preferences.tax_spending)) * 0.16
    } else {
        0.0
    };

    let weighted = (base_alignment * 0.55)
        + (dynamic_anchor * 0.25)
        + (ideology_anchor * 0.10)
        + (base_alignment - 0.5) * salience_amplifier
        - controversy_penalty
        - budget_penalty;

    clamp_unit(weighted)
}

fn derive_procedural_support(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    substantive_support: f32,
) -> f32 {
    let stage_baseline = match context.procedural_stage {
        ProceduralStage::MotionToProceed => senator.procedural.motion_to_proceed_baseline,
        ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote => {
            senator.procedural.cloture_support_baseline
        }
        ProceduralStage::Debate | ProceduralStage::AmendmentPending => {
            senator.procedural.amendment_openness
        }
        ProceduralStage::FinalPassage => substantive_support,
        _ => senator.procedural.motion_to_proceed_baseline,
    };

    let stage_weight = match context.procedural_stage {
        ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote => 0.24,
        ProceduralStage::MotionToProceed => 0.18,
        ProceduralStage::Debate | ProceduralStage::AmendmentPending => 0.14,
        ProceduralStage::FinalPassage => 0.10,
        _ => 0.08,
    };

    let leadership_boost =
        context.leadership_priority * senator.procedural.leadership_deference * 0.22;
    let party_pressure_boost = senator.dynamic_state.current_party_pressure * 0.12;
    let controversy_drag =
        legislative_object.controversy * senator.procedural.uc_objection_tendency * 0.10;
    let substance_bridge = (substantive_support - 0.5) * 0.18;

    clamp_unit(
        (stage_baseline * 0.42)
            + (stage_baseline * stage_weight)
            + (senator.dynamic_state.current_procedural_support * 0.18)
            + leadership_boost
            + party_pressure_boost
            + substance_bridge
            - controversy_drag,
    )
}

fn derive_public_support(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    substantive_support: f32,
    negotiability: f32,
) -> f32 {
    let public_position_bias = match senator.dynamic_state.current_public_position {
        PublicPosition::Support => 0.20,
        PublicPosition::Oppose => -0.20,
        PublicPosition::Undeclared => 0.0,
        PublicPosition::Negotiating => 0.04,
        PublicPosition::Mixed => -0.02,
    };

    let intensity = legislative_object.salience * 0.08 + legislative_object.controversy * 0.06;
    let pressure_bias = senator.dynamic_state.current_party_pressure * 0.10;
    let rigidity_bias = (1.0 - negotiability) * 0.08;
    let media_push = context.media_attention * 0.05;

    let weighted = (substantive_support * 0.58)
        + (senator.dynamic_state.current_substantive_support * 0.12)
        + public_position_bias
        + if substantive_support >= 0.5 {
            intensity
        } else {
            -intensity
        }
        + if substantive_support >= 0.5 {
            pressure_bias * 0.5
        } else {
            -pressure_bias
        }
        + if substantive_support >= 0.5 {
            rigidity_bias * 0.4
        } else {
            -rigidity_bias
        }
        + if substantive_support >= 0.5 {
            media_push * 0.5
        } else {
            -media_push * 0.5
        };

    clamp_unit(weighted)
}

fn derive_negotiability(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    substantive_support: f32,
) -> f32 {
    let public_position_penalty = match senator.dynamic_state.current_public_position {
        PublicPosition::Support | PublicPosition::Oppose => 0.18,
        PublicPosition::Negotiating => 0.04,
        PublicPosition::Mixed => 0.08,
        PublicPosition::Undeclared => 0.02,
    };

    clamp_unit(
        (senator.dynamic_state.current_negotiability * 0.42)
            + (senator.procedural.amendment_openness * 0.20)
            + (senator.structural.bipartisanship_baseline * 0.16)
            + ((0.5 - (substantive_support - 0.5).abs()) * 0.12)
            - (legislative_object.controversy * 0.12)
            - (senator.dynamic_state.current_issue_salience_in_state * 0.10)
            - (context.media_attention * 0.04)
            - public_position_penalty,
    )
}

fn derive_rigidity(
    senator: &Senator,
    context: &LegislativeContext,
    negotiability: f32,
    substantive_support: f32,
) -> f32 {
    let public_position_strength = match senator.dynamic_state.current_public_position {
        PublicPosition::Support | PublicPosition::Oppose => 0.18,
        PublicPosition::Negotiating => 0.06,
        PublicPosition::Mixed => 0.10,
        PublicPosition::Undeclared => 0.03,
    };
    let issue_conviction = (substantive_support - 0.5).abs() * 0.20;
    let leadership_lock =
        context.leadership_priority * senator.procedural.leadership_deference * 0.16;

    clamp_unit(
        ((1.0 - negotiability) * 0.58)
            + (senator.dynamic_state.current_party_pressure * 0.16)
            + leadership_lock
            + public_position_strength
            + issue_conviction,
    )
}

fn derive_defection_probability(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    substantive_support: f32,
) -> f32 {
    let expected_party_direction =
        expected_party_support(&senator.identity.party, &legislative_object.policy_domain);
    let divergence = (substantive_support - expected_party_direction).abs();
    let moderation = 1.0 - ((substantive_support - 0.5).abs() * 2.0);

    clamp_unit(
        0.08 + (senator.structural.bipartisanship_baseline * 0.28)
            + (divergence * 0.24)
            + (moderation * 0.12)
            - (senator.structural.party_loyalty_baseline * 0.24)
            - (context.leadership_priority * 0.16),
    )
}

fn derive_absence_probability(senator: &Senator, context: &LegislativeContext) -> f32 {
    let election_adjustment = context
        .days_until_election
        .map(|days| if days <= 30 { 0.03 } else { 0.0 })
        .unwrap_or(0.0);

    clamp_unit((1.0 - senator.procedural.attendance_reliability) + election_adjustment)
}

pub fn derive_stance_label(score: f32) -> StanceLabel {
    if score >= 0.80 {
        StanceLabel::Support
    } else if score >= 0.60 {
        StanceLabel::LeanSupport
    } else if score > 0.40 {
        StanceLabel::Undecided
    } else if score >= 0.20 {
        StanceLabel::LeanOppose
    } else {
        StanceLabel::Oppose
    }
}

pub fn derive_public_position(score: f32) -> PublicPosition {
    if score >= 0.75 {
        PublicPosition::Support
    } else if score >= 0.58 {
        PublicPosition::Negotiating
    } else if score > 0.42 {
        PublicPosition::Undeclared
    } else if score >= 0.25 {
        PublicPosition::Mixed
    } else {
        PublicPosition::Oppose
    }
}

pub fn derive_procedural_posture(score: f32, stage: &ProceduralStage) -> ProceduralPosture {
    match stage {
        ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote => {
            if score >= 0.60 {
                ProceduralPosture::SupportCloture
            } else if score <= 0.35 {
                ProceduralPosture::OpposeCloture
            } else {
                ProceduralPosture::Unclear
            }
        }
        ProceduralStage::Debate | ProceduralStage::MotionToProceed => {
            if score >= 0.60 {
                ProceduralPosture::SupportDebate
            } else if score <= 0.22 {
                ProceduralPosture::OpposeDebate
            } else {
                ProceduralPosture::Unclear
            }
        }
        ProceduralStage::AmendmentPending => {
            if score >= 0.58 {
                ProceduralPosture::SupportAmendmentProcess
            } else if score <= 0.25 {
                ProceduralPosture::BlockByProcedure
            } else {
                ProceduralPosture::Unclear
            }
        }
        _ => {
            if score <= 0.20 {
                ProceduralPosture::BlockByProcedure
            } else {
                ProceduralPosture::Unclear
            }
        }
    }
}

fn build_top_factors(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    scores: DerivedScores,
) -> Vec<String> {
    let mut factors = Vec::new();

    let domain_label = match legislative_object.policy_domain {
        PolicyDomain::Defense => "defense",
        PolicyDomain::BudgetTax => "tax_spending",
        PolicyDomain::Healthcare => "healthcare",
        PolicyDomain::Immigration => "immigration",
        PolicyDomain::EnergyClimate => "energy_climate",
        PolicyDomain::Judiciary => "judiciary",
        PolicyDomain::Technology => "tech_privacy",
        PolicyDomain::ForeignPolicy => "foreign_policy",
        PolicyDomain::Labor => "labor",
        PolicyDomain::Education => "education-adjacent",
        PolicyDomain::Other(_) => "general ideology",
    };

    if scores.substantive_support >= 0.65 {
        factors.push(format!(
            "high substantive alignment on {domain_label} policy"
        ));
    } else if scores.substantive_support <= 0.35 {
        factors.push(format!(
            "weak substantive alignment on {domain_label} policy"
        ));
    }

    if context.leadership_priority >= 0.70 && senator.procedural.leadership_deference >= 0.60 {
        factors.push("leadership priority boosts procedural support".to_string());
    }

    if legislative_object.controversy >= 0.65 {
        factors.push("high controversy increases rigidity".to_string());
    }

    match senator.dynamic_state.current_public_position {
        PublicPosition::Support => {
            factors.push("existing public support raises visible backing".to_string())
        }
        PublicPosition::Oppose => {
            factors.push("current public opposition lowers negotiability".to_string())
        }
        PublicPosition::Negotiating => {
            factors.push("current negotiating posture preserves flexibility".to_string())
        }
        PublicPosition::Mixed | PublicPosition::Undeclared => {}
    }

    if scores.procedural_support >= 0.65 {
        factors.push("procedural baselines support moving the measure forward".to_string());
    }

    if scores.absence_probability <= 0.08 {
        factors.push("strong attendance reliability keeps absence probability low".to_string());
    }

    if factors.len() < 3 && legislative_object.salience >= 0.70 {
        factors.push("high bill salience sharpens the senator's stance".to_string());
    }

    if factors.len() < 3 && scores.public_support <= 0.35 {
        factors.push("public posture remains defensive under controversy and pressure".to_string());
    }

    if factors.len() < 3 && scores.negotiability >= 0.60 {
        factors.push("negotiability remains elevated because amendment space is open".to_string());
    }

    if factors.len() < 3 && scores.defection_probability >= 0.35 {
        factors.push("cross-party tendencies raise defection risk".to_string());
    }

    if factors.len() < 3 && scores.rigidity >= 0.60 {
        factors.push("party pressure and low flexibility increase rigidity".to_string());
    }

    factors.truncate(5);
    factors
}

fn expected_party_support(party: &Party, domain: &PolicyDomain) -> f32 {
    match party {
        Party::Democrat => match domain {
            PolicyDomain::EnergyClimate
            | PolicyDomain::Healthcare
            | PolicyDomain::Labor
            | PolicyDomain::Technology
            | PolicyDomain::Education => 0.68,
            PolicyDomain::Defense | PolicyDomain::ForeignPolicy => 0.54,
            PolicyDomain::BudgetTax | PolicyDomain::Immigration | PolicyDomain::Judiciary => 0.42,
            PolicyDomain::Other(_) => 0.50,
        },
        Party::Republican => match domain {
            PolicyDomain::Defense | PolicyDomain::ForeignPolicy | PolicyDomain::Judiciary => 0.66,
            PolicyDomain::BudgetTax | PolicyDomain::Immigration => 0.62,
            PolicyDomain::EnergyClimate | PolicyDomain::Labor | PolicyDomain::Healthcare => 0.34,
            PolicyDomain::Technology | PolicyDomain::Education => 0.44,
            PolicyDomain::Other(_) => 0.50,
        },
        Party::Independent => 0.50,
        Party::Other(_) => 0.50,
    }
}

fn build_context_id(context: &LegislativeContext) -> String {
    format!(
        "ctx_{}_session_{:?}_{:?}",
        context.congress_number, context.session, context.procedural_stage
    )
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::model::{
        dynamic_state::{DynamicState, PublicPosition},
        identity::{Identity, Party, SenateClass},
        issue_preferences::IssuePreferences,
        legislative::{BudgetaryImpact, LegislativeObject, LegislativeObjectType, PolicyDomain},
        legislative_context::{Chamber, CongressionalSession, LegislativeContext, ProceduralStage},
        procedural::Procedural,
        structural::Structural,
    };

    use super::{
        derive_procedural_posture, derive_public_position, derive_stance, normalize_signed_score,
    };

    fn senator_with_energy_score(energy_score: f32) -> crate::Senator {
        crate::Senator {
            identity: Identity {
                senator_id: "sen_001".to_string(),
                full_name: "Example Senator".to_string(),
                party: Party::Democrat,
                state: "OR".to_string(),
                class: SenateClass::II,
                start_date: NaiveDate::from_ymd_opt(2023, 1, 3).unwrap(),
                end_date: Some(NaiveDate::from_ymd_opt(2029, 1, 3).unwrap()),
            },
            structural: Structural {
                ideology_score: 0.2,
                party_loyalty_baseline: 0.74,
                bipartisanship_baseline: 0.48,
                committee_assignments: vec!["Energy".to_string()],
                reelection_year: Some(2028),
                electoral_vulnerability: 0.33,
            },
            issue_preferences: IssuePreferences {
                defense: 0.1,
                immigration: -0.2,
                energy_climate: energy_score,
                labor: 0.3,
                healthcare: 0.2,
                tax_spending: -0.1,
                judiciary: 0.0,
                trade: 0.0,
                tech_privacy: 0.2,
                foreign_policy: 0.1,
            },
            procedural: Procedural {
                cloture_support_baseline: 0.72,
                motion_to_proceed_baseline: 0.78,
                uc_objection_tendency: 0.14,
                leadership_deference: 0.82,
                amendment_openness: 0.74,
                attendance_reliability: 0.97,
            },
            dynamic_state: DynamicState {
                current_public_position: PublicPosition::Negotiating,
                current_substantive_support: 0.55,
                current_procedural_support: 0.68,
                current_negotiability: 0.72,
                current_party_pressure: 0.61,
                current_issue_salience_in_state: 0.58,
            },
        }
    }

    fn example_object() -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Clean Grid Permitting Reform Act".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: PolicyDomain::EnergyClimate,
            summary: "Fictional permitting reform for transmission and generation.".to_string(),
            text_embedding_placeholder: None,
            sponsor: Some("sen_014".to_string()),
            cosponsors: vec!["sen_021".to_string()],
            origin_chamber: Chamber::Senate,
            introduced_date: NaiveDate::from_ymd_opt(2026, 2, 11).unwrap(),
            current_version_label: Some("Reported Substitute".to_string()),
            budgetary_impact: BudgetaryImpact::Moderate,
            salience: 0.78,
            controversy: 0.63,
        }
    }

    fn example_context() -> LegislativeContext {
        LegislativeContext {
            congress_number: 119,
            session: CongressionalSession::First,
            current_chamber: Chamber::Senate,
            procedural_stage: ProceduralStage::ClotureFiled,
            majority_party: Party::Democrat,
            minority_party: Party::Republican,
            president_party: Party::Democrat,
            days_until_election: Some(120),
            days_until_deadline: Some(18),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.82,
            media_attention: 0.67,
        }
    }

    #[test]
    fn normalization_maps_signed_range_to_unit_interval() {
        assert_eq!(normalize_signed_score(-1.0), 0.0);
        assert_eq!(normalize_signed_score(0.0), 0.5);
        assert_eq!(normalize_signed_score(1.0), 1.0);
    }

    #[test]
    fn stronger_domain_alignment_produces_higher_substantive_support() {
        let context = example_context();
        let legislative_object = example_object();
        let supportive = derive_stance(
            &senator_with_energy_score(0.9),
            &legislative_object,
            &context,
        )
        .unwrap();
        let oppositional = derive_stance(
            &senator_with_energy_score(-0.9),
            &legislative_object,
            &context,
        )
        .unwrap();

        assert!(supportive.substantive_support > oppositional.substantive_support);
    }

    #[test]
    fn procedural_support_can_exceed_weak_substantive_support() {
        let mut senator = senator_with_energy_score(-0.7);
        senator.procedural.cloture_support_baseline = 0.92;
        senator.procedural.motion_to_proceed_baseline = 0.91;
        senator.procedural.leadership_deference = 0.93;
        senator.dynamic_state.current_party_pressure = 0.80;

        let stance = derive_stance(&senator, &example_object(), &example_context()).unwrap();

        assert!(stance.substantive_support < 0.50);
        assert!(stance.procedural_support >= 0.60);
        assert!(matches!(
            derive_procedural_posture(stance.procedural_support, &ProceduralStage::ClotureFiled),
            crate::ProceduralPosture::SupportCloture
        ));
    }

    #[test]
    fn public_position_thresholds_map_to_support_and_oppose() {
        assert!(matches!(
            derive_public_position(0.82),
            PublicPosition::Support
        ));
        assert!(matches!(
            derive_public_position(0.10),
            PublicPosition::Oppose
        ));
    }

    #[test]
    fn derived_stance_validates_on_example_inputs() {
        let stance = derive_stance(
            &senator_with_energy_score(0.7),
            &example_object(),
            &example_context(),
        )
        .unwrap();

        assert!(stance.validate().is_ok());
        assert!(!stance.top_factors.is_empty());
    }
}
