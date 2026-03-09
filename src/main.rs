use serde::Serialize;

use senate_simulator::{
    FloorActionAssessment, NextEventPrediction, SenateAnalysis, SenatorSignalSummary,
    analyze_chamber, assess_floor_action, build_synthetic_senate, derive_stance,
    io::json::{
        load_legislative_context_from_path, load_legislative_object_from_path, to_pretty_json,
    },
    predict_next_event,
};

#[derive(Debug, Serialize)]
struct DemoOutput {
    roster_size: usize,
    chamber_analysis: SenateAnalysis,
    floor_action_assessment: FloorActionAssessment,
    next_event_prediction: NextEventPrediction,
    top_pivots: Vec<senate_simulator::PivotSummary>,
    top_blockers: Vec<SenatorSignalSummary>,
}

fn main() {
    let legislative_object = load_or_exit(
        "examples/legislative_object_example.json",
        load_legislative_object_from_path,
    );
    let context = load_or_exit(
        "examples/legislative_context_example.json",
        load_legislative_context_from_path,
    );

    let roster = build_synthetic_senate();
    let mut stances = Vec::with_capacity(roster.len());

    for senator in &roster {
        let stance = match derive_stance(senator, &legislative_object, &context) {
            Ok(stance) => stance,
            Err(error) => {
                eprintln!(
                    "Failed to derive senator stance for {}: {error}",
                    senator.identity.senator_id
                );
                std::process::exit(1);
            }
        };
        stances.push(stance);
    }

    let chamber_analysis = match analyze_chamber(&legislative_object, &context, &stances) {
        Ok(analysis) => analysis,
        Err(error) => {
            eprintln!("Failed to analyze chamber: {error}");
            std::process::exit(1);
        }
    };
    let floor_action_assessment =
        match assess_floor_action(&legislative_object, &context, &chamber_analysis) {
            Ok(assessment) => assessment,
            Err(error) => {
                eprintln!("Failed to assess floor action: {error}");
                std::process::exit(1);
            }
        };
    let next_event_prediction =
        match predict_next_event(&legislative_object, &context, &chamber_analysis) {
            Ok(prediction) => prediction,
            Err(error) => {
                eprintln!("Failed to predict next event: {error}");
                std::process::exit(1);
            }
        };

    println!(
        "Synthetic Senate demo: roster={}, majority_viable={}, cloture_viable={}, predicted_action={}, next_event={}",
        roster.len(),
        chamber_analysis.simple_majority_viable,
        chamber_analysis.cloture_viable,
        floor_action_assessment.predicted_action,
        next_event_prediction.predicted_event
    );
    for reason in &floor_action_assessment.top_reasons {
        println!("- {reason}");
    }
    println!(
        "Most likely next event: {} ({:.2} confidence)",
        next_event_prediction.predicted_event, next_event_prediction.confidence
    );
    for alternative in &next_event_prediction.alternative_events {
        println!(
            "  alt: {} {:.2} {}",
            alternative.event, alternative.score, alternative.reason
        );
    }

    let output = DemoOutput {
        roster_size: roster.len(),
        top_pivots: chamber_analysis
            .pivotal_senators
            .iter()
            .take(5)
            .cloned()
            .collect(),
        top_blockers: chamber_analysis
            .likely_blockers
            .iter()
            .take(5)
            .cloned()
            .collect(),
        chamber_analysis,
        floor_action_assessment,
        next_event_prediction,
    };

    match to_pretty_json(&output) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("Failed to serialize demo output: {error}");
            std::process::exit(1);
        }
    }
}

fn load_or_exit<T, F>(path: &'static str, loader: F) -> T
where
    F: FnOnce(&'static str) -> Result<T, senate_simulator::SenateSimError>,
{
    match loader(path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Failed to load resource from {path}: {error}");
            std::process::exit(1);
        }
    }
}
