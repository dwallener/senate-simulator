use crate::model::{
    normalized_records::{NormalizedActionCategory, NormalizedActionRecord},
    senate_event::SenateEvent,
};

pub fn is_consequential_action(action: &NormalizedActionRecord) -> bool {
    if matches!(
        action.category,
        NormalizedActionCategory::MotionToProceed
            | NormalizedActionCategory::Debate
            | NormalizedActionCategory::Amendment
            | NormalizedActionCategory::Cloture
            | NormalizedActionCategory::Passage
            | NormalizedActionCategory::Stall
    ) {
        return true;
    }

    let text = action.action_text.to_ascii_lowercase();
    let consequential_keywords = [
        "motion to proceed",
        "debate",
        "amendment",
        "cloture",
        "passed",
        "failed",
        "blocked",
        "returned to calendar",
        "stall",
    ];

    consequential_keywords
        .iter()
        .any(|keyword| text.contains(keyword))
}

pub fn align_action_to_senate_event(action: &NormalizedActionRecord) -> Option<SenateEvent> {
    let text = action.action_text.to_ascii_lowercase();
    match action.category {
        NormalizedActionCategory::MotionToProceed => Some(SenateEvent::MotionToProceedAttempted),
        NormalizedActionCategory::Debate => {
            if text.contains("amendment") {
                Some(SenateEvent::AmendmentFightBegins)
            } else {
                Some(SenateEvent::DebateBegins)
            }
        }
        NormalizedActionCategory::Amendment => Some(SenateEvent::AmendmentFightBegins),
        NormalizedActionCategory::Cloture => {
            if text.contains("filed") {
                Some(SenateEvent::ClotureFiled)
            } else if text.contains("scheduled") {
                Some(SenateEvent::ClotureVoteScheduled)
            } else if text.contains("invoked") || text.contains("agreed to") {
                Some(SenateEvent::ClotureInvoked)
            } else if text.contains("failed") || text.contains("rejected") {
                Some(SenateEvent::ClotureFails)
            } else {
                None
            }
        }
        NormalizedActionCategory::Passage => {
            if text.contains("scheduled") {
                Some(SenateEvent::FinalPassageScheduled)
            } else if text.contains("failed") || text.contains("rejected") {
                Some(SenateEvent::FinalPassageFails)
            } else if text.contains("passed") || text.contains("agreed to") {
                Some(SenateEvent::FinalPassageSucceeds)
            } else {
                None
            }
        }
        NormalizedActionCategory::Stall => {
            if text.contains("returned to calendar") {
                Some(SenateEvent::ReturnedToCalendar)
            } else if text.contains("blocked") || text.contains("procedural") {
                Some(SenateEvent::ProceduralBlock)
            } else {
                Some(SenateEvent::NoMeaningfulMovement)
            }
        }
        NormalizedActionCategory::Other => {
            if text.contains("leadership") || text.contains("signals action") {
                Some(SenateEvent::LeadershipSignalsAction)
            } else if text.contains("negotiat") {
                Some(SenateEvent::NegotiationIntensifies)
            } else if text.contains("returned to calendar") {
                Some(SenateEvent::ReturnedToCalendar)
            } else {
                None
            }
        }
        NormalizedActionCategory::Introduced
        | NormalizedActionCategory::Referred
        | NormalizedActionCategory::Reported => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::model::{
        legislative_context::Chamber, normalized_records::NormalizedActionCategory,
        senate_event::SenateEvent,
    };

    use super::{align_action_to_senate_event, is_consequential_action};

    fn action(
        category: NormalizedActionCategory,
        action_text: &str,
    ) -> crate::model::normalized_records::NormalizedActionRecord {
        crate::model::normalized_records::NormalizedActionRecord {
            action_id: "act_1".to_string(),
            object_id: "obj_1".to_string(),
            action_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
            chamber: Chamber::Senate,
            action_text: action_text.to_string(),
            category,
            as_of_date: NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
        }
    }

    #[test]
    fn action_alignment_maps_representative_actions() {
        assert_eq!(
            align_action_to_senate_event(&action(
                NormalizedActionCategory::Cloture,
                "Cloture filed on the motion to proceed."
            )),
            Some(SenateEvent::ClotureFiled)
        );
        assert_eq!(
            align_action_to_senate_event(&action(
                NormalizedActionCategory::Passage,
                "Bill passed Senate by yea-nay vote."
            )),
            Some(SenateEvent::FinalPassageSucceeds)
        );
    }

    #[test]
    fn consequential_filter_distinguishes_signal_from_noise() {
        assert!(is_consequential_action(&action(
            NormalizedActionCategory::MotionToProceed,
            "Motion to proceed filed."
        )));
        assert!(!is_consequential_action(&action(
            NormalizedActionCategory::Introduced,
            "Read twice and referred to committee."
        )));
    }
}
