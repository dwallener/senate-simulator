use crate::{
    error::SenateSimError,
    model::{
        legislative::LegislativeObject,
        legislative_context::{LegislativeContext, ProceduralStage},
        next_event_prediction::{EventScore, NextEventPrediction},
        senate_analysis::SenateAnalysis,
        senate_event::SenateEvent,
    },
};

pub fn predict_next_event(
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    analysis: &SenateAnalysis,
) -> Result<NextEventPrediction, SenateSimError> {
    legislative_object.validate()?;
    context.validate()?;
    analysis.validate()?;

    if analysis.object_id != legislative_object.object_id {
        return Err(SenateSimError::Validation {
            field: "next_event_prediction.object_id",
            message: "analysis object_id must match legislative object".to_string(),
        });
    }

    let candidates = candidate_events_for_stage(&context.procedural_stage);
    let scored = score_candidates(&candidates, legislative_object, context, analysis);
    let (predicted_event, top_score) = scored
        .first()
        .map(|event_score| (event_score.event.clone(), event_score.score))
        .ok_or_else(|| SenateSimError::Validation {
            field: "next_event_prediction.candidates",
            message: "must contain at least one candidate event".to_string(),
        })?;
    let second_score = scored
        .get(1)
        .map(|event_score| event_score.score)
        .unwrap_or(0.0);
    let confidence = derive_confidence(top_score, second_score, analysis);
    let top_reasons = build_top_reasons(&predicted_event, context, analysis);
    let alternative_events = scored.into_iter().skip(1).take(4).collect::<Vec<_>>();

    let prediction = NextEventPrediction {
        object_id: legislative_object.object_id.clone(),
        current_stage: context.procedural_stage.clone(),
        predicted_event,
        confidence,
        alternative_events,
        top_reasons,
        simple_majority_viable: analysis.simple_majority_viable,
        cloture_viable: analysis.cloture_viable,
        coalition_stability: analysis.coalition_stability,
        filibuster_risk: analysis.filibuster_risk,
    };

    prediction.validate()?;
    Ok(prediction)
}

pub fn candidate_events_for_stage(stage: &ProceduralStage) -> Vec<SenateEvent> {
    match stage {
        ProceduralStage::Introduced
        | ProceduralStage::InCommittee
        | ProceduralStage::Reported
        | ProceduralStage::OnCalendar => vec![
            SenateEvent::NoMeaningfulMovement,
            SenateEvent::LeadershipSignalsAction,
            SenateEvent::NegotiationIntensifies,
            SenateEvent::MotionToProceedAttempted,
        ],
        ProceduralStage::MotionToProceed => vec![
            SenateEvent::MotionToProceedAttempted,
            SenateEvent::DebateBegins,
            SenateEvent::ProceduralBlock,
            SenateEvent::NoMeaningfulMovement,
            SenateEvent::NegotiationIntensifies,
        ],
        ProceduralStage::Debate => vec![
            SenateEvent::AmendmentFightBegins,
            SenateEvent::ClotureFiled,
            SenateEvent::NegotiationIntensifies,
            SenateEvent::ProceduralBlock,
            SenateEvent::NoMeaningfulMovement,
        ],
        ProceduralStage::AmendmentPending => vec![
            SenateEvent::AmendmentFightBegins,
            SenateEvent::NegotiationIntensifies,
            SenateEvent::ClotureFiled,
            SenateEvent::ProceduralBlock,
        ],
        ProceduralStage::ClotureFiled => vec![
            SenateEvent::ClotureVoteScheduled,
            SenateEvent::ClotureInvoked,
            SenateEvent::ClotureFails,
            SenateEvent::NegotiationIntensifies,
        ],
        ProceduralStage::ClotureVote => vec![
            SenateEvent::ClotureInvoked,
            SenateEvent::ClotureFails,
            SenateEvent::NegotiationIntensifies,
        ],
        ProceduralStage::FinalPassage => vec![
            SenateEvent::FinalPassageScheduled,
            SenateEvent::FinalPassageSucceeds,
            SenateEvent::FinalPassageFails,
            SenateEvent::NegotiationIntensifies,
        ],
        ProceduralStage::Stalled => vec![
            SenateEvent::NoMeaningfulMovement,
            SenateEvent::LeadershipSignalsAction,
            SenateEvent::NegotiationIntensifies,
            SenateEvent::ReturnedToCalendar,
        ],
        ProceduralStage::Conference => vec![
            SenateEvent::NegotiationIntensifies,
            SenateEvent::LeadershipSignalsAction,
            SenateEvent::NoMeaningfulMovement,
            SenateEvent::ReturnedToCalendar,
        ],
        ProceduralStage::Other(_) => vec![
            SenateEvent::NegotiationIntensifies,
            SenateEvent::LeadershipSignalsAction,
            SenateEvent::NoMeaningfulMovement,
        ],
    }
}

