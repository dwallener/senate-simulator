use crate::{
    error::SenateSimError,
    model::{
        floor_action_assessment::{FloorAction, FloorActionAssessment},
        legislative::LegislativeObject,
        legislative_context::{LegislativeContext, ProceduralStage},
        senate_analysis::SenateAnalysis,
    },
};

pub fn assess_floor_action(
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    analysis: &SenateAnalysis,
) -> Result<FloorActionAssessment, SenateSimError> {
    legislative_object.validate()?;
    context.validate()?;
    analysis.validate()?;

    if analysis.object_id != legislative_object.object_id {
        return Err(SenateSimError::Validation {
            field: "floor_action_assessment.object_id",
            message: "analysis object_id must match the legislative object".to_string(),
        });
    }

    let majority_threshold = (analysis.expected_present_count / 2) + 1;
    let current_support = analysis.likely_support_count + analysis.lean_support_count;
    let support_margin_estimate = current_support as i32 - majority_threshold as i32;
    let probable_cloture_votes = estimate_cloture_votes(analysis);
    let cloture_gap_estimate = probable_cloture_votes as i32 - 60;
    let pressure = deadline_pressure(context) * 0.5 + context.media_attention * 0.5;

    let predicted_action = match context.procedural_stage {
        ProceduralStage::FinalPassage => {
            if analysis.simple_majority_viable && analysis.coalition_stability >= 0.58 {
                FloorAction::LikelyFinalPassage
            } else if analysis.simple_majority_viable {
                FloorAction::LikelyNegotiation
            } else {
                FloorAction::LikelyStall
            }
        }
        ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote => {
            if analysis.cloture_viable && analysis.coalition_stability >= 0.55 {
                FloorAction::LikelyFinalPassage
            } else if analysis.simple_majority_viable
                && !analysis.cloture_viable
                && analysis.filibuster_risk >= 0.65
                && !analysis.likely_blockers.is_empty()
            {
                FloorAction::LikelyClotureFailure
            } else if analysis.simple_majority_viable && context.leadership_priority >= 0.70 {
                FloorAction::LikelyClotureVote
            } else if analysis.simple_majority_viable {
                FloorAction::LikelyNegotiation
            } else {
                FloorAction::LikelyProceduralBlock
            }
        }
        ProceduralStage::MotionToProceed | ProceduralStage::Debate => {
            if analysis.simple_majority_viable
                && analysis.coalition_stability >= 0.62
                && analysis.cloture_viable
            {
                FloorAction::LikelyAdvanceToDebate
            } else if analysis.simple_majority_viable
                && analysis.coalition_stability >= 0.42
                && analysis.filibuster_risk < 0.65
            {
                FloorAction::LikelyNegotiation
            } else if !analysis.simple_majority_viable
                && analysis.undecided_count >= 8
                && (context.leadership_priority >= 0.65 || pressure >= 0.60)
            {
                FloorAction::LikelyNegotiation
            } else if !analysis.likely_blockers.is_empty() && analysis.filibuster_risk >= 0.60 {
                FloorAction::LikelyProceduralBlock
            } else {
                FloorAction::LikelyStall
            }
        }
        ProceduralStage::AmendmentPending => {
            if analysis.pivotal_senators.len() >= 4 && analysis.coalition_stability < 0.58 {
                FloorAction::LikelyAmendmentFight
            } else if analysis.simple_majority_viable {
                FloorAction::LikelyNegotiation
            } else {
                FloorAction::LikelyStall
            }
        }
        ProceduralStage::Stalled => {
            if analysis.simple_majority_viable
                && (context.leadership_priority >= 0.75 || pressure >= 0.70)
            {
                FloorAction::LikelyNegotiation
            } else {
                FloorAction::LikelyStall
            }
        }
        _ => {
            if analysis.cloture_viable && analysis.coalition_stability >= 0.60 {
                FloorAction::LikelyAdvanceToDebate
            } else if analysis.simple_majority_viable {
                FloorAction::LikelyNegotiation
            } else {
                FloorAction::LikelyStall
            }
        }
    };

    let confidence = derive_confidence(
        &predicted_action,
        analysis,
        support_margin_estimate,
        cloture_gap_estimate,
    );

    let mut top_reasons = vec![
        if analysis.simple_majority_viable {
            "simple majority coalition exists in the current chamber snapshot".to_string()
        } else {
            "simple majority coalition does not yet exist".to_string()
        },
        if analysis.cloture_viable {
            "procedural coalition appears strong enough to clear cloture".to_string()
        } else {
            "cloture path remains below the 60-vote threshold".to_string()
        },
        if context.leadership_priority >= 0.70 {
            "high leadership priority raises the odds of an attempted floor move".to_string()
        } else {
            "limited leadership priority reduces pressure for immediate floor action".to_string()
        },
        if analysis.coalition_stability >= 0.60 {
            "coalition stability is strong enough to support decisive action".to_string()
        } else {
            "fragile coalition dynamics point toward bargaining rather than clean passage"
                .to_string()
        },
    ];

    if analysis.filibuster_risk >= 0.65 {
        top_reasons.push("rigid procedural blockers keep filibuster risk elevated".to_string());
    }
    if pressure >= 0.60 {
        top_reasons
            .push("deadline and media pressure push leadership toward visible action".to_string());
    }
    top_reasons.truncate(6);

    let assessment = FloorActionAssessment {
        object_id: legislative_object.object_id.clone(),
        procedural_stage: context.procedural_stage.clone(),
        predicted_action,
        confidence,
        simple_majority_viable: analysis.simple_majority_viable,
        cloture_viable: analysis.cloture_viable,
        coalition_stability: analysis.coalition_stability,
        filibuster_risk: analysis.filibuster_risk,
        support_margin_estimate,
        cloture_gap_estimate,
        pivotal_senators: analysis.pivotal_senators.clone(),
        top_reasons,
    };

    assessment.validate()?;
    Ok(assessment)
}

