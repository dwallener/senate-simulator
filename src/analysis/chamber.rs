use crate::{
    error::SenateSimError,
    model::{
        legislative::LegislativeObject,
        legislative_context::{LegislativeContext, ProceduralStage},
        senate_analysis::{PivotSummary, SenateAnalysis, SenatorSignalSummary},
        senator_stance::{ProceduralPosture, SenatorStance, StanceLabel},
    },
};

pub fn analyze_chamber(
    legislative_object: &LegislativeObject,
    context: &LegislativeContext,
    stances: &[SenatorStance],
) -> Result<SenateAnalysis, SenateSimError> {
    legislative_object.validate()?;
    context.validate()?;

    if stances.is_empty() {
        return Err(SenateSimError::Validation {
            field: "stances",
            message: "must contain at least one senator stance".to_string(),
        });
    }

    for stance in stances {
        stance.validate()?;
        if stance.object_id != legislative_object.object_id {
            return Err(SenateSimError::Validation {
                field: "stances.object_id",
                message: "all stances must refer to the analyzed legislative object".to_string(),
            });
        }
    }

    let mut likely_support_count = 0usize;
    let mut lean_support_count = 0usize;
    let mut undecided_count = 0usize;
    let mut lean_oppose_count = 0usize;
    let mut likely_oppose_count = 0usize;

    for stance in stances {
        match stance.stance_label {
            StanceLabel::Support => likely_support_count += 1,
            StanceLabel::LeanSupport => lean_support_count += 1,
            StanceLabel::Undecided => undecided_count += 1,
            StanceLabel::LeanOppose => lean_oppose_count += 1,
            StanceLabel::Oppose => likely_oppose_count += 1,
        }
    }

    let total_senators = stances.len();
    let expected_present_count = rounded_expected_present_count(stances);
    let majority_threshold = (expected_present_count / 2) + 1;
    let current_support_votes = likely_support_count + lean_support_count;
    let probable_majority_votes = probable_majority_votes(stances);
    let simple_majority_viable = probable_majority_votes >= majority_threshold as f32;

    let probable_cloture_votes = probable_cloture_votes(stances, &context.procedural_stage);
    let cloture_viable = probable_cloture_votes >= 60.0;
    let blockers = likely_blockers(stances);
    let defectors = likely_defectors(stances);
    let filibuster_risk = derive_filibuster_risk(
        probable_cloture_votes,
        stances,
        blockers.len(),
        undecided_count,
    );
    let coalition_stability =
        derive_coalition_stability(stances, probable_majority_votes, majority_threshold);
    let pivotal_senators = pivotal_senators(
        stances,
        probable_majority_votes,
        majority_threshold,
        probable_cloture_votes,
    );
    let top_findings = build_top_findings(
        expected_present_count,
        current_support_votes,
        probable_majority_votes,
        majority_threshold,
        probable_cloture_votes,
        simple_majority_viable,
        cloture_viable,
        coalition_stability,
        blockers.len(),
        defectors.len(),
    );

    let analysis = SenateAnalysis {
        object_id: legislative_object.object_id.clone(),
        procedural_stage: context.procedural_stage.clone(),
        total_senators,
        likely_support_count,
        lean_support_count,
        undecided_count,
        lean_oppose_count,
        likely_oppose_count,
        expected_present_count,
        simple_majority_viable,
        cloture_viable,
        filibuster_risk,
        coalition_stability,
        pivotal_senators,
        likely_defectors: defectors,
        likely_blockers: blockers,
        top_findings,
    };

    analysis.validate()?;
    Ok(analysis)
}

fn rounded_expected_present_count(stances: &[SenatorStance]) -> usize {
    let expected_present = stances
        .iter()
        .map(|stance| 1.0 - stance.absence_probability)
        .sum::<f32>();

    expected_present.round().clamp(0.0, stances.len() as f32) as usize
}

