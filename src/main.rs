use senate_simulator::{
    AlignmentReport, BacktestResult, DataSnapshot, EvaluationSummary, FloorActionAssessment,
    NextEventPrediction, SenateAnalysis, SimulationState, TerminationReason,
    FeatureReport, FeatureWindowConfig, SenatorFeatureRecord, build_and_persist_features,
    IngestionConfig, IngestionSourceMode, SenatorProfileMode, StanceDerivationMode, analyze_chamber,
    assess_floor_action, build_evaluation_artifacts_for_snapshot_date, derive_stance_with_mode,
    export_public_artifacts,
    evaluate_from_snapshot_date_with_mode, load_feature_records, load_snapshot,
    predict_next_event, rollout_with_mode, run_backtest_with_mode, run_daily_ingestion,
    run_ingestion, senators_for_snapshot, snapshot_to_contexts, snapshot_to_legislative_objects,
    snapshot_with_features_to_senators,
};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = match args.first().map(String::as_str) {
        Some("ingest") => run_ingest_command(&args[1..]),
        Some("backtest") => run_backtest_command(&args[1..]),
        Some("eval-build") => run_eval_build_command(&args[1..]),
        Some("eval-run") => run_eval_run_command(&args[1..]),
        Some("features-build") => run_features_build_command(&args[1..]),
        Some("features-inspect") => run_features_inspect_command(&args[1..]),
        Some("predict-export") => run_predict_export_command(&args[1..]),
        Some("predict-bill") => run_predict_bill_command(&args[1..]),
        Some("signals-inspect") => run_signals_inspect_command(&args[1..]),
        Some("stance-inspect") => run_stance_inspect_command(&args[1..]),
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
    let source_mode = parse_source_mode(parse_date_arg(args, "--source").unwrap_or("fixtures"))?;
    let mut config = IngestionConfig::fixtures(run_date);
    config.source_mode = source_mode;
    config.use_cached_raw_if_present = args.iter().any(|arg| arg == "--reuse-raw");
    config.include_gdelt = args.iter().any(|arg| arg == "--include-gdelt");
    if source_mode == IngestionSourceMode::Live {
        config.congress_api_key = std::env::var("API_KEY_DATA_GOV").ok();
    }
    let snapshot = run_ingestion(&config)?;
    print_snapshot_summary(&snapshot);
    Ok(())
}

fn run_backtest_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let object_id = parse_date_arg(args, "--object-id").unwrap_or("s_2100");
    let mode = parse_stance_mode(parse_date_arg(args, "--stance-mode").unwrap_or("feature"))?;
    let snapshot_date = parse_date(date)?;
    let result = run_backtest_with_mode(snapshot_date, object_id, mode)?;
    print_backtest_summary(&result);
    Ok(())
}

fn run_default_demo() -> Result<(), senate_simulator::SenateSimError> {
    let run_date = parse_date("2026-03-09")?;
    let snapshot = run_daily_ingestion(run_date)?;
    print_snapshot_summary(&snapshot);

    let result = run_backtest_with_mode(run_date, "s_2100", StanceDerivationMode::FeatureDriven)?;
    print_backtest_summary(&result);

    let snapshot_loaded = load_or_refresh_snapshot(run_date)?;
    let (_, feature_report) = build_and_persist_features(
        &snapshot_loaded,
        std::path::Path::new("data"),
        &FeatureWindowConfig::default(),
    )?;
    print_feature_report(&feature_report);

    let artifacts = build_evaluation_artifacts_for_snapshot_date(run_date)?;
    print_alignment_summary(&artifacts.alignment_report);

    let summary = evaluate_from_snapshot_date_with_mode(
        run_date,
        StanceDerivationMode::FeatureDriven,
    )?;
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
    let mode = parse_stance_mode(parse_date_arg(args, "--stance-mode").unwrap_or("feature"))?;
    let snapshot_date = parse_date(date)?;
    let summary = evaluate_from_snapshot_date_with_mode(snapshot_date, mode)?;
    print_evaluation_summary(&summary);
    Ok(())
}

fn run_features_build_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let snapshot_date = parse_date(date)?;
    let snapshot = load_or_refresh_snapshot(snapshot_date)?;
    let (_, report) = build_and_persist_features(
        &snapshot,
        std::path::Path::new("data"),
        &FeatureWindowConfig::default(),
    )?;
    print_feature_report(&report);
    Ok(())
}

