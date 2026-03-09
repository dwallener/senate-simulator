use senate_simulator::{
    SenatorScenario, derive_stance,
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

    let stance = match derive_stance(&senator, &legislative_object, &context) {
        Ok(stance) => stance,
        Err(error) => {
            eprintln!("Failed to derive senator stance: {error}");
            std::process::exit(1);
        }
    };

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
