use std::env;
use std::io::Write;

use onto_extra::{BamlExtractor, Extractor, Ontology};

fn usage() -> ! {
    eprintln!("usage: onto_extra <ontology.jsonld> [<text>]");
    std::process::exit(1)
}

fn load_ontology(path: &str) -> Ontology {
    let onto = Ontology::from_file(path).unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });
    onto.validate().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });
    onto
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("error: missing ontology path");
        usage();
    }

    let ontology_path = &args[1];
    let ontology = load_ontology(ontology_path);

    // Stage 1: print the JSON Schema derived from the ontology.
    println!("=== JSON Schema ===");
    let schema = ontology.to_json_pretty().expect("failed to serialize");
    println!("{schema}");

    // Stage 2: print the dynamic BAML classes derived from the ontology.
    let extractor = BamlExtractor::new(&ontology);
    let baml_schema = extractor.generate_schema();
    println!("\n=== BAML Classes ===");
    println!("{baml_schema}");

    // Stage 3: (optional) extract entities from the given text.
    let text = match args.get(2) {
        None => {
            std::io::stdout().flush().ok();
            eprintln!("\n(no text provided; skipping extraction)");
            return;
        }
        Some(arg) if arg == "--file" => {
            let path = args.get(3).expect("missing file path after --file");
            std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("error reading '{path}': {e}");
                std::process::exit(1);
            })
        }
        Some(arg) => arg.clone(),
    };

    dotenvy::dotenv().ok();

    println!("\n=== Extraction ===");
    let value = extractor.extract(&text).unwrap_or_else(|e| {
        eprintln!("extraction error: {e}");
        std::process::exit(1);
    });
    println!("{}", serde_json::to_string_pretty(&value).expect("serialize"));
}
