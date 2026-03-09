use chrono::NaiveDate;

use crate::{
    Party, PublicPosition, SenateClass, Senator,
    model::{
        dynamic_state::DynamicState, identity::Identity, issue_preferences::IssuePreferences,
        procedural::Procedural, structural::Structural,
    },
};

const STATE_CODES: [&str; 50] = [
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA", "HI", "ID", "IL", "IN", "IA", "KS",
    "KY", "LA", "ME", "MD", "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ", "NM", "NY",
    "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC", "SD", "TN", "TX", "UT", "VT", "VA", "WA", "WV",
    "WI", "WY",
];

const ARCHETYPES: [Archetype; 6] = [
    Archetype::PartyLoyalist,
    Archetype::PragmaticInstitutionalist,
    Archetype::BipartisanDealmaker,
    Archetype::PopulistHardliner,
    Archetype::CommitteeTechnocrat,
    Archetype::ElectoralModerate,
];

#[derive(Clone, Copy)]
enum Archetype {
    PartyLoyalist,
    PragmaticInstitutionalist,
    BipartisanDealmaker,
    PopulistHardliner,
    CommitteeTechnocrat,
    ElectoralModerate,
}

pub fn build_synthetic_senate() -> Vec<Senator> {
    (0..100)
        .map(|index| build_synthetic_senator(index))
        .collect()
}

fn build_synthetic_senator(index: usize) -> Senator {
    let party = synthetic_party(index);
    let archetype = ARCHETYPES[index % ARCHETYPES.len()];
    let state = STATE_CODES[index % STATE_CODES.len()];
    let senate_class = match index % 3 {
        0 => SenateClass::I,
        1 => SenateClass::II,
        _ => SenateClass::III,
    };
    let reelection_year = match senate_class {
        SenateClass::I => 2028,
        SenateClass::II => 2030,
        SenateClass::III => 2032,
    };
    let offset = ((index % 7) as f32 - 3.0) / 20.0;

    let (ideology, loyalty, bipartisanship) = archetype_identity(party.clone(), archetype, offset);
    let issue_preferences = issue_preferences(party.clone(), archetype, ideology, offset);
    let procedural = procedural_profile(archetype, loyalty, bipartisanship, offset);
    let dynamic_state = dynamic_state(
        party.clone(),
        archetype,
        &issue_preferences,
        &procedural,
        loyalty,
        offset,
    );

    Senator {
        identity: Identity {
            senator_id: format!("sen_{:03}", index + 1),
            full_name: format!("Synthetic Senator {:03}", index + 1),
            party,
            state: state.to_string(),
            class: senate_class,
            start_date: NaiveDate::from_ymd_opt(2023, 1, 3).unwrap(),
            end_date: Some(NaiveDate::from_ymd_opt(reelection_year + 1, 1, 3).unwrap()),
        },
        structural: Structural {
            ideology_score: ideology,
            party_loyalty_baseline: loyalty,
            bipartisanship_baseline: bipartisanship,
            committee_assignments: committee_assignments(archetype),
            reelection_year: Some(reelection_year),
            electoral_vulnerability: clamp_unit(match archetype {
                Archetype::ElectoralModerate => 0.62 - offset * 0.3,
                Archetype::BipartisanDealmaker => 0.46 + offset.abs() * 0.1,
                _ => 0.30 + offset.abs() * 0.12,
            }),
        },
        issue_preferences,
        procedural,
        dynamic_state,
        feature_coverage_score: None,
        feature_notes: vec![],
    }
}

fn synthetic_party(index: usize) -> Party {
    if index < 49 {
        Party::Democrat
    } else if index < 98 {
        Party::Republican
    } else {
        Party::Independent
    }
}