fn score_candidates(
    candidates: &[SenateEvent],
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    analysis: &SenateAnalysis,
) -> Vec<EventScore> {
    let mut scored = candidates
        .iter()
        .map(|event| EventScore {
            event: event.clone(),
            score: event_score(event, legislative_object, context, analysis),
            reason: event_reason(event, context, analysis),
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    scored
}

fn event_score(
    event: &SenateEvent,
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    analysis: &SenateAnalysis,
) -> f32 {
    let urgency = urgency(context);
    let pivots = ratio(analysis.pivotal_senators.len(), analysis.total_senators);
    let blockers = ratio(analysis.likely_blockers.len(), analysis.total_senators);
    let undecideds = ratio(analysis.undecided_count, analysis.total_senators);
    let salience = legislative_object.salience;
    let leadership = context.leadership_priority;

    let raw = match event {
        SenateEvent::NoMeaningfulMovement => {
            0.20 + (1.0 - leadership) * 0.28
                + (1.0 - urgency) * 0.20
                + (!analysis.simple_majority_viable as i32 as f32) * 0.12
                + (1.0 - analysis.coalition_stability) * 0.12
                + early_or_stalled_bonus(&context.procedural_stage) * 0.18
        }
        SenateEvent::LeadershipSignalsAction => {
            0.18 + leadership * 0.30
                + salience * 0.12
                + urgency * 0.10
                + (!analysis.simple_majority_viable as i32 as f32) * 0.10
                + undecideds * 0.10
        }
        SenateEvent::MotionToProceedAttempted => {
            0.18 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::MotionToProceed],
            ) * 0.18
                + leadership * 0.20
                + analysis.coalition_stability * 0.10
                + (1.0 - analysis.filibuster_risk) * 0.14
                + if analysis.simple_majority_viable {
                    0.16
                } else {
                    0.0
                }
        }
        SenateEvent::DebateBegins => {
            0.16 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::MotionToProceed],
            ) * 0.12
                + if analysis.simple_majority_viable {
                    0.22
                } else {
                    0.0
                }
                + if analysis.cloture_viable { 0.18 } else { 0.0 }
                + analysis.coalition_stability * 0.16
                - blockers * 0.14
        }
        SenateEvent::AmendmentFightBegins => {
            0.16 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::Debate, ProceduralStage::AmendmentPending],
            ) * 0.16
                + pivots * 0.20
                + (1.0 - analysis.coalition_stability) * 0.16
                + undecideds * 0.12
                + if !analysis.cloture_viable { 0.08 } else { 0.0 }
        }
        SenateEvent::ClotureFiled => {
            0.18 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::Debate, ProceduralStage::AmendmentPending],
            ) * 0.20
                + leadership * 0.24
                + salience * 0.10
                + blockers * 0.10
                + if !analysis.cloture_viable { 0.08 } else { 0.0 }
        }
        SenateEvent::ClotureVoteScheduled => {
            0.24 + stage_match(&context.procedural_stage, &[ProceduralStage::ClotureFiled]) * 0.24
                + leadership * 0.22
                + urgency * 0.14
                + if analysis.cloture_viable { 0.08 } else { -0.12 }
        }
        SenateEvent::ClotureInvoked => {
            0.12 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::ClotureFiled, ProceduralStage::ClotureVote],
            ) * 0.18
                + if analysis.cloture_viable { 0.30 } else { 0.0 }
                + analysis.coalition_stability * 0.18
                + (1.0 - analysis.filibuster_risk) * 0.14
        }
        SenateEvent::ClotureFails => {
            0.12 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::ClotureFiled, ProceduralStage::ClotureVote],
            ) * 0.16
                + if !analysis.cloture_viable { 0.28 } else { 0.0 }
                + analysis.filibuster_risk * 0.22
                + blockers * 0.14
        }
        SenateEvent::FinalPassageScheduled => {
            0.12 + stage_match(&context.procedural_stage, &[ProceduralStage::FinalPassage]) * 0.20
                + leadership * 0.18
                + if analysis.simple_majority_viable {
                    0.18
                } else {
                    0.0
                }
                + if analysis.cloture_viable { 0.12 } else { 0.0 }
                + urgency * 0.08
        }
        SenateEvent::FinalPassageSucceeds => {
            0.12 + stage_match(&context.procedural_stage, &[ProceduralStage::FinalPassage]) * 0.20
                + if analysis.simple_majority_viable {
                    0.28
                } else {
                    0.0
                }
                + analysis.coalition_stability * 0.20
                + (1.0 - blockers) * 0.08
        }
        SenateEvent::FinalPassageFails => {
            0.10 + stage_match(&context.procedural_stage, &[ProceduralStage::FinalPassage]) * 0.20
                + if !analysis.simple_majority_viable {
                    0.26
                } else {
                    0.0
                }
                + (1.0 - analysis.coalition_stability) * 0.18
                + blockers * 0.12
        }
        SenateEvent::NegotiationIntensifies => {
            0.22 + closeness(analysis) * 0.40
                + pivots * 0.28
                + urgency * 0.18
                + undecideds * 0.16
                + if !analysis.simple_majority_viable || !analysis.cloture_viable {
                    0.18
                } else {
                    0.0
                }
        }
        SenateEvent::ProceduralBlock => {
            0.16 + blockers * 0.24
                + analysis.filibuster_risk * 0.24
                + if !analysis.cloture_viable { 0.18 } else { 0.0 }
                + low_process_viability_penalty(analysis) * 0.12
        }
        SenateEvent::ReturnedToCalendar => {
            0.08 + stage_match(
                &context.procedural_stage,
                &[ProceduralStage::Stalled, ProceduralStage::FinalPassage],
            ) * 0.20
                + (1.0 - leadership) * 0.18
                + (1.0 - analysis.coalition_stability) * 0.16
                + if !analysis.simple_majority_viable && !analysis.cloture_viable {
                    0.16
                } else {
                    0.0
                }
        }
        SenateEvent::Other(_) => 0.0,
    };

    clamp_unit(raw)
}

