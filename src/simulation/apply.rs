use crate::{
    error::SenateSimError,
    model::{
        legislative_context::ProceduralStage, senate_event::SenateEvent,
        simulation_state::SimulationState,
    },
};

pub fn apply_event(
    state: &SimulationState,
    event: &SenateEvent,
) -> Result<SimulationState, SenateSimError> {
    state.validate()?;

    let mut next_state = state.clone();
    next_state.step_index += 1;
    next_state.last_event = Some(event.clone());
    next_state.days_elapsed += 1;
    tick_deadlines(&mut next_state);

    match event {
        SenateEvent::NoMeaningfulMovement => {
            next_state.consecutive_no_movement += 1;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, -0.05);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, -0.04);
        }
        SenateEvent::LeadershipSignalsAction => {
            reset_inactivity(&mut next_state);
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.12);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.06);
        }
        SenateEvent::MotionToProceedAttempted => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::MotionToProceed;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.08);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.08);
        }
        SenateEvent::DebateBegins => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::Debate;
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.12);
        }
        SenateEvent::AmendmentFightBegins => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::AmendmentPending;
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.14);
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.04);
        }
        SenateEvent::ClotureFiled => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::ClotureFiled;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.10);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.10);
            next_state.cloture_attempts += 1;
        }
        SenateEvent::ClotureVoteScheduled => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::ClotureVote;
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.10);
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.04);
            next_state.cloture_attempts += 1;
            advance_deadline(&mut next_state, 2);
        }
        SenateEvent::ClotureInvoked => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::FinalPassage;
            next_state.context.under_unanimous_consent = false;
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.08);
        }
        SenateEvent::ClotureFails => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::Stalled;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, -0.10);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.04);
            next_state.cloture_attempts += 1;
        }
        SenateEvent::FinalPassageScheduled => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::FinalPassage;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.08);
        }
        SenateEvent::FinalPassageSucceeds | SenateEvent::FinalPassageFails => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::FinalPassage;
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.06);
        }
        SenateEvent::NegotiationIntensifies => {
            reset_inactivity(&mut next_state);
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, 0.05);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.06);
            if matches!(
                next_state.context.procedural_stage,
                ProceduralStage::Stalled
            ) {
                next_state.context.procedural_stage = ProceduralStage::OnCalendar;
            }
        }
        SenateEvent::ProceduralBlock => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::Stalled;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, -0.04);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, 0.02);
        }
        SenateEvent::ReturnedToCalendar => {
            reset_inactivity(&mut next_state);
            next_state.context.procedural_stage = ProceduralStage::OnCalendar;
            next_state.context.leadership_priority =
                adjust_unit(next_state.context.leadership_priority, -0.12);
            next_state.context.media_attention =
                adjust_unit(next_state.context.media_attention, -0.08);
        }
        SenateEvent::Other(_) => {
            reset_inactivity(&mut next_state);
        }
    }

    next_state.validate()?;
    Ok(next_state)
}

fn tick_deadlines(state: &mut SimulationState) {
    if let Some(days) = state.context.days_until_deadline.as_mut() {
        *days = (*days - 1).max(0);
    }
    if let Some(days) = state.context.days_until_election.as_mut() {
        *days = (*days - 1).max(0);
    }
}

fn advance_deadline(state: &mut SimulationState, extra_days: i32) {
    if let Some(days) = state.context.days_until_deadline.as_mut() {
        *days = (*days - extra_days).max(0);
    }
}

fn adjust_unit(value: f32, delta: f32) -> f32 {
    (value + delta).clamp(0.0, 1.0)
}

fn reset_inactivity(state: &mut SimulationState) {
    state.consecutive_no_movement = 0;
}

#[cfg(test)]
mod tests {
    use crate::{
        BudgetaryImpact, Chamber, CongressionalSession, LegislativeContext, LegislativeObject,
        LegislativeObjectType, Party, PolicyDomain, ProceduralStage, SenateEvent, SimulationState,
        build_synthetic_senate, simulation::apply::apply_event,
    };

    fn example_object() -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Synthetic Bill".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: PolicyDomain::EnergyClimate,
            summary: "Synthetic".to_string(),
            text_embedding_placeholder: None,
            sponsor: None,
            cosponsors: vec![],
            origin_chamber: Chamber::Senate,
            introduced_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            current_version_label: None,
            budgetary_impact: BudgetaryImpact::Moderate,
            salience: 0.7,
            controversy: 0.6,
        }
    }

    fn example_context() -> LegislativeContext {
        LegislativeContext {
            congress_number: 119,
            session: CongressionalSession::First,
            current_chamber: Chamber::Senate,
            procedural_stage: ProceduralStage::Debate,
            majority_party: Party::Democrat,
            minority_party: Party::Republican,
            president_party: Party::Democrat,
            days_until_election: Some(120),
            days_until_deadline: Some(18),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.7,
            media_attention: 0.5,
        }
    }

    fn state() -> SimulationState {
        SimulationState {
            legislative_object: example_object(),
            context: example_context(),
            roster: build_synthetic_senate(),
            step_index: 0,
            last_event: None,
            consecutive_no_movement: 0,
            days_elapsed: 0,
            cloture_attempts: 0,
        }
    }

    #[test]
    fn apply_event_updates_cloture_stage() {
        let next = apply_event(&state(), &SenateEvent::ClotureFiled).unwrap();
        assert_eq!(next.context.procedural_stage, ProceduralStage::ClotureFiled);
    }

    #[test]
    fn apply_event_invoked_moves_to_final_passage() {
        let next = apply_event(&state(), &SenateEvent::ClotureInvoked).unwrap();
        assert_eq!(next.context.procedural_stage, ProceduralStage::FinalPassage);
    }

    #[test]
    fn apply_event_block_moves_to_stalled() {
        let next = apply_event(&state(), &SenateEvent::ProceduralBlock).unwrap();
        assert_eq!(next.context.procedural_stage, ProceduralStage::Stalled);
    }
}
