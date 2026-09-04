use onto_extra::{CypherAdapter, Ontology, PersistenceAdapter};
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

fn sample_payload() -> serde_json::Value {
    json!({
        "Person": [
            {
                "id": "person-1",
                "name": "Alice Johnson",
                "age": 30,
                "parent_source_ids": ["company-1", "skill-1", "skill-2"]
            },
            {
                "id": "person-2",
                "name": "Bob Smith",
                "age": 25,
                "parent_source_ids": ["company-1"]
            }
        ],
        "Company": [
            {
                "id": "company-1",
                "CompanyName": "Acme Corp",
                "industry": "Technology"
            }
        ],
        "Skill": [
            {
                "id": "skill-1",
                "SkillName": "Rust"
            },
            {
                "id": "skill-2",
                "SkillName": "Neo4j"
            }
        ]
    })
}

#[test]
fn generate_cypher_for_neo4j() {
    let ontology = sample_ontology();
    let adapter = CypherAdapter::new(&ontology);
    let payload = sample_payload();
    let cypher = adapter.generate_queries(&payload);

    assert!(!cypher.is_empty());
    println!("=== Generated Cypher ===");
    println!("{cypher}");
    println!("========================");
}