fn event_reason(
    event: &SenateEvent,
    _context: &LegislativeContext,
    _analysis: &SenateAnalysis,
) -> String {
    match event {
        SenateEvent::NoMeaningfulMovement => {
            "weak coalition pressure and low immediate urgency favor inaction".to_string()
        }
        SenateEvent::LeadershipSignalsAction => {
            "leadership pressure is building even without a clean formal path".to_string()
        }
        SenateEvent::MotionToProceedAttempted => {
            "leadership has enough incentive to test floor access".to_string()
        }
        SenateEvent::DebateBegins => {
            "procedural conditions are good enough to move into floor debate".to_string()
        }
        SenateEvent::AmendmentFightBegins => {
            "fragile coalition and many pivots make amendment conflict likely".to_string()
        }
        SenateEvent::ClotureFiled => {
            "leadership pressure and blocker presence favor forcing the process".to_string()
        }
        SenateEvent::ClotureVoteScheduled => {
            "with cloture already filed, the next meaningful move is the vote itself".to_string()
        }
        SenateEvent::ClotureInvoked => {
            "stable procedural support makes a successful cloture vote plausible".to_string()
        }
        SenateEvent::ClotureFails => {
            "blockers and filibuster risk outweigh the cloture path".to_string()
        }
        SenateEvent::FinalPassageScheduled => {
            "late-stage support and leadership attention point toward scheduling passage"
                .to_string()
        }
        SenateEvent::FinalPassageSucceeds => {
            "late-stage majority support and stability favor passage".to_string()
        }
        SenateEvent::FinalPassageFails => {
            "late-stage coalition weakness makes defeat plausible".to_string()
        }
        SenateEvent::NegotiationIntensifies => {
            "near-threshold numbers and pivotal senators keep bargaining active".to_string()
        }
        SenateEvent::ProceduralBlock => {
            "rigid blockers and procedural weakness point toward obstruction".to_string()
        }
        SenateEvent::ReturnedToCalendar => {
            "low momentum and weak coalition coherence favor pulling the bill back".to_string()
        }
        SenateEvent::Other(value) => format!("custom event score for {value}"),
    }
}

