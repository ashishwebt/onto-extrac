use crate::ontology::Ontology;
use crate::persistence::{CypherAdapter, PersistenceAdapter};
use serde_json::json;

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
                "@id": "ex:worksFor",
                "@type": "owl:ObjectProperty",
                "rdfs:label": "worksFor",
                "rdfs:domain": { "@id": "ex:Person" },
                "rdfs:range": { "@id": "ex:Company" }
            },
            {
                "@id": "ex:hasSkill",
                "@type": "owl:ObjectProperty",
                "rdfs:label": "hasSkill",
                "rdfs:domain": { "@id": "ex:Person" },
                "rdfs:range": { "@id": "ex:Skill" }
            }
        ]
    });
    Ontology::from_jsonld(&value).expect("valid ontology")
}

#[test]
fn generates_merge_for_each_node() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            { "id": "p1", "name": "Alice" },
            { "id": "p2", "name": "Bob" }
        ]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (n:Person { id: \"p1\" });"));
    assert!(cypher.contains("MERGE (n:Person { id: \"p2\" });"));
}

#[test]
fn sets_scalar_properties_excluding_id() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "name": "Alice", "age": 30 }]
    });

    let cypher = adapter.generate_queries(&payload);
    // Properties are sorted alphabetically by serde_json's BTreeMap.
    assert!(cypher.contains("SET n += { age: 30, name: \"Alice\" };"));
    // id should not appear inside the SET clause
    assert!(!cypher.contains("SET n += { age: 30, id:"));
}

#[test]
fn generates_edge_merge_between_related_nodes() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "parent_source_ids": ["c1"] }],
        "Company": [{ "id": "c1", "CompanyName": "Acme" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (a)-[:worksFor]->(b);"));
    assert!(cypher.contains("MATCH (a:Person { id: \"p1\" }),"));
    assert!(cypher.contains("(b:Company { id: \"c1\" })"));
}

#[test]
fn falls_back_to_related_to_for_unknown_pairs() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Company": [{ "id": "c1", "parent_source_ids": ["p1"] }],
        "Person": [{ "id": "p1" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (a)-[:RELATED_TO]->(b);"));
}

#[test]
fn skips_records_without_id() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "name": "NoID" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.is_empty());
}

#[test]
fn handles_empty_payload() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);
    assert_eq!(adapter.generate_queries(&json!({})), "");
}

#[test]
fn handles_non_object_payload() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);
    assert_eq!(adapter.generate_queries(&json!("just a string")), "");
    assert_eq!(adapter.generate_queries(&json!(42)), "");
}

#[test]
fn escapes_quotes_in_string_values() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "name": "Alice \"The Great\"" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("name: \"Alice \\\"The Great\\\"\""));
}

#[test]
fn multiple_entity_types_in_payload() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "name": "Alice" }],
        "Company": [{ "id": "c1", "CompanyName": "Acme" }],
        "Skill": [{ "id": "s1", "SkillName": "Rust" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (n:Person { id: \"p1\" });"));
    assert!(cypher.contains("MERGE (n:Company { id: \"c1\" });"));
    assert!(cypher.contains("MERGE (n:Skill { id: \"s1\" });"));
}

#[test]
fn uses_ontology_edge_names_for_relationships() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            { "id": "p1", "parent_source_ids": ["c1", "s1"] }
        ],
        "Company": [{ "id": "c1" }],
        "Skill": [{ "id": "s1" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("[:worksFor]"));
    assert!(cypher.contains("[:hasSkill]"));
}

#[test]
fn handles_array_payload() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            { "id": "p1", "name": "Alice" },
            { "id": "p2", "name": "Bob" }
        ]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (n:Person { id: \"p1\" });"));
    assert!(cypher.contains("SET n += { name: \"Alice\" };"));
    assert!(cypher.contains("MERGE (n:Person { id: \"p2\" });"));
    assert!(cypher.contains("SET n += { name: \"Bob\" };"));
}

#[test]
fn handles_single_object_payload() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": { "id": "p1", "name": "Alice" }
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (n:Person { id: \"p1\" });"));
    assert!(cypher.contains("SET n += { name: \"Alice\" };"));
}
