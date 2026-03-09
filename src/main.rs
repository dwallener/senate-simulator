use serde::Serialize;

use senate_simulator::{
    SenateAnalysis, Senator, SenatorScenario, analyze_chamber, derive_stance,
    io::json::{
        load_legislative_context_from_path, load_legislative_object_from_path,
        load_senator_from_path, to_pretty_json,
    },
};

#[derive(Debug, Serialize)]
struct DemoOutput {
    scenarios: Vec<SenatorScenario>,
    chamber_analysis: SenateAnalysis,
}

fn main() {
    let base_senator = load_or_exit("examples/senator_example.json", load_senator_from_path);
    let legislative_object = load_or_exit(
        "examples/legislative_object_example.json",
        load_legislative_object_from_path,
    );
    let context = load_or_exit(
        "examples/legislative_context_example.json",
        load_legislative_context_from_path,
    );

    let senators = synthetic_senators(&base_senator);
    let mut scenarios = Vec::with_capacity(senators.len());
    let mut stances = Vec::with_capacity(senators.len());

    for senator in senators {
        let stance = match derive_stance(&senator, &legislative_object, &context) {
            Ok(stance) => stance,
            Err(error) => {
                eprintln!(
                    "Failed to derive senator stance for {}: {error}",
                    senator.identity.senator_id
                );
                std::process::exit(1);
            }
        };

        stances.push(stance.clone());
        scenarios.push(SenatorScenario {
            senator,
            legislative_object: legislative_object.clone(),
            context: context.clone(),
            stance,
        });
    }

    let chamber_analysis = match analyze_chamber(&legislative_object, &context, &stances) {
        Ok(analysis) => analysis,
        Err(error) => {
            eprintln!("Failed to analyze chamber: {error}");
            std::process::exit(1);
        }
    };

    let output = DemoOutput {
        scenarios,
        chamber_analysis,
    };

    println!("Derived synthetic senator roster and chamber analysis");
    match to_pretty_json(&output) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("Failed to serialize demo output: {error}");
            std::process::exit(1);
        }
    }
}

fn synthetic_senators(base: &Senator) -> Vec<Senator> {
    vec![
        configured_senator(
            base,
            "sen_001",
            "Avery Brooks",
            "OR",
            senate_simulator::Party::Independent,
            0.70,
            -0.10,
            0.66,
            0.54,
            0.82,
            0.76,
            0.18,
            senate_simulator::PublicPosition::Negotiating,
        ),
        configured_senator(
            base,
            "sen_002",
            "Elena Park",
            "WA",
            senate_simulator::Party::Democrat,
            0.92,
            0.15,
            0.78,
            0.72,
            0.88,
            0.74,
            0.22,
            senate_simulator::PublicPosition::Support,
        ),
        configured_senator(
            base,
            "sen_003",
            "Marcus Hale",
            "CO",
            senate_simulator::Party::Democrat,
            0.84,
            -0.05,
            0.72,
            0.64,
            0.76,
            0.68,
            0.32,
            senate_simulator::PublicPosition::Negotiating,
        ),
        configured_senator(
            base,
            "sen_004",
            "Rina Solis",
            "NV",
            senate_simulator::Party::Democrat,
            0.58,
            -0.20,
            0.60,
            0.58,
            0.70,
            0.62,
            0.42,
            senate_simulator::PublicPosition::Undeclared,
        ),
        configured_senator(
            base,
            "sen_005",
            "Thomas Reed",
            "AZ",
            senate_simulator::Party::Republican,
            0.22,
            -0.45,
            0.64,
            0.48,
            0.62,
            0.60,
            0.46,
            senate_simulator::PublicPosition::Mixed,
        ),
        configured_senator(
            base,
            "sen_006",
            "Grace Mercer",
            "UT",
            senate_simulator::Party::Republican,
            -0.25,
            -0.55,
            0.52,
            0.40,
            0.54,
            0.50,
            0.58,
            senate_simulator::PublicPosition::Oppose,
        ),
        configured_senator(
            base,
            "sen_007",
            "Daniel Voss",
            "TX",
            senate_simulator::Party::Republican,
            -0.62,
            -0.72,
            0.38,
            0.22,
            0.35,
            0.36,
            0.80,
            senate_simulator::PublicPosition::Oppose,
        ),
        configured_senator(
            base,
            "sen_008",
            "Mina Carver",
            "ME",
            senate_simulator::Party::Independent,
            0.40,
            -0.05,
            0.68,
            0.66,
            0.84,
            0.78,
            0.28,
            senate_simulator::PublicPosition::Negotiating,
        ),
        configured_senator(
            base,
            "sen_009",
            "Leo Hart",
            "PA",
            senate_simulator::Party::Democrat,
            0.76,
            -0.12,
            0.74,
            0.70,
            0.86,
            0.72,
            0.26,
            senate_simulator::PublicPosition::Support,
        ),
        configured_senator(
            base,
            "sen_010",
            "Nadia Quinn",
            "OH",
            senate_simulator::Party::Republican,
            0.05,
            -0.28,
            0.58,
            0.44,
            0.60,
            0.56,
            0.48,
            senate_simulator::PublicPosition::Undeclared,
        ),
    ]
}

fn configured_senator(
    base: &Senator,
    senator_id: &str,
    full_name: &str,
    state: &str,
    party: senate_simulator::Party,
    energy_climate: f32,
    tax_spending: f32,
    cloture_support: f32,
    leadership_deference: f32,
    current_negotiability: f32,
    amendment_openness: f32,
    current_party_pressure: f32,
    public_position: senate_simulator::PublicPosition,
) -> Senator {
    let mut senator = base.clone();
    senator.identity.senator_id = senator_id.to_string();
    senator.identity.full_name = full_name.to_string();
    senator.identity.state = state.to_string();
    senator.identity.party = party;
    senator.issue_preferences.energy_climate = energy_climate;
    senator.issue_preferences.tax_spending = tax_spending;
    senator.structural.ideology_score = ((energy_climate + tax_spending) / 2.0).clamp(-1.0, 1.0);
    senator.structural.party_loyalty_baseline =
        (0.45 + current_party_pressure * 0.4).clamp(0.0, 1.0);
    senator.structural.bipartisanship_baseline =
        (0.85 - senator.structural.party_loyalty_baseline).clamp(0.0, 1.0);
    senator.procedural.cloture_support_baseline = cloture_support;
    senator.procedural.motion_to_proceed_baseline = (cloture_support + 0.06).clamp(0.0, 1.0);
    senator.procedural.leadership_deference = leadership_deference;
    senator.procedural.amendment_openness = amendment_openness;
    senator.procedural.uc_objection_tendency = (1.0 - cloture_support).clamp(0.0, 1.0) * 0.6;
    senator.dynamic_state.current_public_position = public_position;
    senator.dynamic_state.current_substantive_support =
        ((energy_climate + 1.0) / 2.0).clamp(0.0, 1.0);
    senator.dynamic_state.current_procedural_support = cloture_support;
    senator.dynamic_state.current_negotiability = current_negotiability;
    senator.dynamic_state.current_party_pressure = current_party_pressure;
    senator.dynamic_state.current_issue_salience_in_state =
        (0.45 + energy_climate.abs() * 0.35).clamp(0.0, 1.0);
    senator.structural.electoral_vulnerability =
        (0.20 + (1.0 - current_party_pressure) * 0.35).clamp(0.0, 1.0);
    senator.procedural.attendance_reliability =
        (0.92 - senator.structural.electoral_vulnerability * 0.08).clamp(0.0, 1.0);
    senator
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
