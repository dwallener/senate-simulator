use crate::{
    error::SenateSimError,
    model::{
        legislative::{LegislativeObject, PolicyDomain},
        legislative_context::{LegislativeContext, ProceduralStage},
        senator::Senator,
        senator_stance::SenatorStance,
        stance_score_breakdown::StanceScoreBreakdown,
    },
};

use super::stance::{
    clamp_unit, derive_procedural_posture, derive_public_position, derive_stance_heuristic,
    derive_stance_label, normalize_signed_score,
};

pub fn derive_stance_feature_driven(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
) -> Result<SenatorStance, SenateSimError> {
    senator.validate()?;
    legislative_object.validate()?;
    context.validate()?;

    let coverage_score = senator.feature_coverage_score.unwrap_or(0.0);
    if coverage_score <= 0.05 {
        return heuristic_fallback(
            senator,
            legislative_object,
            context,
            coverage_score,
            "historical feature coverage is too sparse; using legacy heuristic stance scorer",
        );
    }

    let domain_affinity_score = domain_affinity_score(senator, &legislative_object.policy_domain);
    let party_alignment_score = party_alignment_score(senator, context);
    let procedural_compatibility_score =
        procedural_compatibility_score(senator, context, party_alignment_score);
    let salience_adjustment =
        ((legislative_object.salience - 0.5) * 0.20) + deadline_pressure(context) * 0.08;
    let controversy_adjustment = -((legislative_object.controversy - 0.4).max(0.0)
        * (0.12 + senator.structural.party_loyalty_baseline * 0.08));
    let recent_drift_adjustment = (senator.dynamic_state.current_procedural_support
        - senator.procedural.cloture_support_baseline)
        * 0.20
        + (senator.dynamic_state.current_party_pressure - senator.structural.party_loyalty_baseline)
            * 0.15;
    let attendance_adjustment = (senator.procedural.attendance_reliability - 0.9) * 0.25;

    let raw_substantive = domain_affinity_score * 0.62
        + party_alignment_score * 0.18
        + salience_adjustment * 0.35
        + controversy_adjustment * 0.20
        + recent_drift_adjustment * 0.15;
    let substantive_support = shrink_to_neutral(clamp_unit(raw_substantive), coverage_score);

    let raw_procedural = procedural_compatibility_score * 0.55
        + party_alignment_score * 0.22
        + deadline_pressure(context) * 0.08
        + recent_drift_adjustment * 0.20
        + if substantive_support >= 0.5 { 0.06 } else { -0.04 };
    let procedural_support = shrink_to_neutral(clamp_unit(raw_procedural), coverage_score);

    let negotiability_raw = senator.structural.bipartisanship_baseline * 0.28
        + senator.dynamic_state.current_negotiability * 0.24
        + senator.procedural.amendment_openness * 0.18
        + (1.0 - party_alignment_score) * 0.12
        + (0.5 - (substantive_support - 0.5).abs()) * 0.16
        - legislative_object.controversy * 0.16
        - context.media_attention * 0.06;
    let negotiability = shrink_to_neutral(clamp_unit(negotiability_raw), coverage_score);

    let rigidity = clamp_unit(
        senator.structural.party_loyalty_baseline * 0.26
            + senator.dynamic_state.current_party_pressure * 0.20
            + (1.0 - senator.structural.bipartisanship_baseline) * 0.10
            + (1.0 - negotiability) * 0.24
            + legislative_object.controversy * 0.12
            + context.leadership_priority * 0.08,
    );

    let defection_probability = clamp_unit(
        senator.structural.bipartisanship_baseline * 0.24
            + (1.0 - senator.structural.party_loyalty_baseline) * 0.26
            + (1.0 - context.leadership_priority) * 0.12
            + (0.5 - (substantive_support - 0.5).abs()) * 0.22
            - rigidity * 0.18,
    );

    let absence_probability = clamp_unit(
        (1.0 - senator.procedural.attendance_reliability) * 0.70
            + (1.0 - senator.dynamic_state.current_procedural_support) * 0.08
            + if context.days_until_election.unwrap_or(365) <= 30 {
                0.03
            } else {
                0.0
            }
            + (1.0 - coverage_score) * 0.04,
    );

    let public_support = shrink_to_neutral(
        clamp_unit(
            substantive_support * 0.48
                + party_alignment_score * 0.18
                + salience_adjustment * 0.28
                + controversy_adjustment * 0.24
                + rigidity * 0.12
                - negotiability * 0.10,
        ),
        coverage_score,
    );

    let mut fallback_notes = senator.feature_notes.clone();
    if coverage_score < 0.45 {
        fallback_notes.push("sparse historical coverage shrinks scores toward neutral".to_string());
    }

    let mut top_factors = vec![
        format!(
            "historical domain affinity is {:.2}",
            domain_affinity_score
        ),
        format!(
            "{:?} procedural compatibility is {:.2}",
            context.procedural_stage, procedural_compatibility_score
        ),
        format!(
            "party-alignment score is {:.2} under leadership priority {:.2}",
            party_alignment_score, context.leadership_priority
        ),
    ];
    if legislative_object.controversy >= 0.6 {
        top_factors.push("high controversy increases rigidity and public signaling".to_string());
    }
    if coverage_score < 0.45 {
        top_factors.push("sparse feature coverage shrinks extreme stance outputs".to_string());
    }
    for note in &fallback_notes {
        top_factors.push(note.clone());
    }
    let mut deduped_factors = Vec::new();
    for factor in top_factors {
        if !deduped_factors.contains(&factor) {
            deduped_factors.push(factor);
        }
    }
    deduped_factors.truncate(6);

    let score_breakdown = StanceScoreBreakdown {
        domain_affinity_score,
        procedural_compatibility_score,
        party_alignment_score,
        salience_adjustment,
        controversy_adjustment,
        recent_drift_adjustment,
        attendance_adjustment,
        coverage_score,
        fallback_notes,
        top_factors: deduped_factors.clone(),
    };

    let stance = SenatorStance {
        senator_id: senator.identity.senator_id.clone(),
        object_id: legislative_object.object_id.clone(),
        context_id: Some(format!(
            "{}-{:?}-{}",
            context.congress_number, context.procedural_stage, context.current_chamber
        )),
        substantive_support,
        procedural_support,
        public_support,
        negotiability,
        rigidity,
        defection_probability,
        absence_probability,
        stance_label: derive_stance_label(substantive_support),
        procedural_posture: derive_procedural_posture(procedural_support, &context.procedural_stage),
        public_position: derive_public_position(public_support),
        top_factors: deduped_factors,
        score_breakdown: Some(score_breakdown),
    };
    stance.validate()?;
    Ok(stance)
}

