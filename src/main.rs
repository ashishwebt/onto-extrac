use std::env;

use onto_extra::Ontology;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("usage: onto_extra <input.jsonld>");

    let onto = Ontology::from_file(path).unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    onto.validate().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    let result = onto.to_json_pretty().expect("failed to serialize");
    println!("{}", result);
}
