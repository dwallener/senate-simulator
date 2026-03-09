use senate_simulator::{
    ProceduralPosture, PublicPosition, SenatorScenario, SenatorStance, StanceLabel,
    io::json::{
        load_legislative_context_from_path, load_legislative_object_from_path,
        load_senator_from_path, to_pretty_json,
    },
};

fn main() {
    let senator = load_or_exit("examples/senator_example.json", load_senator_from_path);
    let legislative_object = load_or_exit(
        "examples/legislative_object_example.json",
        load_legislative_object_from_path,
    );
    let context = load_or_exit(
        "examples/legislative_context_example.json",
        load_legislative_context_from_path,
    );

    let stance = SenatorStance {
        senator_id: senator.identity.senator_id.clone(),
        object_id: legislative_object.object_id.clone(),
        context_id: Some("ctx_119_senate_cloture".to_string()),
        substantive_support: 0.64,
        procedural_support: 0.71,
        public_support: 0.59,
        negotiability: 0.76,
        rigidity: 0.28,
        defection_probability: 0.22,
        absence_probability: 0.03,
        stance_label: StanceLabel::LeanSupport,
        procedural_posture: ProceduralPosture::SupportCloture,
        public_position: PublicPosition::Negotiating,
        top_factors: vec![
            "high alignment with leadership energy agenda".to_string(),
            "permitting provisions improve home-state infrastructure outlook".to_string(),
            "still negotiating labor-side implementation details".to_string(),
        ],
    };

    if let Err(error) = stance.validate() {
        eprintln!("Failed to validate senator stance: {error}");
        std::process::exit(1);
    }

    let scenario = SenatorScenario {
        senator,
        legislative_object,
        context,
        stance,
    };

    println!("Loaded and validated senator scenario from example JSON files");
    match to_pretty_json(&scenario) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("Failed to serialize scenario for display: {error}");
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