fn probable_majority_votes(stances: &[SenatorStance]) -> f32 {
    stances
        .iter()
        .map(|stance| match stance.stance_label {
            StanceLabel::Support => 1.0,
            StanceLabel::LeanSupport => 1.0,
            StanceLabel::Undecided => 0.5 * stance.negotiability + 0.5 * stance.procedural_support,
            StanceLabel::LeanOppose => 0.20 * stance.negotiability,
            StanceLabel::Oppose => 0.0,
        })
        .sum()
}

fn probable_cloture_votes(stances: &[SenatorStance], stage: &ProceduralStage) -> f32 {
    stances
        .iter()
        .map(|stance| cloture_contribution(stance, stage))
        .sum()
}

fn cloture_contribution(stance: &SenatorStance, stage: &ProceduralStage) -> f32 {
    match stance.procedural_posture {
        ProceduralPosture::SupportCloture => 1.0,
        ProceduralPosture::OpposeCloture | ProceduralPosture::BlockByProcedure => 0.0,
        ProceduralPosture::SupportDebate | ProceduralPosture::SupportAmendmentProcess => {
            clamp_unit(0.45 + (stance.procedural_support * 0.45))
        }
        ProceduralPosture::OpposeDebate => 0.10,
        ProceduralPosture::Unclear => match stage {
            ProceduralStage::ClotureFiled | ProceduralStage::ClotureVote => {
                clamp_unit(stance.procedural_support * 0.75)
            }
            _ => clamp_unit(stance.procedural_support * 0.60),
        },
    }
}

fn likely_blockers(stances: &[SenatorStance]) -> Vec<SenatorSignalSummary> {
    let mut summaries: Vec<(f32, SenatorSignalSummary)> = stances
        .iter()
        .filter_map(|stance| {
            let blocking_score = (1.0 - stance.procedural_support) * 0.45
                + stance.rigidity * 0.35
                + if matches!(
                    stance.procedural_posture,
                    ProceduralPosture::OpposeCloture | ProceduralPosture::BlockByProcedure
                ) {
                    0.20
                } else {
                    0.0
                };

            if blocking_score >= 0.55 {
                Some((
                    blocking_score,
                    SenatorSignalSummary {
                        senator_id: stance.senator_id.clone(),
                        public_position: stance.public_position.clone(),
                        defection_probability: stance.defection_probability,
                        rigidity: stance.rigidity,
                        reason: "low procedural support and high rigidity mark a likely blocker"
                            .to_string(),
                    },
                ))
            } else {
                None
            }
        })
        .collect();

    summaries.sort_by(|a, b| b.0.total_cmp(&a.0));
    summaries
        .into_iter()
        .map(|(_, summary)| summary)
        .take(5)
        .collect()
}

fn likely_defectors(stances: &[SenatorStance]) -> Vec<SenatorSignalSummary> {
    let mut summaries: Vec<(f32, SenatorSignalSummary)> = stances
        .iter()
        .filter_map(|stance| {
            let coalition_side = matches!(
                stance.stance_label,
                StanceLabel::Support | StanceLabel::LeanSupport
            );
            let divergence = if coalition_side
                && matches!(
                    stance.procedural_posture,
                    ProceduralPosture::OpposeCloture | ProceduralPosture::BlockByProcedure
                ) {
                0.20
            } else {
                0.0
            };
            let defector_score = stance.defection_probability
                + divergence
                + if coalition_side && stance.public_support < 0.55 {
                    0.12
                } else {
                    0.0
                };

            if defector_score >= 0.38 {
                Some((
                    defector_score,
                    SenatorSignalSummary {
                        senator_id: stance.senator_id.clone(),
                        public_position: stance.public_position.clone(),
                        defection_probability: stance.defection_probability,
                        rigidity: stance.rigidity,
                        reason: "coalition-side support is vulnerable to defection under pressure"
                            .to_string(),
                    },
                ))
            } else {
                None
            }
        })
        .collect();

    summaries.sort_by(|a, b| b.0.total_cmp(&a.0));
    summaries
        .into_iter()
        .map(|(_, summary)| summary)
        .take(5)
        .collect()
}

