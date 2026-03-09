use std::collections::HashMap;

use crate::{
    analysis::chamber::analyze_chamber,
    analysis::transition::predict_next_event,
    derive::{stance::derive_stance_with_mode, StanceDerivationMode},
    error::SenateSimError,
    model::{
        senate_event::SenateEvent,
        simulation_state::SimulationState,
        simulation_step::{SimulationStep, StepAnalysisSummary},
        trajectory_result::{TerminationReason, TrajectoryResult},
    },
    simulation::apply::apply_event,
};

pub fn rollout(
    initial_state: &SimulationState,
    max_steps: usize,
) -> Result<TrajectoryResult, SenateSimError> {
    rollout_with_mode(initial_state, max_steps, StanceDerivationMode::FeatureDriven)
}

pub fn rollout_with_mode(
    initial_state: &SimulationState,
    max_steps: usize,
    mode: StanceDerivationMode,
) -> Result<TrajectoryResult, SenateSimError> {
    initial_state.validate()?;

    let mut current_state = initial_state.clone();
    let mut steps = Vec::new();
    let mut seen_pairs: HashMap<(String, String), usize> = HashMap::new();

    for step_index in 0..max_steps {
        let stances = current_state
            .roster
            .iter()
            .map(|senator| {
                derive_stance_with_mode(
                    senator,
                    &current_state.legislative_object,
                    &current_state.context,
                    mode,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let analysis = analyze_chamber(
            &current_state.legislative_object,
            &current_state.context,
            &stances,
        )?;
        let prediction = predict_next_event(
            &current_state.legislative_object,
            &current_state.context,
            &analysis,
        )?;

        let event = prediction.predicted_event.clone();
        let pair = (
            format!("{:?}", current_state.context.procedural_stage),
            format!("{event}"),
        );
        let pair_count = seen_pairs.entry(pair).or_insert(0);
        *pair_count += 1;

        steps.push(SimulationStep {
            step_index,
            starting_stage: current_state.context.procedural_stage.clone(),
            predicted_event: event.clone(),
            confidence: prediction.confidence,
            analysis_summary: StepAnalysisSummary {
                likely_support_count: analysis.likely_support_count,
                lean_support_count: analysis.lean_support_count,
                undecided_count: analysis.undecided_count,
                likely_oppose_count: analysis.likely_oppose_count,
                simple_majority_viable: analysis.simple_majority_viable,
                cloture_viable: analysis.cloture_viable,
                coalition_stability: analysis.coalition_stability,
                filibuster_risk: analysis.filibuster_risk,
            },
            alternative_events: prediction.alternative_events.clone(),
            top_reasons: prediction.top_reasons.clone(),
        });

        let next_state = apply_event(&current_state, &event)?;

        if is_terminal_event(&event) {
            let result = TrajectoryResult {
                steps,
                final_state: next_state,
                terminated_reason: TerminationReason::ReachedTerminalEvent,
            };
            result.validate()?;
            return Ok(result);
        }

        if matches!(event, SenateEvent::NoMeaningfulMovement)
            && next_state.consecutive_no_movement >= 2
        {
            let result = TrajectoryResult {
                steps,
                final_state: next_state,
                terminated_reason: TerminationReason::NoMeaningfulFurtherMovement,
            };
            result.validate()?;
            return Ok(result);
        }

        if seen_pairs.values().any(|count| *count >= 2) {
            let result = TrajectoryResult {
                steps,
                final_state: next_state,
                terminated_reason: TerminationReason::LoopDetected,
            };
            result.validate()?;
            return Ok(result);
        }

        current_state = next_state;
    }

    let result = TrajectoryResult {
        steps,
        final_state: current_state,
        terminated_reason: TerminationReason::ReachedHorizon,
    };
    result.validate()?;
    Ok(result)
}

fn is_terminal_event(event: &SenateEvent) -> bool {
    matches!(
        event,
        SenateEvent::FinalPassageSucceeds | SenateEvent::FinalPassageFails
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        BudgetaryImpact, Chamber, CongressionalSession, LegislativeContext, LegislativeObject,
        LegislativeObjectType, Party, PolicyDomain, ProceduralStage, SenateEvent, SimulationState,
        StanceDerivationMode, TerminationReason, analyze_chamber, build_synthetic_senate,
        derive_stance_with_mode, rollout,
        simulation::apply::apply_event,
    };

    fn example_object() -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Synthetic Rollout Bill".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: PolicyDomain::EnergyClimate,
            summary: "Synthetic rollout bill".to_string(),
            text_embedding_placeholder: None,
            sponsor: None,
            cosponsors: vec![],
            origin_chamber: Chamber::Senate,
            introduced_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            current_version_label: None,
            budgetary_impact: BudgetaryImpact::Moderate,
            salience: 0.78,
            controversy: 0.63,
        }
    }

    fn context(
        stage: ProceduralStage,
        leadership: f32,
        media: f32,
        deadline: i32,
    ) -> LegislativeContext {
        LegislativeContext {
            congress_number: 119,
            session: CongressionalSession::First,
            current_chamber: Chamber::Senate,
            procedural_stage: stage,
            majority_party: Party::Democrat,
            minority_party: Party::Republican,
            president_party: Party::Democrat,
            days_until_election: Some(120),
            days_until_deadline: Some(deadline),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: leadership,
            media_attention: media,
        }
    }

    fn state(
        stage: ProceduralStage,
        leadership: f32,
        media: f32,
        deadline: i32,
    ) -> SimulationState {
        SimulationState {
            legislative_object: example_object(),
            context: context(stage, leadership, media, deadline),
            roster: build_synthetic_senate(),
            step_index: 0,
            last_event: None,
            consecutive_no_movement: 0,
            days_elapsed: 0,
            cloture_attempts: 0,
        }
    }

    #[test]
    fn rollout_executes() {
        let result = rollout(&state(ProceduralStage::ClotureFiled, 0.82, 0.67, 18), 4).unwrap();
        assert!(!result.steps.is_empty());
    }

    #[test]
    fn horizon_termination_is_reported() {
        let result = rollout(&state(ProceduralStage::Debate, 0.82, 0.67, 18), 2).unwrap();
        assert!(matches!(
            result.terminated_reason,
            TerminationReason::ReachedHorizon
                | TerminationReason::ReachedTerminalEvent
                | TerminationReason::LoopDetected
        ));
    }

    #[test]
    fn terminal_passage_stops_early() {
        let result = rollout(&state(ProceduralStage::FinalPassage, 0.95, 0.80, 4), 5).unwrap();
        assert!(matches!(
            result.terminated_reason,
            TerminationReason::ReachedTerminalEvent | TerminationReason::ReachedHorizon
        ));
    }

    #[test]
    fn repeated_inactivity_terminates_rollout() {
        let result = rollout(&state(ProceduralStage::Stalled, 0.10, 0.10, 80), 5).unwrap();
        assert!(matches!(
            result.terminated_reason,
            TerminationReason::NoMeaningfulFurtherMovement
                | TerminationReason::LoopDetected
                | TerminationReason::ReachedHorizon
        ));
    }

    #[test]
    fn rollout_does_not_repeat_same_event_indefinitely() {
        let result = rollout(&state(ProceduralStage::Debate, 0.82, 0.67, 18), 5).unwrap();
        let has_long_repeat = result.steps.windows(3).any(|window| {
            window
                .iter()
                .all(|step| step.predicted_event == window[0].predicted_event)
        });
        assert!(!has_long_repeat);
    }

    #[test]
    fn rederivation_changes_chamber_outputs_when_context_changes() {
        let state = state(ProceduralStage::Debate, 0.82, 0.40, 18);
        let stances_before = state
            .roster
            .iter()
            .map(|senator| {
                derive_stance_with_mode(
                    senator,
                    &state.legislative_object,
                    &state.context,
                    StanceDerivationMode::FeatureDriven,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let analysis_before =
            analyze_chamber(&state.legislative_object, &state.context, &stances_before).unwrap();

        let next_state = apply_event(&state, &SenateEvent::ClotureFiled).unwrap();
        let stances_after = next_state
            .roster
            .iter()
            .map(|senator| {
                derive_stance_with_mode(
                    senator,
                    &next_state.legislative_object,
                    &next_state.context,
                    StanceDerivationMode::FeatureDriven,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let analysis_after = analyze_chamber(
            &next_state.legislative_object,
            &next_state.context,
            &stances_after,
        )
        .unwrap();

        assert_ne!(
            analysis_before.procedural_stage,
            analysis_after.procedural_stage
        );
        assert!(
            stances_before
                .iter()
                .zip(stances_after.iter())
                .any(|(before, after)| before.procedural_support != after.procedural_support)
        );
    }
}