fn build_top_reasons(
    predicted_event: &SenateEvent,
    context: &LegislativeContext,
    analysis: &SenateAnalysis,
) -> Vec<String> {
    let mut reasons = vec![event_reason(predicted_event, context, analysis)];

    reasons.push(if analysis.simple_majority_viable {
        "simple-majority math currently favors action".to_string()
    } else {
        "simple-majority support is not yet locked in".to_string()
    });

    reasons.push(if analysis.cloture_viable {
        "procedural coalition is strong enough to keep floor options open".to_string()
    } else {
        "procedural coalition is weak enough to constrain floor movement".to_string()
    });

    if analysis.filibuster_risk >= 0.60 {
        reasons
            .push("filibuster risk remains elevated because blockers are meaningful".to_string());
    }
    if context.leadership_priority >= 0.70 {
        reasons.push(
            "high leadership priority raises the odds of a consequential next move".to_string(),
        );
    }
    if urgency(context) >= 0.60 {
        reasons.push("deadline and media pressure reduce the odds of passive drift".to_string());
    }

    reasons.truncate(6);
    reasons
}

fn derive_confidence(top_score: f32, second_score: f32, analysis: &SenateAnalysis) -> f32 {
    let score_gap = clamp_unit((top_score - second_score).max(0.0) * 1.6);
    let clarity = if analysis.simple_majority_viable == analysis.cloture_viable {
        0.14
    } else {
        0.04
    };
    let stability_bonus = analysis.coalition_stability * 0.16;
    let ambiguity_penalty = ratio(analysis.undecided_count, analysis.total_senators) * 0.18;

    clamp_unit(0.36 + score_gap + clarity + stability_bonus - ambiguity_penalty)
}

fn urgency(context: &LegislativeContext) -> f32 {
    let deadline = context
        .days_until_deadline
        .map(|days| {
            if days <= 7 {
                1.0
            } else if days <= 21 {
                0.78
            } else if days <= 45 {
                0.48
            } else {
                0.18
            }
        })
        .unwrap_or(0.12);

    clamp_unit(deadline * 0.6 + context.media_attention * 0.4)
}

fn closeness(analysis: &SenateAnalysis) -> f32 {
    let pivot_pressure = ratio(analysis.pivotal_senators.len(), analysis.total_senators);
    let undecided_pressure = ratio(analysis.undecided_count, analysis.total_senators);
    clamp_unit(
        (pivot_pressure * 0.6)
            + (undecided_pressure * 0.4)
            + (1.0 - analysis.coalition_stability) * 0.2,
    )
}