fn pivotal_senators(
    stances: &[SenatorStance],
    probable_majority_votes: f32,
    majority_threshold: usize,
    probable_cloture_votes: f32,
) -> Vec<PivotSummary> {
    let majority_margin = (probable_majority_votes - majority_threshold as f32).abs();
    let cloture_margin = (60.0 - probable_cloture_votes).abs();
    let threshold_pressure = clamp_unit((3.0 - majority_margin.min(3.0)) / 3.0)
        .max(clamp_unit((4.0 - cloture_margin.min(4.0)) / 4.0));

    let mut candidates: Vec<(f32, PivotSummary)> = stances
        .iter()
        .filter_map(|stance| {
            let ideological_margin = 1.0 - ((stance.substantive_support - 0.5).abs() * 2.0);
            let procedural_margin = 1.0 - ((stance.procedural_support - 0.55).abs() * 2.0);
            let pivot_score = ideological_margin.max(0.0) * 0.35
                + procedural_margin.max(0.0) * 0.30
                + stance.negotiability * 0.20
                + threshold_pressure * 0.15;

            let is_candidate = matches!(
                stance.stance_label,
                StanceLabel::LeanSupport | StanceLabel::Undecided | StanceLabel::LeanOppose
            ) || (0.35..=0.75).contains(&stance.procedural_support);

            if !is_candidate {
                return None;
            }

            let reason = if stance.stance_label == StanceLabel::Undecided {
                "undecided stance leaves this senator near the coalition boundary"
            } else if stance.procedural_support < 0.60 && stance.substantive_support >= 0.50 {
                "substantive support exists, but procedural hesitation makes this senator pivotal"
            } else {
                "marginal support and meaningful negotiability make this senator pivotal"
            };

            Some((
                pivot_score,
                PivotSummary {
                    senator_id: stance.senator_id.clone(),
                    stance_label: stance.stance_label,
                    procedural_posture: stance.procedural_posture,
                    substantive_support: stance.substantive_support,
                    procedural_support: stance.procedural_support,
                    negotiability: stance.negotiability,
                    reason: reason.to_string(),
                },
            ))
        })
        .collect();

    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    candidates
        .into_iter()
        .map(|(_, summary)| summary)
        .take(5)
        .collect()
}

fn derive_filibuster_risk(
    probable_cloture_votes: f32,
    stances: &[SenatorStance],
    blocker_count: usize,
    undecided_count: usize,
) -> f32 {
    let cloture_gap = clamp_unit((60.0 - probable_cloture_votes) / 60.0);
    let blocker_intensity = if stances.is_empty() {
        0.0
    } else {
        blocker_count as f32 / stances.len() as f32
    };
    let procedural_hostility = if stances.is_empty() {
        0.0
    } else {
        stances
            .iter()
            .filter(|stance| stance.procedural_support < 0.40)
            .map(|stance| stance.rigidity)
            .sum::<f32>()
            / stances.len() as f32
    };
    let undecided_dependency = if stances.is_empty() {
        0.0
    } else {
        undecided_count as f32 / stances.len() as f32
    };

    clamp_unit(
        (cloture_gap * 0.55)
            + (blocker_intensity * 0.20)
            + (procedural_hostility * 0.15)
            + (undecided_dependency * 0.10),
    )
}

