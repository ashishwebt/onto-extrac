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
fn generates_unwind_merge_for_nodes() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            { "id": "p1", "name": "Alice" },
            { "id": "p2", "name": "Bob" }
        ]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("UNWIND [") && cypher.contains("AS record"));
    assert!(cypher.contains("MERGE (n:Person { id: record.id })"));
    assert!(cypher.contains("ON CREATE SET n += record"));
    assert!(cypher.contains("ON MATCH SET n += record"));
}

#[test]
fn sets_properties_via_record_spread() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "name": "Alice", "age": 30 }]
    });

    let cypher = adapter.generate_queries(&payload);
    // Properties are spread from the record map, so they appear in the UNWIND list.
    assert!(cypher.contains("age: 30"));
    assert!(cypher.contains("name: \"Alice\""));
    assert!(cypher.contains("ON CREATE SET n += record"));
}

#[test]
fn generates_edge_merge_between_related_nodes() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "worksFor": { "id": "c1", "type": "Company" } }],
        "Company": [{ "id": "c1", "CompanyName": "Acme" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("[:worksFor]"));
    assert!(cypher.contains("MATCH (a:Person { id: edge.childId }),"));
    assert!(cypher.contains("(b:Company { id: edge.parentId })"));
}

#[test]
fn falls_back_to_related_to_for_unknown_pairs() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Company": [{ "id": "c1", "worksFor": { "id": "p1", "type": "Person" } }],
        "Person": [{ "id": "p1" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("[:RELATED_TO]"));
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
    assert!(cypher.contains("Alice \\\"The Great\\\""));
}

#[test]
fn escapes_newlines_in_string_values() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "bio": "Line one\nLine two" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("Line one\\nLine two"));
}

#[test]
fn escapes_backslashes_in_string_values() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "path": "C:\\Users\\test" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("C:\\\\Users\\\\test"));
}

#[test]
fn escapes_tabs_in_string_values() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "data": "col1\tcol2" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("col1\\tcol2"));
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
    assert!(cypher.contains("MERGE (n:Person { id: record.id })"));
    assert!(cypher.contains("MERGE (n:Company { id: record.id })"));
    assert!(cypher.contains("MERGE (n:Skill { id: record.id })"));
}

#[test]
fn uses_ontology_edge_names_for_relationships() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            {
                "id": "p1",
                "worksFor": { "id": "c1", "type": "Company" },
                "hasSkill": { "id": "s1", "type": "Skill" }
            }
        ],
        "Company": [{ "id": "c1" }],
        "Skill": [{ "id": "s1" }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("[:worksFor]"));
    assert!(cypher.contains("[:hasSkill]"));
}

#[test]
fn canonicalizes_lowercased_payload_labels() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    // Mirrors what the BAML extractor actually returns: node arrays are keyed
    // by the lowercased class name (person, company, skill) while EntityRef
    // `type` values use the capitalized ontology class name (Company, Skill).
    let payload = json!({
        "person": [
            {
                "id": "p1",
                "name": "Alice",
                "worksFor": { "id": "c1", "type": "Company" },
                "hasSkill": { "id": "s1", "type": "Skill" }
            }
        ],
        "company": [{ "id": "c1", "CompanyName": "Acme" }],
        "skill": [{ "id": "s1", "SkillName": "Rust" }]
    });

    let cypher = adapter.generate_queries(&payload);

    // Nodes are MERGEd with canonical (ontology) labels.
    assert!(cypher.contains("MERGE (n:Person { id: record.id })"), "{cypher}");
    assert!(cypher.contains("MERGE (n:Company { id: record.id })"), "{cypher}");
    assert!(cypher.contains("MERGE (n:Skill { id: record.id })"), "{cypher}");

    // Edges MATCH canonical labels and resolve the real relationship names.
    assert!(cypher.contains("MATCH (a:Person { id: edge.childId }),"), "{cypher}");
    assert!(cypher.contains("(b:Company { id: edge.parentId })"), "{cypher}");
    assert!(cypher.contains("(b:Skill { id: edge.parentId })"), "{cypher}");
    assert!(cypher.contains("[:worksFor]"), "{cypher}");
    assert!(cypher.contains("[:hasSkill]"), "{cypher}");
    assert!(!cypher.contains("[:RELATED_TO]"), "{cypher}");
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
    // Both records are in the same UNWIND block.
    assert!(cypher.contains("id: \"p1\""));
    assert!(cypher.contains("name: \"Alice\""));
    assert!(cypher.contains("id: \"p2\""));
    assert!(cypher.contains("name: \"Bob\""));
}

#[test]
fn handles_single_object_payload() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": { "id": "p1", "name": "Alice" }
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("MERGE (n:Person { id: record.id })"));
    assert!(cypher.contains("name: \"Alice\""));
}

#[test]
fn single_record_uses_unwind() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "name": "Alice" }]
    });

    let cypher = adapter.generate_queries(&payload);
    // Even a single record uses UNWIND for consistency.
    assert!(cypher.contains("UNWIND ["));
    assert!(cypher.contains("AS record"));
}

#[test]
fn multiple_relationship_types_produce_separate_unwind_blocks() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            {
                "id": "p1",
                "worksFor": { "id": "c1", "type": "Company" },
                "hasSkill": { "id": "s1", "type": "Skill" }
            }
        ],
        "Company": [{ "id": "c1" }],
        "Skill": [{ "id": "s1" }]
    });

    let cypher = adapter.generate_queries(&payload);
    // Two relationship types produce two separate UNWIND blocks.
    let unwind_count = cypher.matches("UNWIND [").count();
    // 3 node UNWINDs (Person, Company, Skill) + 2 edge UNWINDs (worksFor, hasSkill) = 5
    assert_eq!(unwind_count, 5);
    assert!(cypher.contains("[:worksFor]"));
    assert!(cypher.contains("[:hasSkill]"));
}

#[test]
fn batched_nodes_share_single_unwind() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [
            { "id": "p1", "name": "Alice" },
            { "id": "p2", "name": "Bob" },
            { "id": "p3", "name": "Carol" }
        ]
    });

    let cypher = adapter.generate_queries(&payload);
    // All three Person records should be in one UNWIND block.
    let unwind_count = cypher.matches("UNWIND [").count();
    assert_eq!(unwind_count, 1);
    assert!(cypher.contains("id: \"p1\""));
    assert!(cypher.contains("id: \"p2\""));
    assert!(cypher.contains("id: \"p3\""));
}

#[test]
fn edge_unwind_uses_correct_labels() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "worksFor": { "id": "c1", "type": "Company" } }],
        "Company": [{ "id": "c1" }]
    });

    let cypher = adapter.generate_queries(&payload);
    // The MATCH should use Person for a and Company for b.
    assert!(cypher.contains("MATCH (a:Person { id: edge.childId }),"));
    assert!(cypher.contains("(b:Company { id: edge.parentId })"));
}

#[test]
fn null_values_are_preserved() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "nickname": null }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("nickname: null"));
}

#[test]
fn boolean_values_are_preserved() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);

    let payload = json!({
        "Person": [{ "id": "p1", "active": true }]
    });

    let cypher = adapter.generate_queries(&payload);
    assert!(cypher.contains("active: true"));
}
