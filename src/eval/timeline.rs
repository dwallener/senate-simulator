use std::collections::BTreeMap;

use crate::{
    error::SenateSimError,
    eval::align::{align_action_to_senate_event, is_consequential_action},
    model::{
        historical_timeline::{HistoricalActionEvent, HistoricalTimeline},
        normalized_records::NormalizedActionRecord,
    },
};

pub fn build_historical_timelines(
    actions: &[NormalizedActionRecord],
) -> Result<Vec<HistoricalTimeline>, SenateSimError> {
    let mut grouped: BTreeMap<String, Vec<NormalizedActionRecord>> = BTreeMap::new();
    for action in actions {
        action.validate()?;
        grouped
            .entry(action.object_id.clone())
            .or_default()
            .push(action.clone());
    }

    let mut timelines = Vec::with_capacity(grouped.len());
    for (object_id, mut records) in grouped {
        records.sort_by(|left, right| {
            left.action_date
                .cmp(&right.action_date)
                .then_with(|| left.action_id.cmp(&right.action_id))
                .then_with(|| left.action_text.cmp(&right.action_text))
        });

        let events = records
            .into_iter()
            .map(|record| HistoricalActionEvent {
                object_id: record.object_id.clone(),
                action_date: record.action_date,
                raw_action_text: record.action_text.clone(),
                normalized_action_category: record.category,
                aligned_senate_event: align_action_to_senate_event(&record),
                is_consequential: is_consequential_action(&record),
                source_record_id: Some(record.action_id),
            })
            .collect::<Vec<_>>();

        timelines.push(HistoricalTimeline { object_id, events });
    }

    Ok(timelines)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::model::{
        historical_timeline::HistoricalTimeline,
        legislative_context::Chamber,
        normalized_records::{NormalizedActionCategory, NormalizedActionRecord},
    };

    use super::build_historical_timelines;

    fn action(id: &str, date: (i32, u32, u32), text: &str) -> NormalizedActionRecord {
        NormalizedActionRecord {
            action_id: id.to_string(),
            object_id: "obj_1".to_string(),
            action_date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            chamber: Chamber::Senate,
            action_text: text.to_string(),
            category: NormalizedActionCategory::Cloture,
            as_of_date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
        }
    }

    #[test]
    fn timeline_ordering_is_deterministic() {
        let timelines = build_historical_timelines(&[
            action("b", (2026, 3, 10), "Cloture vote scheduled."),
            action("a", (2026, 3, 9), "Cloture filed."),
            action("c", (2026, 3, 10), "Cloture invoked."),
        ])
        .unwrap();

        let HistoricalTimeline { events, .. } = &timelines[0];
        assert_eq!(events[0].source_record_id.as_deref(), Some("a"));
        assert_eq!(events[1].source_record_id.as_deref(), Some("b"));
        assert_eq!(events[2].source_record_id.as_deref(), Some("c"));
    }
}