fn low_process_viability_penalty(analysis: &SenateAnalysis) -> f32 {
    if analysis.cloture_viable {
        0.0
    } else {
        1.0 - analysis.coalition_stability
    }
}

fn early_or_stalled_bonus(stage: &ProceduralStage) -> f32 {
    match stage {
        ProceduralStage::Introduced
        | ProceduralStage::InCommittee
        | ProceduralStage::Reported
        | ProceduralStage::OnCalendar
        | ProceduralStage::Stalled => 1.0,
        _ => 0.0,
    }
}

fn stage_match(stage: &ProceduralStage, matching: &[ProceduralStage]) -> f32 {
    if matching.iter().any(|candidate| stage == candidate) {
        1.0
    } else {
        0.0
    }
}

fn ratio(count: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        count as f32 / total as f32
    }
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
        EventScore, NextEventPrediction, PivotSummary, SenateAnalysis, SenateEvent,
        SenatorSignalSummary,
        model::{
            dynamic_state::PublicPosition,
            legislative_context::ProceduralStage,
            senator_stance::{ProceduralPosture, StanceLabel},
        },
    };

    use super::{candidate_events_for_stage, predict_next_event};

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

    fn context(
        stage: ProceduralStage,
        leadership_priority: f32,
        media_attention: f32,
        days_until_deadline: i32,
    ) -> crate::LegislativeContext {
        crate::LegislativeContext {
            congress_number: 119,
            session: crate::CongressionalSession::First,
            current_chamber: crate::Chamber::Senate,
            procedural_stage: stage,
            majority_party: crate::Party::Democrat,
            minority_party: crate::Party::Republican,
            president_party: crate::Party::Democrat,
            days_until_election: Some(120),
            days_until_deadline: Some(days_until_deadline),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority,
            media_attention,
        }
    }

    fn analysis(
        stage: ProceduralStage,
        simple_majority_viable: bool,
        cloture_viable: bool,
        coalition_stability: f32,
        filibuster_risk: f32,
        undecided_count: usize,
        blockers: usize,
        pivots: usize,
    ) -> SenateAnalysis {
        let likely_support_count = if simple_majority_viable { 34 } else { 22 };
        let lean_support_count = if simple_majority_viable { 20 } else { 16 };
        let lean_oppose_count = 12;
        let likely_oppose_count =
            100 - likely_support_count - lean_support_count - undecided_count - lean_oppose_count;

        SenateAnalysis {
            object_id: "obj_001".to_string(),
            procedural_stage: stage,
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
                    negotiability: 0.62,
                    reason: "pivot".to_string(),
                })
                .collect(),
            likely_defectors: vec![],
            likely_blockers: (0..blockers)
                .map(|idx| SenatorSignalSummary {
                    senator_id: format!("blk_{idx:03}"),
                    public_position: PublicPosition::Oppose,
                    defection_probability: 0.08,
                    rigidity: 0.90,
                    reason: "blocker".to_string(),
                })
                .collect(),
            top_findings: vec!["synthetic".to_string()],
        }
    }

    #[test]
    fn stage_candidate_mapping_is_explicit() {
        let cloture = candidate_events_for_stage(&ProceduralStage::ClotureFiled);
        assert!(cloture.contains(&SenateEvent::ClotureVoteScheduled));
        assert!(cloture.contains(&SenateEvent::ClotureInvoked));
        assert!(cloture.contains(&SenateEvent::ClotureFails));

        let debate = candidate_events_for_stage(&ProceduralStage::Debate);
        assert!(debate.contains(&SenateEvent::ClotureFiled));
        assert!(debate.contains(&SenateEvent::AmendmentFightBegins));
        assert!(debate.contains(&SenateEvent::NegotiationIntensifies));
    }

    #[test]
    fn low_pressure_weak_coalition_favors_no_movement() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::Introduced, 0.18, 0.12, 90),
            &analysis(
                ProceduralStage::Introduced,
                false,
                false,
                0.24,
                0.44,
                8,
                1,
                2,
            ),
        )
        .unwrap();

        assert_eq!(
            prediction.predicted_event,
            SenateEvent::NoMeaningfulMovement
        );
    }

    #[test]
    fn high_risk_cloture_stage_favors_failure_or_block() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::ClotureFiled, 0.82, 0.72, 12),
            &analysis(
                ProceduralStage::ClotureFiled,
                true,
                false,
                0.34,
                0.86,
                7,
                6,
                4,
            ),
        )
        .unwrap();

        assert!(matches!(
            prediction.predicted_event,
            SenateEvent::ClotureFails | SenateEvent::ProceduralBlock
        ));
    }

    #[test]
    fn viable_stable_cloture_stage_favors_vote_or_invocation() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::ClotureVote, 0.84, 0.70, 10),
            &analysis(
                ProceduralStage::ClotureVote,
                true,
                true,
                0.74,
                0.18,
                3,
                1,
                2,
            ),
        )
        .unwrap();

        assert!(matches!(
            prediction.predicted_event,
            SenateEvent::ClotureInvoked | SenateEvent::ClotureVoteScheduled
        ));
    }

    #[test]
    fn near_threshold_case_favors_negotiation() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::Debate, 0.76, 0.68, 14),
            &analysis(ProceduralStage::Debate, false, false, 0.42, 0.58, 10, 2, 7),
        )
        .unwrap();

        let negotiation_scores_highly = prediction.predicted_event
            == SenateEvent::NegotiationIntensifies
            || prediction.alternative_events.iter().any(|event| {
                event.event == SenateEvent::NegotiationIntensifies && event.score >= 0.60
            });

        assert!(negotiation_scores_highly);
    }

    #[test]
    fn final_passage_stage_can_predict_success() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::FinalPassage, 0.82, 0.66, 8),
            &analysis(
                ProceduralStage::FinalPassage,
                true,
                true,
                0.76,
                0.16,
                2,
                0,
                2,
            ),
        )
        .unwrap();

        assert_eq!(
            prediction.predicted_event,
            SenateEvent::FinalPassageSucceeds
        );
    }

    #[test]
    fn confidence_stays_in_unit_interval() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::ClotureFiled, 0.82, 0.66, 12),
            &analysis(
                ProceduralStage::ClotureFiled,
                true,
                false,
                0.42,
                0.72,
                6,
                4,
                5,
            ),
        )
        .unwrap();

        assert!((0.0..=1.0).contains(&prediction.confidence));
    }

    #[test]
    fn alternatives_are_sorted_descending() {
        let prediction = predict_next_event(
            &example_object(),
            &context(ProceduralStage::Debate, 0.74, 0.62, 18),
            &analysis(ProceduralStage::Debate, false, false, 0.46, 0.52, 9, 2, 6),
        )
        .unwrap();

        assert!(
            prediction
                .alternative_events
                .windows(2)
                .all(|pair| pair[0].score >= pair[1].score)
        );
    }

    #[test]
    fn prediction_validation_accepts_sorted_alternatives() {
        let prediction = NextEventPrediction {
            object_id: "obj_001".to_string(),
            current_stage: ProceduralStage::Debate,
            predicted_event: SenateEvent::NegotiationIntensifies,
            confidence: 0.61,
            alternative_events: vec![
                EventScore {
                    event: SenateEvent::ClotureFiled,
                    score: 0.53,
                    reason: "alt".to_string(),
                },
                EventScore {
                    event: SenateEvent::ProceduralBlock,
                    score: 0.40,
                    reason: "alt".to_string(),
                },
            ],
            top_reasons: vec!["reason".to_string()],
            simple_majority_viable: false,
            cloture_viable: false,
            coalition_stability: 0.44,
            filibuster_risk: 0.55,
        };

        assert!(prediction.validate().is_ok());
    }
}