fn heuristic_fallback(
    senator: &Senator,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    coverage_score: f32,
    note: &str,
) -> Result<SenatorStance, SenateSimError> {
    let mut stance = derive_stance_heuristic(senator, legislative_object, context)?;
    let mut fallback_notes = senator.feature_notes.clone();
    fallback_notes.push(note.to_string());
    stance.top_factors.push(note.to_string());
    stance.score_breakdown = Some(StanceScoreBreakdown {
        domain_affinity_score: 0.5,
        procedural_compatibility_score: stance.procedural_support,
        party_alignment_score: senator.structural.party_loyalty_baseline,
        salience_adjustment: 0.0,
        controversy_adjustment: 0.0,
        recent_drift_adjustment: 0.0,
        attendance_adjustment: -(stance.absence_probability),
        coverage_score,
        fallback_notes,
        top_factors: stance.top_factors.clone(),
    });
    stance.validate()?;
    Ok(stance)
}

fn domain_affinity_score(senator: &Senator, domain: &PolicyDomain) -> f32 {
    let signed = match domain {
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
    normalize_signed_score(signed)
}

fn procedural_compatibility_score(
    senator: &Senator,
    context: &LegislativeContext,
    party_alignment_score: f32,
) -> f32 {
    let stage_base = match context.procedural_stage {
        ProceduralStage::MotionToProceed => senator.procedural.motion_to_proceed_baseline,
        ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote => {
            senator.procedural.cloture_support_baseline
        }
        ProceduralStage::Debate | ProceduralStage::AmendmentPending => {
            senator.procedural.amendment_openness
        }
        ProceduralStage::FinalPassage => {
            (senator.procedural.motion_to_proceed_baseline + senator.procedural.cloture_support_baseline)
                / 2.0
        }
        _ => senator.dynamic_state.current_procedural_support,
    };

    clamp_unit(
        stage_base * 0.62
            + senator.dynamic_state.current_procedural_support * 0.18
            + party_alignment_score * 0.12
            - senator.procedural.uc_objection_tendency * 0.10,
    )
}

fn party_alignment_score(senator: &Senator, context: &LegislativeContext) -> f32 {
    let leadership_side = if context.majority_party == senator.identity.party {
        1.0
    } else if context.minority_party == senator.identity.party {
        0.35
    } else {
        0.55
    };
    clamp_unit(
        senator.structural.party_loyalty_baseline * 0.45
            + senator.dynamic_state.current_party_pressure * 0.18
            + senator.procedural.leadership_deference * 0.15
            + leadership_side * context.leadership_priority * 0.17
            - senator.structural.bipartisanship_baseline * 0.15,
    )
}

fn deadline_pressure(context: &LegislativeContext) -> f32 {
    context
        .days_until_deadline
        .map(|days| {
            if days <= 7 {
                1.0
            } else if days <= 21 {
                0.75
            } else if days <= 45 {
                0.45
            } else {
                0.15
            }
        })
        .unwrap_or(0.05)
}

fn shrink_to_neutral(score: f32, coverage_score: f32) -> f32 {
    let shrink = (1.0 - coverage_score).clamp(0.0, 1.0) * 0.35;
    clamp_unit((score * (1.0 - shrink)) + (0.5 * shrink))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::{
        derive::{stance::derive_stance_with_mode, StanceDerivationMode},
        model::{
            dynamic_state::{DynamicState, PublicPosition},
            identity::{Identity, Party, SenateClass},
            issue_preferences::IssuePreferences,
            legislative::{
                BudgetaryImpact, LegislativeObject, LegislativeObjectType, PolicyDomain,
            },
            legislative_context::{
                Chamber, CongressionalSession, LegislativeContext, ProceduralStage,
            },
            procedural::Procedural,
            structural::Structural,
        },
    };

    use super::derive_stance_feature_driven;

    fn senator_for_feature_tests() -> crate::Senator {
        crate::Senator {
            identity: Identity {
                senator_id: "real_test_001".to_string(),
                full_name: "Feature Test Senator".to_string(),
                party: Party::Democrat,
                state: "WA".to_string(),
                class: SenateClass::I,
                start_date: NaiveDate::from_ymd_opt(2021, 1, 3).unwrap(),
                end_date: None,
            },
            structural: Structural {
                ideology_score: 0.15,
                party_loyalty_baseline: 0.78,
                bipartisanship_baseline: 0.32,
                committee_assignments: vec!["Energy".to_string()],
                reelection_year: Some(2028),
                electoral_vulnerability: 0.30,
            },
            issue_preferences: IssuePreferences {
                defense: 0.10,
                immigration: -0.25,
                energy_climate: 0.75,
                labor: 0.20,
                healthcare: 0.30,
                tax_spending: -0.10,
                judiciary: -0.05,
                trade: 0.05,
                tech_privacy: 0.12,
                foreign_policy: 0.08,
            },
            procedural: Procedural {
                cloture_support_baseline: 0.74,
                motion_to_proceed_baseline: 0.71,
                uc_objection_tendency: 0.12,
                leadership_deference: 0.79,
                amendment_openness: 0.67,
                attendance_reliability: 0.96,
            },
            dynamic_state: DynamicState {
                current_public_position: PublicPosition::Negotiating,
                current_substantive_support: 0.58,
                current_procedural_support: 0.72,
                current_negotiability: 0.60,
                current_party_pressure: 0.76,
                current_issue_salience_in_state: 0.55,
            },
            feature_coverage_score: Some(0.92),
            feature_notes: vec![],
        }
    }

    fn legislative_object(domain: PolicyDomain) -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_feature_001".to_string(),
            title: "Feature-Driven Test Bill".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: domain,
            summary: "Synthetic object for feature-driven stance tests.".to_string(),
            text_embedding_placeholder: None,
            sponsor: Some("real_test_010".to_string()),
            cosponsors: vec!["real_test_011".to_string()],
            origin_chamber: Chamber::Senate,
            introduced_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            current_version_label: Some("Test Version".to_string()),
            budgetary_impact: BudgetaryImpact::Moderate,
            salience: 0.72,
            controversy: 0.54,
        }
    }

    fn context(stage: ProceduralStage) -> LegislativeContext {
        LegislativeContext {
            congress_number: 119,
            session: CongressionalSession::First,
            current_chamber: Chamber::Senate,
            procedural_stage: stage,
            majority_party: Party::Democrat,
            minority_party: Party::Republican,
            president_party: Party::Democrat,
            days_until_election: Some(90),
            days_until_deadline: Some(14),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.84,
            media_attention: 0.63,
        }
    }

    #[test]
    fn domain_affinity_raises_substantive_support_for_aligned_senator() {
        let aligned = senator_for_feature_tests();
        let mut misaligned = senator_for_feature_tests();
        misaligned.issue_preferences.energy_climate = -0.75;

        let aligned_stance = derive_stance_feature_driven(
            &aligned,
            &legislative_object(PolicyDomain::EnergyClimate),
            &context(ProceduralStage::Debate),
        )
        .unwrap();
        let misaligned_stance = derive_stance_feature_driven(
            &misaligned,
            &legislative_object(PolicyDomain::EnergyClimate),
            &context(ProceduralStage::Debate),
        )
        .unwrap();

        assert!(aligned_stance.substantive_support > misaligned_stance.substantive_support);
    }

    #[test]
    fn procedural_compatibility_respects_cloture_history() {
        let mut strong = senator_for_feature_tests();
        strong.procedural.cloture_support_baseline = 0.90;
        strong.dynamic_state.current_procedural_support = 0.84;

        let mut weak = senator_for_feature_tests();
        weak.procedural.cloture_support_baseline = 0.28;
        weak.dynamic_state.current_procedural_support = 0.35;

        let strong_stance = derive_stance_feature_driven(
            &strong,
            &legislative_object(PolicyDomain::EnergyClimate),
            &context(ProceduralStage::ClotureVote),
        )
        .unwrap();
        let weak_stance = derive_stance_feature_driven(
            &weak,
            &legislative_object(PolicyDomain::EnergyClimate),
            &context(ProceduralStage::ClotureVote),
        )
        .unwrap();

        assert!(strong_stance.procedural_support > weak_stance.procedural_support);
    }

    #[test]
    fn recent_drift_changes_procedural_support() {
        let mut senator = senator_for_feature_tests();
        let object = legislative_object(PolicyDomain::EnergyClimate);
        let stage = context(ProceduralStage::ClotureFiled);

        senator.dynamic_state.current_procedural_support = 0.82;
        let supportive = derive_stance_feature_driven(&senator, &object, &stage).unwrap();

        senator.dynamic_state.current_procedural_support = 0.28;
        let drifting_away = derive_stance_feature_driven(&senator, &object, &stage).unwrap();

        assert!(supportive.procedural_support > drifting_away.procedural_support);
    }

    #[test]
    fn lower_attendance_history_increases_absence_probability() {
        let mut reliable = senator_for_feature_tests();
        reliable.procedural.attendance_reliability = 0.98;

        let mut unreliable = senator_for_feature_tests();
        unreliable.procedural.attendance_reliability = 0.62;

        let reliable_stance = derive_stance_feature_driven(
            &reliable,
            &legislative_object(PolicyDomain::BudgetTax),
            &context(ProceduralStage::MotionToProceed),
        )
        .unwrap();
        let unreliable_stance = derive_stance_feature_driven(
            &unreliable,
            &legislative_object(PolicyDomain::BudgetTax),
            &context(ProceduralStage::MotionToProceed),
        )
        .unwrap();

        assert!(unreliable_stance.absence_probability > reliable_stance.absence_probability);
    }

    #[test]
    fn leadership_priority_and_loyalty_raise_procedural_alignment() {
        let mut aligned = senator_for_feature_tests();
        aligned.structural.party_loyalty_baseline = 0.90;
        aligned.dynamic_state.current_party_pressure = 0.88;
        aligned.procedural.leadership_deference = 0.90;

        let mut independent = senator_for_feature_tests();
        independent.structural.party_loyalty_baseline = 0.45;
        independent.structural.bipartisanship_baseline = 0.70;
        independent.dynamic_state.current_party_pressure = 0.42;
        independent.procedural.leadership_deference = 0.35;

        let aligned_stance = derive_stance_feature_driven(
            &aligned,
            &legislative_object(PolicyDomain::Defense),
            &context(ProceduralStage::MotionToProceed),
        )
        .unwrap();
        let independent_stance = derive_stance_feature_driven(
            &independent,
            &legislative_object(PolicyDomain::Defense),
            &context(ProceduralStage::MotionToProceed),
        )
        .unwrap();

        assert!(aligned_stance.procedural_support > independent_stance.procedural_support);
        assert!(aligned_stance.public_support > independent_stance.public_support);
    }

    #[test]
    fn sparse_coverage_shrinks_scores_toward_neutral() {
        let rich = senator_for_feature_tests();
        let mut sparse = senator_for_feature_tests();
        sparse.feature_coverage_score = Some(0.18);
        sparse.feature_notes = vec!["limited vote history".to_string()];

        let rich_stance = derive_stance_feature_driven(
            &rich,
            &legislative_object(PolicyDomain::EnergyClimate),
            &context(ProceduralStage::Debate),
        )
        .unwrap();
        let sparse_stance = derive_stance_feature_driven(
            &sparse,
            &legislative_object(PolicyDomain::EnergyClimate),
            &context(ProceduralStage::Debate),
        )
        .unwrap();

        let rich_distance = (rich_stance.substantive_support - 0.5).abs();
        let sparse_distance = (sparse_stance.substantive_support - 0.5).abs();
        assert!(sparse_distance < rich_distance);
        assert!(sparse_stance
            .top_factors
            .iter()
            .any(|factor| factor.contains("sparse")));
    }

    #[test]
    fn fallback_to_heuristic_is_explicit_when_feature_coverage_is_missing() {
        let mut senator = senator_for_feature_tests();
        senator.feature_coverage_score = Some(0.0);

        let stance = derive_stance_feature_driven(
            &senator,
            &legislative_object(PolicyDomain::Healthcare),
            &context(ProceduralStage::Debate),
        )
        .unwrap();

        let breakdown = stance.score_breakdown.as_ref().unwrap();
        assert!(breakdown
            .fallback_notes
            .iter()
            .any(|note| note.contains("legacy heuristic")));
    }

    #[test]
    fn heuristic_and_feature_driven_modes_both_produce_valid_stances() {
        let senator = senator_for_feature_tests();
        let object = legislative_object(PolicyDomain::EnergyClimate);
        let ctx = context(ProceduralStage::Debate);

        let heuristic = derive_stance_with_mode(
            &senator,
            &object,
            &ctx,
            StanceDerivationMode::Heuristic,
        )
        .unwrap();
        let feature = derive_stance_with_mode(
            &senator,
            &object,
            &ctx,
            StanceDerivationMode::FeatureDriven,
        )
        .unwrap();

        assert!(heuristic.validate().is_ok());
        assert!(feature.validate().is_ok());
        assert!(feature.score_breakdown.is_some());
    }
}