fn run_features_inspect_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let senator_id = parse_date_arg(args, "--senator-id").unwrap_or("real_a0001");
    let snapshot_date = parse_date(date)?;
    let features = match load_feature_records(std::path::Path::new("data"), snapshot_date) {
        Ok(records) => records,
        Err(_) => {
            let snapshot = load_or_refresh_snapshot(snapshot_date)?;
            let (records, _) = build_and_persist_features(
                &snapshot,
                std::path::Path::new("data"),
                &FeatureWindowConfig::default(),
            )?;
            records
        }
    };
    let record = features
        .into_iter()
        .find(|record| record.senator_id == senator_id)
        .ok_or_else(|| senate_simulator::SenateSimError::Validation {
            field: "cli.senator_id",
            message: format!("senator_id {senator_id} not found in feature artifacts"),
        })?;
    print_feature_record(&record);
    Ok(())
}

fn run_predict_export_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let tracked_bills_file = parse_date_arg(args, "--tracked-bills-file").unwrap_or("tracked_bills.json");
    let out = parse_date_arg(args, "--out").unwrap_or("data/public");
    let mode = parse_stance_mode(parse_date_arg(args, "--stance-mode").unwrap_or("feature"))?;
    let steps = parse_date_arg(args, "--steps")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3);
    let snapshot_date = parse_date(date)?;
    let artifacts = export_public_artifacts(
        snapshot_date,
        std::path::Path::new(tracked_bills_file),
        std::path::Path::new(out),
        mode,
        steps,
    )?;
    println!(
        "Public export {}: tracked={} exported={} out={}",
        snapshot_date,
        artifacts.last_updated.tracked_bill_count,
        artifacts.last_updated.exported_bill_count,
        out
    );
    for note in &artifacts.summary.notes {
        println!("  - {note}");
    }
    Ok(())
}

fn run_stance_inspect_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let senator_id = parse_date_arg(args, "--senator-id").unwrap_or("real_a0001");
    let object_id = parse_date_arg(args, "--object-id").unwrap_or("s_2100");
    let mode = parse_stance_mode(parse_date_arg(args, "--stance-mode").unwrap_or("feature"))?;
    let snapshot_date = parse_date(date)?;
    let snapshot = load_or_refresh_snapshot(snapshot_date)?;
    let features = match load_feature_records(std::path::Path::new("data"), snapshot_date) {
        Ok(records) => records,
        Err(_) => {
            let (records, _) = build_and_persist_features(
                &snapshot,
                std::path::Path::new("data"),
                &FeatureWindowConfig::default(),
            )?;
            records
        }
    };
    let senators = snapshot_with_features_to_senators(&snapshot, &features)?;
    let objects = snapshot_to_legislative_objects(&snapshot)?;
    let contexts = snapshot_to_contexts(&snapshot)?;
    let senator = senators
        .iter()
        .find(|senator| senator.identity.senator_id == senator_id)
        .ok_or_else(|| senate_simulator::SenateSimError::Validation {
            field: "cli.senator_id",
            message: format!("senator_id {senator_id} not found"),
        })?;
    let index = objects
        .iter()
        .position(|object| object.object_id == object_id)
        .ok_or_else(|| senate_simulator::SenateSimError::Validation {
            field: "cli.object_id",
            message: format!("object_id {object_id} not found"),
        })?;
    let stance = derive_stance_with_mode(senator, &objects[index], &contexts[index], mode)?;
    print_stance_details(&stance);
    Ok(())
}

fn run_signals_inspect_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let snapshot_date = parse_date(date)?;
    let mut snapshot = load_or_refresh_snapshot(snapshot_date)?;
    if snapshot.public_signal_records.is_empty() {
        let mut config = IngestionConfig::fixtures(snapshot_date);
        config.include_gdelt = true;
        snapshot = run_ingestion(&config)?;
    }
    println!(
        "Public signals {}: records={}",
        snapshot.snapshot_date,
        snapshot.public_signal_records.len()
    );
    if let Some(object_id) = parse_date_arg(args, "--object-id") {
        for record in snapshot
            .public_signal_records
            .iter()
            .filter(|record| record.linked_object_id.as_deref() == Some(object_id))
        {
            print_public_signal_record(record);
        }
    } else if let Some(senator_id) = parse_date_arg(args, "--senator-id") {
        for record in snapshot
            .public_signal_records
            .iter()
            .filter(|record| record.linked_senator_id.as_deref() == Some(senator_id))
        {
            print_public_signal_record(record);
        }
    } else {
        for record in snapshot.public_signal_records.iter().take(5) {
            print_public_signal_record(record);
        }
    }
    if let Some(summary) = &snapshot.public_signal_summary {
        println!(
            "  summary: object_attention={} senator_attention={} domain_attention={}",
            summary.object_attention.len(),
            summary.senator_attention.len(),
            summary.domain_attention.len()
        );
        for note in &summary.notes {
            println!("  - {note}");
        }
    }
    Ok(())
}