fn archetype_identity(party: Party, archetype: Archetype, offset: f32) -> (f32, f32, f32) {
    let base_ideology = match party {
        Party::Democrat => match archetype {
            Archetype::PartyLoyalist => -0.58,
            Archetype::PragmaticInstitutionalist => -0.28,
            Archetype::BipartisanDealmaker => -0.08,
            Archetype::PopulistHardliner => -0.42,
            Archetype::CommitteeTechnocrat => -0.18,
            Archetype::ElectoralModerate => 0.02,
        },
        Party::Republican => match archetype {
            Archetype::PartyLoyalist => 0.62,
            Archetype::PragmaticInstitutionalist => 0.30,
            Archetype::BipartisanDealmaker => 0.10,
            Archetype::PopulistHardliner => 0.78,
            Archetype::CommitteeTechnocrat => 0.26,
            Archetype::ElectoralModerate => 0.06,
        },
        Party::Independent | Party::Other(_) => match archetype {
            Archetype::PartyLoyalist => 0.05,
            Archetype::PragmaticInstitutionalist => 0.00,
            Archetype::BipartisanDealmaker => -0.02,
            Archetype::PopulistHardliner => 0.20,
            Archetype::CommitteeTechnocrat => -0.04,
            Archetype::ElectoralModerate => 0.08,
        },
    };

    let party_loyalty = match archetype {
        Archetype::PartyLoyalist => 0.90,
        Archetype::PragmaticInstitutionalist => 0.68,
        Archetype::BipartisanDealmaker => 0.44,
        Archetype::PopulistHardliner => 0.84,
        Archetype::CommitteeTechnocrat => 0.60,
        Archetype::ElectoralModerate => 0.56,
    };

    let bipartisanship = match archetype {
        Archetype::PartyLoyalist => 0.20,
        Archetype::PragmaticInstitutionalist => 0.46,
        Archetype::BipartisanDealmaker => 0.78,
        Archetype::PopulistHardliner => 0.18,
        Archetype::CommitteeTechnocrat => 0.50,
        Archetype::ElectoralModerate => 0.64,
    };

    (
        clamp_signed(base_ideology + offset),
        clamp_unit(party_loyalty - offset.abs() * 0.2),
        clamp_unit(bipartisanship + offset * 0.2),
    )
}

fn issue_preferences(
    party: Party,
    archetype: Archetype,
    ideology: f32,
    offset: f32,
) -> IssuePreferences {
    let party_energy = match party {
        Party::Democrat => -0.55,
        Party::Republican => 0.40,
        Party::Independent | Party::Other(_) => -0.10,
    };
    let party_tax = match party {
        Party::Democrat => -0.25,
        Party::Republican => 0.50,
        Party::Independent | Party::Other(_) => 0.05,
    };

    let dealmaking_adjustment = match archetype {
        Archetype::BipartisanDealmaker => -0.18,
        Archetype::ElectoralModerate => -0.10,
        Archetype::CommitteeTechnocrat => -0.06,
        Archetype::PragmaticInstitutionalist => -0.04,
        Archetype::PartyLoyalist => 0.0,
        Archetype::PopulistHardliner => 0.14,
    };

    IssuePreferences {
        defense: clamp_signed(
            ideology * 0.30
                + if matches!(party, Party::Republican) {
                    0.30
                } else {
                    -0.05
                }
                + offset,
        ),
        immigration: clamp_signed(
            ideology * 0.55
                + if matches!(party, Party::Republican) {
                    0.22
                } else {
                    -0.16
                }
                - offset * 0.5,
        ),
        energy_climate: clamp_signed(
            -party_energy - ideology * 0.35 + dealmaking_adjustment - offset * 0.4,
        ),
        labor: clamp_signed(
            -ideology * 0.45
                + if matches!(party, Party::Democrat) {
                    0.18
                } else {
                    -0.18
                },
        ),
        healthcare: clamp_signed(
            -ideology * 0.40
                + if matches!(party, Party::Democrat) {
                    0.22
                } else {
                    -0.15
                },
        ),
        tax_spending: clamp_signed(party_tax + ideology * 0.25 + offset * 0.6),
        judiciary: clamp_signed(
            ideology * 0.65
                + if matches!(party, Party::Republican) {
                    0.10
                } else {
                    -0.08
                },
        ),
        trade: clamp_signed(offset * 0.6),
        tech_privacy: clamp_signed(
            -ideology * 0.15
                + match archetype {
                    Archetype::CommitteeTechnocrat => 0.22,
                    Archetype::BipartisanDealmaker => 0.10,
                    _ => 0.0,
                },
        ),
        foreign_policy: clamp_signed(
            ideology * 0.20
                + if matches!(archetype, Archetype::PragmaticInstitutionalist) {
                    0.12
                } else {
                    0.0
                },
        ),
    }
}