fn estimate_cloture_votes(analysis: &SenateAnalysis) -> usize {
    let blocker_penalty = analysis.likely_blockers.len() as i32;
    let pivot_bonus = (analysis.pivotal_senators.len() as i32 / 2).max(0);
    let rough = (analysis.total_senators as i32
        - analysis.likely_oppose_count as i32
        - analysis.lean_oppose_count as i32
        - blocker_penalty
        + pivot_bonus)
        .max(0) as usize;
    rough.min(analysis.total_senators)
}

fn derive_confidence(
    predicted_action: &FloorAction,
    analysis: &SenateAnalysis,
    support_margin_estimate: i32,
    cloture_gap_estimate: i32,
) -> f32 {
    let support_signal = clamp_unit((support_margin_estimate.abs() as f32) / 12.0);
    let cloture_signal = clamp_unit((cloture_gap_estimate.abs() as f32) / 12.0);
    let ambiguity_penalty =
        if analysis.simple_majority_viable != analysis.cloture_viable {
            0.18
        } else {
            0.0
        } + clamp_unit(analysis.undecided_count as f32 / analysis.total_senators as f32) * 0.18
            + (1.0 - analysis.coalition_stability) * 0.18;

    let base = match predicted_action {
        FloorAction::LikelyFinalPassage => 0.54 + support_signal * 0.20 + cloture_signal * 0.16,
        FloorAction::LikelyClotureFailure | FloorAction::LikelyProceduralBlock => {
            0.52 + cloture_signal * 0.22 + analysis.filibuster_risk * 0.14
        }
        FloorAction::LikelyAdvanceToDebate => {
            0.48 + support_signal * 0.14 + analysis.coalition_stability * 0.14
        }
        FloorAction::LikelyNegotiation | FloorAction::LikelyAmendmentFight => {
            0.42 + clamp_unit(analysis.pivotal_senators.len() as f32 / 8.0) * 0.14
        }
        FloorAction::LikelyClotureVote => 0.46 + analysis.filibuster_risk * 0.08,
        FloorAction::LikelyStall => 0.50 + (1.0 - analysis.coalition_stability) * 0.16,
    };

    clamp_unit(base - ambiguity_penalty)
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
                0.20
            }
        })
        .unwrap_or(0.10)
}

