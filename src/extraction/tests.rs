//! Tests for the dynamic BAML extraction layer.

use crate::extraction::BamlExtractor;
use crate::ontology::Ontology;
use serde_json::json;

/// A small ontology with scalar and reference properties shared by the tests.
fn sample_ontology() -> Ontology {
    let value = json!({
        "@context": {
            "schema": "https://schema.org/",
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "owl": "http://www.w3.org/2002/07/owl#",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "ex": "https://example.com/ontology/",
            "Company": "ex:Company",
            "Person": "ex:Person",
            "Skill": "ex:Skill"
        },
        "@graph": [
            { "@id": "ex:Company", "@type": "owl:Class", "rdfs:label": "Company" },
            { "@id": "ex:Person", "@type": "owl:Class", "rdfs:label": "Person" },
            { "@id": "ex:Skill", "@type": "owl:Class", "rdfs:label": "Skill" },
            {
                "@id": "ex:name",
                "@type": "owl:DatatypeProperty",
                "rdfs:label": "name",
                "rdfs:range": { "@id": "xsd:string" },
                "rdfs:domain": { "@id": "ex:Person" }
            },
            {
                "@id": "ex:age",
                "@type": "owl:DatatypeProperty",
                "rdfs:label": "age",
                "rdfs:range": { "@id": "xsd:integer" },
                "rdfs:domain": { "@id": "ex:Person" }
            }
        ]
    });
    Ontology::from_jsonld(&value).expect("valid ontology")
}

#[test]
fn generates_class_per_node() {
    let ontology = sample_ontology();
    let schema = BamlExtractor::new(&ontology).generate_schema();

    for name in ["Person", "Company", "Skill"] {
        assert!(
            schema.contains(&format!("class {name} {{")),
            "expected class {name} in schema:\n{schema}"
        );
    }
}

#[test]
fn generates_dynamic_result_with_aggregate_arrays() {
    let ontology = sample_ontology();
    let schema = BamlExtractor::new(&ontology).generate_schema();

    assert!(schema.contains("dynamic class ExtractionResult {"));
    for name in ["Person", "Company", "Skill"] {
        let field = {
            let mut chars = name.chars();
            let first = chars.next().unwrap();
            first.to_lowercase().collect::<String>() + chars.as_str()
        };
        assert!(
            schema.contains(&format!("{field} {name}[]")),
            "expected aggregate field for {name} in schema:\n{schema}"
        );
    }
}

#[test]
fn maps_xsd_scalars_to_baml_types() {
    let ontology = sample_ontology();
    let schema = BamlExtractor::new(&ontology).generate_schema();

    // Person has `name string` (xsd:string) and `age int` (xsd:integer).
    assert!(schema.contains("  name string"), "schema:\n{schema}");
    assert!(schema.contains("  age int"), "schema:\n{schema}");
}

#[test]
fn adds_id_field_to_every_entity() {
    let ontology = sample_ontology();
    let schema = BamlExtractor::new(&ontology).generate_schema();
    // Each entity class should carry a stable `id string` field.
    let count = schema.matches("  id string\n").count();
    assert_eq!(count, ontology.nodes.len(), "schema:\n{schema}");
}

#[test]
fn generated_schema_is_valid_baml() {
    let ontology = sample_ontology();
    let schema = BamlExtractor::new(&ontology).generate_schema();

    // Injecting into a TypeBuilder validates it without hitting the network.
    let tb = crate::baml_client::TypeBuilder::new();
    assert!(tb.add_baml(&schema).is_ok(), "schema should be valid BAML:\n{schema}");
}