fn run_predict_bill_command(args: &[String]) -> Result<(), senate_simulator::SenateSimError> {
    let date = parse_date_arg(args, "--date").unwrap_or("2026-03-09");
    let object_id = parse_date_arg(args, "--object-id").unwrap_or("s_2100");
    let mode = parse_stance_mode(parse_date_arg(args, "--stance-mode").unwrap_or("feature"))?;
    let steps = parse_date_arg(args, "--steps")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3);
    let snapshot_date = parse_date(date)?;
    let snapshot = load_or_refresh_snapshot(snapshot_date)?;
    let senators = senators_for_snapshot(
        &snapshot,
        std::path::Path::new("data"),
        SenatorProfileMode::HistoricalFeatures,
    )?;
    let objects = snapshot_to_legislative_objects(&snapshot)?;
    let contexts = snapshot_to_contexts(&snapshot)?;
    let index = objects
        .iter()
        .position(|object| object.object_id == object_id)
        .ok_or_else(|| senate_simulator::SenateSimError::Validation {
            field: "cli.object_id",
            message: format!("object_id {object_id} not found"),
        })?;

    let legislative_object = &objects[index];
    let context = &contexts[index];
    let stances = senators
        .iter()
        .map(|senator| derive_stance_with_mode(senator, legislative_object, context, mode))
        .collect::<Result<Vec<_>, _>>()?;
    let analysis = analyze_chamber(legislative_object, context, &stances)?;
    let floor_action = assess_floor_action(legislative_object, context, &analysis)?;
    let next_event = predict_next_event(legislative_object, context, &analysis)?;
    let trajectory = rollout_with_mode(
        &SimulationState {
            legislative_object: legislative_object.clone(),
            context: context.clone(),
            roster: senators,
            step_index: 0,
            last_event: None,
            consecutive_no_movement: 0,
            days_elapsed: 0,
            cloture_attempts: 0,
        },
        steps,
        mode,
    )?;

    print_bill_prediction_summary(
        legislative_object.object_id.as_str(),
        snapshot_date,
        &analysis,
        &floor_action,
        &next_event,
        &trajectory.terminated_reason,
        &trajectory.steps,
    );
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

fn parse_stance_mode(
    value: &str,
) -> Result<StanceDerivationMode, senate_simulator::SenateSimError> {
    match value {
        "heuristic" => Ok(StanceDerivationMode::Heuristic),
        "feature" | "feature-driven" | "feature_driven" => {
            Ok(StanceDerivationMode::FeatureDriven)
        }
        _ => Err(senate_simulator::SenateSimError::Validation {
            field: "cli.stance_mode",
            message: format!("invalid stance mode {value}, expected heuristic or feature"),
        }),
    }
}

fn parse_source_mode(
    value: &str,
) -> Result<IngestionSourceMode, senate_simulator::SenateSimError> {
    match value {
        "fixtures" | "fixture" => Ok(IngestionSourceMode::Fixtures),
        "live" => Ok(IngestionSourceMode::Live),
        _ => Err(senate_simulator::SenateSimError::Validation {
            field: "cli.source",
            message: format!("invalid source mode {value}, expected fixtures or live"),
        }),
    }
}