fn clamp_unit(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }

    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use crate::{
        FloorAction, PivotSummary, SenateAnalysis, SenatorSignalSummary,
        model::{
            dynamic_state::PublicPosition,
            legislative_context::ProceduralStage,
            senator_stance::{ProceduralPosture, StanceLabel},
        },
    };

    use super::assess_floor_action;

    fn example_object() -> crate::LegislativeObject {
        crate::LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Synthetic Bill".to_string(),
            object_type: crate::LegislativeObjectType::Bill,
            policy_domain: crate::PolicyDomain::EnergyClimate,
            summary: "Synthetic bill".to_string(),
            text_embedding_placeholder: None,
            sponsor: None,
            cosponsors: vec![],
            origin_chamber: crate::Chamber::Senate,
            introduced_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            current_version_label: None,
            budgetary_impact: crate::BudgetaryImpact::Moderate,
            salience: 0.7,
            controversy: 0.6,
        }
    }

    fn context(stage: ProceduralStage) -> crate::LegislativeContext {
        crate::LegislativeContext {
            congress_number: 119,
            session: crate::CongressionalSession::First,
            current_chamber: crate::Chamber::Senate,
            procedural_stage: stage,
            majority_party: crate::Party::Democrat,
            minority_party: crate::Party::Republican,
            president_party: crate::Party::Democrat,
            days_until_election: Some(100),
            days_until_deadline: Some(10),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.8,
            media_attention: 0.7,
        }
    }

    fn low_pressure_context(stage: ProceduralStage) -> crate::LegislativeContext {
        crate::LegislativeContext {
            congress_number: 119,
            session: crate::CongressionalSession::First,
            current_chamber: crate::Chamber::Senate,
            procedural_stage: stage,
            majority_party: crate::Party::Democrat,
            minority_party: crate::Party::Republican,
            president_party: crate::Party::Democrat,
            days_until_election: Some(220),
            days_until_deadline: Some(75),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.25,
            media_attention: 0.20,
        }
    }

    fn analysis(
        simple_majority_viable: bool,
        cloture_viable: bool,
        coalition_stability: f32,
        filibuster_risk: f32,
        undecided_count: usize,
        blockers: usize,
        pivots: usize,
    ) -> SenateAnalysis {
        let likely_support_count = 35;
        let lean_support_count = 18;
        let lean_oppose_count = 12;
        let likely_oppose_count =
            100 - likely_support_count - lean_support_count - undecided_count - lean_oppose_count;

        SenateAnalysis {
            object_id: "obj_001".to_string(),
            procedural_stage: ProceduralStage::ClotureFiled,
            total_senators: 100,
            likely_support_count,
            lean_support_count,
            undecided_count,
            lean_oppose_count,
            likely_oppose_count,
            expected_present_count: 98,
            simple_majority_viable,
            cloture_viable,
            filibuster_risk,
            coalition_stability,
            pivotal_senators: (0..pivots)
                .map(|idx| PivotSummary {
                    senator_id: format!("sen_{idx:03}"),
                    stance_label: StanceLabel::Undecided,
                    procedural_posture: ProceduralPosture::Unclear,
                    substantive_support: 0.55,
                    procedural_support: 0.52,
                    negotiability: 0.60,
                    reason: "pivot".to_string(),
                })
                .collect(),
            likely_defectors: vec![],
            likely_blockers: (0..blockers)
                .map(|idx| SenatorSignalSummary {
                    senator_id: format!("blk_{idx:03}"),
                    public_position: PublicPosition::Oppose,
                    defection_probability: 0.10,
                    rigidity: 0.90,
                    reason: "blocker".to_string(),
                })
                .collect(),
            top_findings: vec!["synthetic".to_string()],
        }
    }

    #[test]
    fn predicts_stall_when_no_majority_and_low_stability() {
        let assessment = assess_floor_action(
            &example_object(),
            &low_pressure_context(ProceduralStage::Debate),
            &analysis(false, false, 0.22, 0.42, 4, 1, 2),
        )
        .unwrap();

        assert_eq!(assessment.predicted_action, FloorAction::LikelyStall);
    }

    #[test]
    fn predicts_cloture_failure_when_majority_exists_but_cloture_does_not() {
        let assessment = assess_floor_action(
            &example_object(),
            &context(ProceduralStage::ClotureFiled),
            &analysis(true, false, 0.42, 0.82, 6, 5, 4),
        )
        .unwrap();

        assert_eq!(
            assessment.predicted_action,
            FloorAction::LikelyClotureFailure
        );
    }

    #[test]
    fn predicts_final_passage_when_majority_and_cloture_are_strong() {
        let assessment = assess_floor_action(
            &example_object(),
            &context(ProceduralStage::FinalPassage),
            &analysis(true, true, 0.76, 0.18, 2, 0, 2),
        )
        .unwrap();

        assert_eq!(assessment.predicted_action, FloorAction::LikelyFinalPassage);
    }

    #[test]
    fn confidence_stays_in_unit_interval() {
        let assessment = assess_floor_action(
            &example_object(),
            &context(ProceduralStage::ClotureVote),
            &analysis(true, false, 0.48, 0.68, 5, 3, 5),
        )
        .unwrap();

        assert!((0.0..=1.0).contains(&assessment.confidence));
    }
}