fn derive_coalition_stability(
    stances: &[SenatorStance],
    probable_majority_votes: f32,
    majority_threshold: usize,
) -> f32 {
    let coalition_members: Vec<&SenatorStance> = stances
        .iter()
        .filter(|stance| {
            matches!(
                stance.stance_label,
                StanceLabel::Support | StanceLabel::LeanSupport
            )
        })
        .collect();

    if coalition_members.is_empty() {
        return 0.0;
    }

    let support_count = coalition_members
        .iter()
        .filter(|stance| stance.stance_label == StanceLabel::Support)
        .count();
    let lean_share = 1.0 - (support_count as f32 / coalition_members.len() as f32);
    let avg_negotiability = coalition_members
        .iter()
        .map(|stance| stance.negotiability)
        .sum::<f32>()
        / coalition_members.len() as f32;
    let avg_defection = coalition_members
        .iter()
        .map(|stance| stance.defection_probability)
        .sum::<f32>()
        / coalition_members.len() as f32;
    let avg_absence = coalition_members
        .iter()
        .map(|stance| stance.absence_probability)
        .sum::<f32>()
        / coalition_members.len() as f32;
    let avg_alignment = coalition_members
        .iter()
        .map(|stance| (stance.substantive_support + stance.procedural_support) / 2.0)
        .sum::<f32>()
        / coalition_members.len() as f32;
    let avg_rigidity = coalition_members
        .iter()
        .map(|stance| stance.rigidity)
        .sum::<f32>()
        / coalition_members.len() as f32;
    let majority_margin = if probable_majority_votes > majority_threshold as f32 {
        (probable_majority_votes - majority_threshold as f32) / majority_threshold as f32
    } else {
        0.0
    };

    clamp_unit(
        0.20 + (majority_margin * 0.30) + (avg_alignment * 0.20) + (avg_rigidity * 0.15)
            - (lean_share * 0.20)
            - (avg_negotiability * 0.12)
            - (avg_defection * 0.18)
            - (avg_absence * 0.10),
    )
}

