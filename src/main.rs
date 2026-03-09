use senate_simulator::{
    AlignmentReport, BacktestResult, DataSnapshot, EvaluationSummary,
    build_evaluation_artifacts_for_snapshot_date, evaluate_from_snapshot_date, run_backtest,
    run_daily_ingestion,
};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = match args.first().map(String::as_str) {
        Some("ingest") => run_ingest_command(&args[1..]),
        Some("backtest") => run_backtest_command(&args[1..]),
        Some("eval-build") => run_eval_build_command(&args[1..]),
        Some("eval-run") => run_eval_run_command(&args[1..]),
        _ => run_default_demo(),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_ingest_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let run_date = parse_date(date)?;
    let snapshot = run_daily_ingestion(run_date)?;
    print_snapshot_summary(&snapshot);
    Ok(())
}

fn run_backtest_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let object_id = parse_date_arg(args, "--object-id").unwrap_or("s_2100");
    let snapshot_date = parse_date(date)?;
    let result = run_backtest(snapshot_date, object_id)?;
    print_backtest_summary(&result);
    Ok(())
}

fn run_default_demo() -> Result<(), senate_simulator::SenateSimError> {
    let run_date = parse_date("2026-03-09")?;
    let snapshot = run_daily_ingestion(run_date)?;
    print_snapshot_summary(&snapshot);

    let result = run_backtest(run_date, "s_2100")?;
    print_backtest_summary(&result);

    let artifacts = build_evaluation_artifacts_for_snapshot_date(run_date)?;
    print_alignment_summary(&artifacts.alignment_report);

    let summary = evaluate_from_snapshot_date(run_date)?;
    print_evaluation_summary(&summary);
    Ok(())
}

fn run_eval_build_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let snapshot_date = parse_date(date)?;
    let artifacts = build_evaluation_artifacts_for_snapshot_date(snapshot_date)?;
    print_alignment_summary(&artifacts.alignment_report);
    println!(
        "  persisted examples={} trajectories={}",
        artifacts.examples.len(),
        artifacts.trajectories.len()
    );
    Ok(())
}

fn run_eval_run_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let snapshot_date = parse_date(date)?;
    let summary = evaluate_from_snapshot_date(snapshot_date)?;
    print_evaluation_summary(&summary);
    Ok(())
}

fn parse_date(value: &str) -> Result<chrono::NaiveDate, senate_simulator::SenateSimError> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        senate_simulator::SenateSimError::Validation {
            field: "cli.date",
            message: format!("invalid date {value}, expected YYYY-MM-DD"),
        }
    })
}

fn parse_date_arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn print_snapshot_summary(snapshot: &DataSnapshot) {
    println!(
        "Ingested snapshot {}: senators={}, legislation={}, actions={}, manifests={}",
        snapshot.snapshot_date,
        snapshot.roster_records.len(),
        snapshot.legislative_records.len(),
        snapshot.action_records.len(),
        snapshot.source_manifests.len()
    );
    for manifest in &snapshot.source_manifests {
        println!(
            "  {}: records={}, hash={}, as_of={}",
            manifest.source_name, manifest.record_count, manifest.content_hash, manifest.as_of_date
        );
    }
}

fn print_backtest_summary(result: &BacktestResult) {
    println!(
        "Backtest {} {}: predicted={:?}, actual={:?}, top1={}, topk={}, confidence={:.2}",
        result.snapshot_date,
        result.object_id,
        result.predicted_next_event,
        result.actual_next_event,
        result.match_top_1,
        result.match_top_k,
        result.prediction_confidence.unwrap_or(0.0)
    );
    for note in &result.notes {
        println!("  - {note}");
    }
}

fn print_alignment_summary(report: &AlignmentReport) {
    println!(
        "Evaluation build {}: objects={}, examples={}, ambiguous_actions={}, unaligned_consequential={}",
        report.snapshot_date,
        report.objects_processed,
        report.examples_generated,
        report.ambiguous_actions,
        report.unaligned_consequential_actions
    );
    for note in &report.notes {
        println!("  - {note}");
    }
}

fn print_evaluation_summary(summary: &EvaluationSummary) {
    println!(
        "Evaluation summary: total={}, top1={:.2}, topk={:.2}, trajectory_prefix={:.2}, unscorable={}",
        summary.total_examples,
        summary.top_1_next_event_accuracy,
        summary.top_k_next_event_accuracy,
        summary.trajectory_prefix_match_rate,
        summary.unscorable_examples
    );
    for note in &summary.notes {
        println!("  - {note}");
    }
}

#[cfg(test)]
mod tests {
    use super::parse_date_arg;

    #[test]
    fn parses_flag_values() {
        let args = vec![
            "--date".to_string(),
            "2026-03-09".to_string(),
            "--object-id".to_string(),
            "s_2100".to_string(),
        ];
        assert_eq!(parse_date_arg(&args, "--date"), Some("2026-03-09"));
        assert_eq!(parse_date_arg(&args, "--object-id"), Some("s_2100"));
        assert_eq!(parse_date_arg(&args, "--missing"), None);
    }
}