fn procedural_profile(
    archetype: Archetype,
    loyalty: f32,
    bipartisanship: f32,
    offset: f32,
) -> Procedural {
    let cloture = match archetype {
        Archetype::PartyLoyalist => 0.72,
        Archetype::PragmaticInstitutionalist => 0.78,
        Archetype::BipartisanDealmaker => 0.83,
        Archetype::PopulistHardliner => 0.28,
        Archetype::CommitteeTechnocrat => 0.76,
        Archetype::ElectoralModerate => 0.66,
    };

    Procedural {
        cloture_support_baseline: clamp_unit(cloture + offset * 0.2),
        motion_to_proceed_baseline: clamp_unit(cloture + 0.05),
        uc_objection_tendency: clamp_unit((1.0 - cloture) * 0.75 + offset.abs() * 0.1),
        leadership_deference: clamp_unit(
            match archetype {
                Archetype::PartyLoyalist => 0.86,
                Archetype::PragmaticInstitutionalist => 0.70,
                Archetype::BipartisanDealmaker => 0.48,
                Archetype::PopulistHardliner => 0.34,
                Archetype::CommitteeTechnocrat => 0.62,
                Archetype::ElectoralModerate => 0.54,
            } + loyalty * 0.08,
        ),
        amendment_openness: clamp_unit(
            match archetype {
                Archetype::PartyLoyalist => 0.42,
                Archetype::PragmaticInstitutionalist => 0.62,
                Archetype::BipartisanDealmaker => 0.78,
                Archetype::PopulistHardliner => 0.22,
                Archetype::CommitteeTechnocrat => 0.74,
                Archetype::ElectoralModerate => 0.68,
            } + bipartisanship * 0.08,
        ),
        attendance_reliability: clamp_unit(
            match archetype {
                Archetype::PartyLoyalist => 0.97,
                Archetype::PragmaticInstitutionalist => 0.96,
                Archetype::BipartisanDealmaker => 0.95,
                Archetype::PopulistHardliner => 0.93,
                Archetype::CommitteeTechnocrat => 0.98,
                Archetype::ElectoralModerate => 0.94,
            } - offset.abs() * 0.03,
        ),
    }
}

fn dynamic_state(
    party: Party,
    archetype: Archetype,
    issue_preferences: &IssuePreferences,
    procedural: &Procedural,
    loyalty: f32,
    offset: f32,
) -> DynamicState {
    let public_position = match archetype {
        Archetype::PartyLoyalist => {
            if matches!(party, Party::Republican) {
                PublicPosition::Oppose
            } else {
                PublicPosition::Support
            }
        }
        Archetype::PopulistHardliner => PublicPosition::Oppose,
        Archetype::BipartisanDealmaker => PublicPosition::Negotiating,
        Archetype::CommitteeTechnocrat => PublicPosition::Undeclared,
        Archetype::PragmaticInstitutionalist => PublicPosition::Negotiating,
        Archetype::ElectoralModerate => PublicPosition::Mixed,
    };

    DynamicState {
        current_public_position: public_position,
        current_substantive_support: clamp_unit((issue_preferences.energy_climate + 1.0) / 2.0),
        current_procedural_support: procedural.cloture_support_baseline,
        current_negotiability: clamp_unit(
            match archetype {
                Archetype::PartyLoyalist => 0.28,
                Archetype::PragmaticInstitutionalist => 0.54,
                Archetype::BipartisanDealmaker => 0.82,
                Archetype::PopulistHardliner => 0.18,
                Archetype::CommitteeTechnocrat => 0.58,
                Archetype::ElectoralModerate => 0.74,
            } + offset * 0.2,
        ),
        current_party_pressure: clamp_unit(match party {
            Party::Democrat | Party::Republican => loyalty * 0.78,
            Party::Independent | Party::Other(_) => 0.32,
        }),
        current_issue_salience_in_state: clamp_unit(
            0.45 + issue_preferences.energy_climate.abs() * 0.30 + offset.abs() * 0.10,
        ),
    }
}

fn committee_assignments(archetype: Archetype) -> Vec<String> {
    match archetype {
        Archetype::PartyLoyalist => vec!["Rules".to_string(), "Appropriations".to_string()],
        Archetype::PragmaticInstitutionalist => {
            vec!["Homeland Security".to_string(), "Commerce".to_string()]
        }
        Archetype::BipartisanDealmaker => {
            vec!["Finance".to_string(), "Foreign Relations".to_string()]
        }
        Archetype::PopulistHardliner => vec!["Judiciary".to_string(), "Budget".to_string()],
        Archetype::CommitteeTechnocrat => {
            vec![
                "Energy and Natural Resources".to_string(),
                "Commerce".to_string(),
            ]
        }
        Archetype::ElectoralModerate => {
            vec!["Agriculture".to_string(), "Small Business".to_string()]
        }
    }
}

