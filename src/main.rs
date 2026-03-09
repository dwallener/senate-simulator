use serde::Serialize;

use senate_simulator::{
    LegislativeContext, ProceduralStage, SimulationState, SimulationStep, TerminationReason,
    build_synthetic_senate,
    io::json::{
        load_legislative_context_from_path, load_legislative_object_from_path, to_pretty_json,
    },
    rollout,
};

#[derive(Debug, Serialize)]
struct DemoOutput {
    initial_state: SimulationState,
    final_context: LegislativeContext,
    final_stage: ProceduralStage,
    terminated_reason: TerminationReason,
    steps: Vec<SimulationStep>,
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

    let initial_state = SimulationState {
        legislative_object,
        context,
        roster: build_synthetic_senate(),
        step_index: 0,
        last_event: None,
        consecutive_no_movement: 0,
        days_elapsed: 0,
        cloture_attempts: 0,
    };

    let trajectory = match rollout(&initial_state, 5) {
        Ok(trajectory) => trajectory,
        Err(error) => {
            eprintln!("Failed to run rollout: {error}");
            std::process::exit(1);
        }
    };

    println!(
        "Synthetic Senate rollout: roster={}, steps={}, terminated={:?}",
        initial_state.roster.len(),
        trajectory.steps.len(),
        trajectory.terminated_reason
    );
    for step in &trajectory.steps {
        println!(
            "Step {}: {:?} -> {} ({:.2})",
            step.step_index + 1,
            step.starting_stage,
            step.predicted_event,
            step.confidence
        );
        println!(
            "  Majority viable: {} | Cloture viable: {} | Support: {}+{} | Undecided: {}",
            step.analysis_summary.simple_majority_viable,
            step.analysis_summary.cloture_viable,
            step.analysis_summary.likely_support_count,
            step.analysis_summary.lean_support_count,
            step.analysis_summary.undecided_count
        );
        for reason in &step.top_reasons {
            println!("  - {reason}");
        }
    }
    println!("Terminated: {:?}", trajectory.terminated_reason);

    let output = DemoOutput {
        initial_state,
        final_context: trajectory.final_state.context.clone(),
        final_stage: trajectory.final_state.context.procedural_stage.clone(),
        terminated_reason: trajectory.terminated_reason.clone(),
        steps: trajectory.steps.clone(),
    };

    match to_pretty_json(&output) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("Failed to serialize rollout output: {error}");
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