fn build_top_findings(
    expected_present_count: usize,
    current_support_votes: usize,
    probable_majority_votes: f32,
    majority_threshold: usize,
    probable_cloture_votes: f32,
    simple_majority_viable: bool,
    cloture_viable: bool,
    coalition_stability: f32,
    blocker_count: usize,
    defector_count: usize,
) -> Vec<String> {
    let mut findings = Vec::new();

    findings.push(format!(
        "current support coalition is {current_support_votes} likely/lean supporters out of {expected_present_count} expected present"
    ));

    findings.push(if simple_majority_viable {
        format!(
            "simple majority is viable at an estimated {:.1} votes against a {}-vote threshold",
            probable_majority_votes, majority_threshold
        )
    } else {
        format!(
            "simple majority is not yet viable; estimated support is {:.1} against a {}-vote threshold",
            probable_majority_votes, majority_threshold
        )
    });

    findings.push(if cloture_viable {
        format!(
            "procedural coalition reaches an estimated {:.1} cloture votes and clears the 60-vote threshold",
            probable_cloture_votes
        )
    } else {
        format!(
            "cloture remains short at an estimated {:.1} procedural votes",
            probable_cloture_votes
        )
    });

    findings.push(if coalition_stability >= 0.65 {
        "coalition stability is relatively strong at this snapshot".to_string()
    } else if coalition_stability >= 0.40 {
        "coalition exists but remains somewhat fragile".to_string()
    } else {
        "coalition stability is weak and vulnerable to movement".to_string()
    });

    if blocker_count > 0 {
        findings.push(format!(
            "{blocker_count} likely procedural blockers materially raise filibuster risk"
        ));
    }

    if defector_count > 0 {
        findings.push(format!(
            "{defector_count} coalition-side senators show meaningful defection risk"
        ));
    }

    findings.truncate(6);
    findings
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
        SenateAnalysis,
        model::{
            dynamic_state::PublicPosition,
            legislative::{
                BudgetaryImpact, LegislativeObject, LegislativeObjectType, PolicyDomain,
            },
            legislative_context::{
                Chamber, CongressionalSession, LegislativeContext, ProceduralStage,
            },
            senator_stance::{ProceduralPosture, SenatorStance, StanceLabel},
        },
    };

    use super::analyze_chamber;

    fn example_object() -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Synthetic Test Bill".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: PolicyDomain::EnergyClimate,
            summary: "Synthetic bill for chamber analysis tests.".to_string(),
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

    fn example_context(stage: ProceduralStage) -> LegislativeContext {
        LegislativeContext {
            congress_number: 119,
            session: CongressionalSession::First,
            current_chamber: Chamber::Senate,
            procedural_stage: stage,
            majority_party: crate::Party::Democrat,
            minority_party: crate::Party::Republican,
            president_party: crate::Party::Democrat,
            days_until_election: Some(100),
            days_until_deadline: Some(15),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.8,
            media_attention: 0.7,
        }
    }

    fn stance(
        senator_id: &str,
        stance_label: StanceLabel,
        procedural_posture: ProceduralPosture,
        substantive_support: f32,
        procedural_support: f32,
        negotiability: f32,
        rigidity: f32,
        defection_probability: f32,
        absence_probability: f32,
    ) -> SenatorStance {
        SenatorStance {
            senator_id: senator_id.to_string(),
            object_id: "obj_001".to_string(),
            context_id: Some("ctx_test".to_string()),
            substantive_support,
            procedural_support,
            public_support: substantive_support,
            negotiability,
            rigidity,
            defection_probability,
            absence_probability,
            stance_label,
            procedural_posture,
            public_position: if substantive_support >= 0.65 {
                PublicPosition::Support
            } else if substantive_support <= 0.25 {
                PublicPosition::Oppose
            } else {
                PublicPosition::Negotiating
            },
            top_factors: vec!["synthetic".to_string()],
            score_breakdown: None,
        }
    }

    fn analyze(stances: &[SenatorStance], stage: ProceduralStage) -> SenateAnalysis {
        analyze_chamber(&example_object(), &example_context(stage), stances).unwrap()
    }

    #[test]
    fn buckets_counts_by_stance_label() {
        let stances = vec![
            stance(
                "s1",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.85,
                0.90,
                0.2,
                0.8,
                0.1,
                0.0,
            ),
            stance(
                "s2",
                StanceLabel::LeanSupport,
                ProceduralPosture::SupportCloture,
                0.65,
                0.85,
                0.4,
                0.6,
                0.2,
                0.0,
            ),
            stance(
                "s3",
                StanceLabel::Undecided,
                ProceduralPosture::Unclear,
                0.50,
                0.50,
                0.6,
                0.4,
                0.3,
                0.1,
            ),
            stance(
                "s4",
                StanceLabel::LeanOppose,
                ProceduralPosture::OpposeCloture,
                0.30,
                0.20,
                0.3,
                0.7,
                0.2,
                0.0,
            ),
            stance(
                "s5",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.10,
                0.10,
                0.2,
                0.9,
                0.1,
                0.0,
            ),
        ];

        let analysis = analyze(&stances, ProceduralStage::ClotureFiled);

        assert_eq!(analysis.likely_support_count, 1);
        assert_eq!(analysis.lean_support_count, 1);
        assert_eq!(analysis.undecided_count, 1);
        assert_eq!(analysis.lean_oppose_count, 1);
        assert_eq!(analysis.likely_oppose_count, 1);
    }

    #[test]
    fn majority_viability_is_true_when_support_clears_threshold() {
        let stances = vec![
            stance(
                "s1",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.88,
                0.88,
                0.2,
                0.8,
                0.1,
                0.0,
            ),
            stance(
                "s2",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.82,
                0.84,
                0.2,
                0.8,
                0.1,
                0.0,
            ),
            stance(
                "s3",
                StanceLabel::LeanSupport,
                ProceduralPosture::SupportDebate,
                0.68,
                0.70,
                0.3,
                0.6,
                0.2,
                0.0,
            ),
            stance(
                "s4",
                StanceLabel::LeanSupport,
                ProceduralPosture::SupportDebate,
                0.62,
                0.68,
                0.3,
                0.6,
                0.2,
                0.0,
            ),
            stance(
                "s5",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.12,
                0.10,
                0.2,
                0.9,
                0.1,
                0.0,
            ),
            stance(
                "s6",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.10,
                0.10,
                0.2,
                0.9,
                0.1,
                0.0,
            ),
        ];

        let analysis = analyze(&stances, ProceduralStage::Debate);
        assert!(analysis.simple_majority_viable);
    }

    #[test]
    fn majority_can_be_viable_while_cloture_is_not() {
        let mut stances = Vec::new();
        for idx in 0..35 {
            stances.push(stance(
                &format!("s{idx}"),
                StanceLabel::Support,
                ProceduralPosture::Unclear,
                0.82,
                0.42,
                0.3,
                0.7,
                0.1,
                0.0,
            ));
        }
        for idx in 35..55 {
            stances.push(stance(
                &format!("s{idx}"),
                StanceLabel::LeanSupport,
                ProceduralPosture::Unclear,
                0.66,
                0.38,
                0.4,
                0.5,
                0.2,
                0.0,
            ));
        }
        for idx in 55..70 {
            stances.push(stance(
                &format!("s{idx}"),
                StanceLabel::Oppose,
                ProceduralPosture::OpposeCloture,
                0.12,
                0.08,
                0.2,
                0.9,
                0.1,
                0.0,
            ));
        }

        let analysis = analyze(&stances, ProceduralStage::ClotureFiled);
        assert!(analysis.simple_majority_viable);
        assert!(!analysis.cloture_viable);
    }

    #[test]
    fn robust_coalition_scores_higher_stability_than_fragile_one() {
        let robust = vec![
            stance(
                "r1",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.90,
                0.92,
                0.10,
                0.85,
                0.05,
                0.01,
            ),
            stance(
                "r2",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.88,
                0.90,
                0.10,
                0.84,
                0.05,
                0.01,
            ),
            stance(
                "r3",
                StanceLabel::Support,
                ProceduralPosture::SupportDebate,
                0.86,
                0.85,
                0.15,
                0.80,
                0.06,
                0.01,
            ),
            stance(
                "r4",
                StanceLabel::LeanSupport,
                ProceduralPosture::SupportDebate,
                0.70,
                0.74,
                0.20,
                0.72,
                0.10,
                0.01,
            ),
            stance(
                "r5",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.08,
                0.10,
                0.10,
                0.90,
                0.05,
                0.01,
            ),
            stance(
                "r6",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.05,
                0.08,
                0.10,
                0.92,
                0.05,
                0.01,
            ),
        ];
        let fragile = vec![
            stance(
                "f1",
                StanceLabel::LeanSupport,
                ProceduralPosture::Unclear,
                0.62,
                0.58,
                0.65,
                0.40,
                0.35,
                0.06,
            ),
            stance(
                "f2",
                StanceLabel::LeanSupport,
                ProceduralPosture::Unclear,
                0.61,
                0.55,
                0.62,
                0.42,
                0.36,
                0.06,
            ),
            stance(
                "f3",
                StanceLabel::Undecided,
                ProceduralPosture::Unclear,
                0.52,
                0.54,
                0.70,
                0.35,
                0.32,
                0.06,
            ),
            stance(
                "f4",
                StanceLabel::Support,
                ProceduralPosture::SupportDebate,
                0.80,
                0.70,
                0.55,
                0.50,
                0.28,
                0.05,
            ),
            stance(
                "f5",
                StanceLabel::Oppose,
                ProceduralPosture::OpposeCloture,
                0.12,
                0.15,
                0.20,
                0.85,
                0.08,
                0.03,
            ),
            stance(
                "f6",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.10,
                0.10,
                0.20,
                0.90,
                0.08,
                0.03,
            ),
        ];

        let robust_analysis = analyze(&robust, ProceduralStage::Debate);
        let fragile_analysis = analyze(&fragile, ProceduralStage::Debate);

        assert!(robust_analysis.coalition_stability > fragile_analysis.coalition_stability);
    }

    #[test]
    fn rigid_low_procedural_senator_is_flagged_as_blocker() {
        let stances = vec![
            stance(
                "blocker",
                StanceLabel::Oppose,
                ProceduralPosture::BlockByProcedure,
                0.08,
                0.05,
                0.10,
                0.95,
                0.08,
                0.0,
            ),
            stance(
                "supporter",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.90,
                0.90,
                0.15,
                0.80,
                0.05,
                0.0,
            ),
            stance(
                "supporter2",
                StanceLabel::Support,
                ProceduralPosture::SupportCloture,
                0.88,
                0.88,
                0.15,
                0.82,
                0.05,
                0.0,
            ),
        ];

        let analysis = analyze(&stances, ProceduralStage::ClotureVote);
        assert!(
            analysis
                .likely_blockers
                .iter()
                .any(|summary| summary.senator_id == "blocker")
        );
    }
}