fn load_or_refresh_snapshot(
    snapshot_date: chrono::NaiveDate,
) -> Result<DataSnapshot, senate_simulator::SenateSimError> {
    match load_snapshot(std::path::Path::new("data"), snapshot_date) {
        Ok(snapshot) => Ok(snapshot),
        Err(_) => run_daily_ingestion(snapshot_date),
    }
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

fn print_feature_report(report: &FeatureReport) {
    println!(
        "Feature build {}: senators={}, sparse={}, avg_coverage={:.2}",
        report.snapshot_date,
        report.senators_processed,
        report.senators_with_sparse_history,
        report.average_coverage_score
    );
    for note in &report.notes {
        println!("  - {note}");
    }
}

fn print_feature_record(record: &SenatorFeatureRecord) {
    println!(
        "Feature record {} {}: loyalty={:.2}, bipartisanship={:.2}, attendance={:.2}, cloture={:.2}, coverage={:.2}",
        record.senator_id,
        record.full_name,
        record.party_loyalty_baseline,
        record.bipartisanship_baseline,
        record.attendance_reliability,
        record.cloture_support_baseline,
        record.coverage_score
    );
    println!(
        "  ideology={:.2}, recent_loyalty={:.2}, recent_cloture={:.2}",
        record.ideology_proxy, record.recent_party_loyalty, record.recent_cloture_support
    );
    for note in &record.notes {
        println!("  - {note}");
    }
}

fn print_stance_details(stance: &senate_simulator::SenatorStance) {
    println!(
        "Stance {} on {}: substantive={:.2}, procedural={:.2}, public={:.2}, label={:?}, posture={:?}",
        stance.senator_id,
        stance.object_id,
        stance.substantive_support,
        stance.procedural_support,
        stance.public_support,
        stance.stance_label,
        stance.procedural_posture
    );
    if let Some(breakdown) = &stance.score_breakdown {
        println!(
            "  breakdown: domain={:.2}, procedural={:.2}, party={:.2}, coverage={:.2}",
            breakdown.domain_affinity_score,
            breakdown.procedural_compatibility_score,
            breakdown.party_alignment_score,
            breakdown.coverage_score
        );
        println!(
            "  adjustments: salience={:.2}, controversy={:.2}, recent_drift={:.2}, attendance={:.2}",
            breakdown.salience_adjustment,
            breakdown.controversy_adjustment,
            breakdown.recent_drift_adjustment,
            breakdown.attendance_adjustment
        );
        for note in &breakdown.fallback_notes {
            println!("  - {note}");
        }
        for factor in &stance.top_factors {
            if !breakdown.fallback_notes.contains(factor) {
                println!("  - {factor}");
            }
        }
        return;
    }
    for factor in &stance.top_factors {
        println!("  - {factor}");
    }
}

fn print_public_signal_record(record: &senate_simulator::NormalizedPublicSignalRecord) {
    println!(
        "Signal {} {:?}: mentions={}, attention={:.2}, senator={:?}, object={:?}, domain={:?}",
        record.signal_id,
        record.signal_scope,
        record.mention_count,
        record.attention_score,
        record.linked_senator_id,
        record.linked_object_id,
        record.policy_domain
    );
    if let Some(tone) = record.tone_score {
        println!("  tone={:.2}", tone);
    }
    if !record.top_themes.is_empty() {
        println!("  themes={}", record.top_themes.join(", "));
    }
    if !record.top_organizations.is_empty() {
        println!("  sources={}", record.top_organizations.join(", "));
    }
}

fn print_bill_prediction_summary(
    object_id: &str,
    snapshot_date: chrono::NaiveDate,
    analysis: &SenateAnalysis,
    floor_action: &FloorActionAssessment,
    next_event: &NextEventPrediction,
    terminated_reason: &TerminationReason,
    steps: &[senate_simulator::SimulationStep],
) {
    println!(
        "Prediction {} {}: support={} lean_support={} undecided={} oppose={}",
        snapshot_date,
        object_id,
        analysis.likely_support_count,
        analysis.lean_support_count,
        analysis.undecided_count,
        analysis.likely_oppose_count + analysis.lean_oppose_count
    );
    println!(
        "  majority_viable={} cloture_viable={} stability={:.2} filibuster_risk={:.2}",
        analysis.simple_majority_viable,
        analysis.cloture_viable,
        analysis.coalition_stability,
        analysis.filibuster_risk
    );
    println!(
        "  floor_action={} ({:.2}) support_margin={} cloture_gap={}",
        floor_action.predicted_action,
        floor_action.confidence,
        floor_action.support_margin_estimate,
        floor_action.cloture_gap_estimate
    );
    println!(
        "  next_event={:?} score={:.2} confidence={:.2} stage={:?}",
        next_event.predicted_event,
        next_event.predicted_event_score,
        next_event.confidence,
        next_event.current_stage
    );
    for alternative in next_event.alternative_events.iter().take(3) {
        println!("  alt {:?} {:.2}: {}", alternative.event, alternative.score, alternative.reason);
    }
    for reason in next_event.top_reasons.iter().take(4) {
        println!("  - {reason}");
    }
    if !analysis.pivotal_senators.is_empty() {
        let pivots = analysis
            .pivotal_senators
            .iter()
            .take(5)
            .map(|pivot| pivot.senator_id.as_str())
            .collect::<Vec<_>>();
        println!("  pivots={}", pivots.join(", "));
    }
    if !analysis.likely_blockers.is_empty() {
        let blockers = analysis
            .likely_blockers
            .iter()
            .take(5)
            .map(|blocker| blocker.senator_id.as_str())
            .collect::<Vec<_>>();
        println!("  blockers={}", blockers.join(", "));
    }
    for step in steps.iter().take(steps.len().min(3)) {
        println!(
            "  rollout step {}: {:?} -> {:?} ({:.2})",
            step.step_index + 1,
            step.starting_stage,
            step.predicted_event,
            step.confidence
        );
    }
    println!("  terminated={:?}", terminated_reason);
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