fn clamp_unit(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn clamp_signed(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::NaiveDate;

    use crate::{
        BudgetaryImpact, Chamber, CongressionalSession, LegislativeContext, LegislativeObject,
        LegislativeObjectType, Party, PolicyDomain, ProceduralStage, analyze_chamber,
        assess_floor_action, build_synthetic_senate, derive_stance,
    };

    fn example_object() -> LegislativeObject {
        LegislativeObject {
            object_id: "obj_001".to_string(),
            title: "Synthetic Clean Energy Permitting Package".to_string(),
            object_type: LegislativeObjectType::Bill,
            policy_domain: PolicyDomain::EnergyClimate,
            summary: "Synthetic bill used for full-roster pipeline testing.".to_string(),
            text_embedding_placeholder: None,
            sponsor: None,
            cosponsors: vec![],
            origin_chamber: Chamber::Senate,
            introduced_date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            current_version_label: Some("Floor Draft".to_string()),
            budgetary_impact: BudgetaryImpact::Moderate,
            salience: 0.78,
            controversy: 0.64,
        }
    }

    fn example_context() -> LegislativeContext {
        LegislativeContext {
            congress_number: 119,
            session: CongressionalSession::First,
            current_chamber: Chamber::Senate,
            procedural_stage: ProceduralStage::ClotureFiled,
            majority_party: Party::Democrat,
            minority_party: Party::Republican,
            president_party: Party::Democrat,
            days_until_election: Some(120),
            days_until_deadline: Some(18),
            under_unanimous_consent: false,
            under_reconciliation: false,
            leadership_priority: 0.82,
            media_attention: 0.67,
        }
    }

    #[test]
    fn builds_valid_full_synthetic_roster() {
        let roster = build_synthetic_senate();
        assert_eq!(roster.len(), 100);

        let mut ids = HashSet::new();
        let mut democrats = 0;
        let mut republicans = 0;
        let mut independents = 0;

        for senator in &roster {
            assert!(ids.insert(senator.identity.senator_id.clone()));
            assert!(senator.validate().is_ok());

            match senator.identity.party {
                Party::Democrat => democrats += 1,
                Party::Republican => republicans += 1,
                Party::Independent => independents += 1,
                Party::Other(_) => {}
            }
        }

        assert_eq!(democrats + republicans + independents, 100);
        assert_eq!(democrats, 49);
        assert_eq!(republicans, 49);
        assert_eq!(independents, 2);
    }

    #[test]
    fn derives_valid_stances_for_full_synthetic_roster() {
        let roster = build_synthetic_senate();
        let legislative_object = example_object();
        let context = example_context();

        let stances: Vec<_> = roster
            .iter()
            .map(|senator| derive_stance(senator, &legislative_object, &context).unwrap())
            .collect();

        assert_eq!(stances.len(), 100);
        assert!(stances.iter().all(|stance| stance.validate().is_ok()));
    }

    #[test]
    fn full_roster_chamber_analysis_completes_with_sensible_counts() {
        let roster = build_synthetic_senate();
        let legislative_object = example_object();
        let context = example_context();
        let stances: Vec<_> = roster
            .iter()
            .map(|senator| derive_stance(senator, &legislative_object, &context).unwrap())
            .collect();

        let analysis = analyze_chamber(&legislative_object, &context, &stances).unwrap();
        assert_eq!(analysis.total_senators, 100);
        assert!(analysis.expected_present_count <= 100);
        assert_eq!(
            analysis.likely_support_count
                + analysis.lean_support_count
                + analysis.undecided_count
                + analysis.lean_oppose_count
                + analysis.likely_oppose_count,
            100
        );
    }

    #[test]
    fn full_roster_floor_action_assessment_validates() {
        let roster = build_synthetic_senate();
        let legislative_object = example_object();
        let context = example_context();
        let stances: Vec<_> = roster
            .iter()
            .map(|senator| derive_stance(senator, &legislative_object, &context).unwrap())
            .collect();

        let analysis = analyze_chamber(&legislative_object, &context, &stances).unwrap();
        let assessment = assess_floor_action(&legislative_object, &context, &analysis).unwrap();

        assert!(assessment.validate().is_ok());
        assert!((0.0..=1.0).contains(&assessment.confidence));
    }
}
