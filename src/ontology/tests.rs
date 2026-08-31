use crate::ontology::{Ontology, OntologyError, PropertyRange};
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
            "Skill": "ex:Skill",
            "worksFor": { "@id": "ex:worksFor", "@type": "@id" },
            "employs": { "@id": "ex:employs", "@type": "@id" },
            "hasSkill": { "@id": "ex:hasSkill", "@type": "@id" },
            "skillLevel": { "@id": "ex:skillLevel", "@type": "xsd:string" }
        },
        "@graph": [
            {
                "@id": "ex:Company",
                "@type": "owl:Class",
                "rdfs:label": "Company",
                "rdfs:comment": "A business or organization that employs people."
            },
            {
                "@id": "ex:Person",
                "@type": "owl:Class",
                "rdfs:label": "Person",
                "rdfs:comment": "A human individual who can work for a company and have skills."
            },
            {
                "@id": "ex:Skill",
                "@type": "owl:Class",
                "rdfs:label": "Skill",
                "rdfs:comment": "A capability or area of knowledge."
            },
            {
                "@id": "ex:worksFor",
                "@type": "owl:ObjectProperty",
                "rdfs:label": "worksFor",
                "rdfs:comment": "Person works for Company.",
                "rdfs:domain": { "@id": "ex:Person" },
                "rdfs:range": { "@id": "ex:Company" }
            },
            {
                "@id": "ex:employs",
                "@type": "owl:ObjectProperty",
                "rdfs:label": "employs",
                "rdfs:comment": "Company employs Person.",
                "rdfs:domain": { "@id": "ex:Company" },
                "rdfs:range": { "@id": "ex:Person" }
            },
            {
                "@id": "ex:hasSkill",
                "@type": "owl:ObjectProperty",
                "rdfs:label": "hasSkill",
                "rdfs:comment": "Person has Skill.",
                "rdfs:domain": { "@id": "ex:Person" },
                "rdfs:range": { "@id": "ex:Skill" }
            },
            {
                "@id": "ex:skillLevel",
                "@type": "owl:DatatypeProperty",
                "rdfs:label": "skillLevel",
                "rdfs:domain": { "@id": "ex:Person" },
                "rdfs:range": { "@id": "xsd:string" }
            }
        ]
    });

    Ontology::from_jsonld(&value).expect("sample ontology parses")
}

#[test]
fn parses_nodes_from_jsonld() {
    let ontology = sample_ontology();

    assert_eq!(ontology.node_names(), vec!["Company", "Person", "Skill"]);
    assert_eq!(ontology.edge_names(), vec!["worksFor", "employs", "hasSkill"]);

    let person = ontology.node("Person").expect("person node exists");
    assert_eq!(person.id, "https://example.com/ontology/Person");
    assert_eq!(person.description.as_deref(), Some("A human individual who can work for a company and have skills."));
}

#[test]
fn derives_edges_from_object_properties() {
    let ontology = sample_ontology();

    let works_for = &ontology.edges[0];
    assert_eq!(works_for.name, "worksFor");
    assert_eq!(works_for.from, "Person");
    assert_eq!(works_for.to, "Company");

    let employs = &ontology.edges[1];
    assert_eq!(employs.name, "employs");
    assert_eq!(employs.from, "Company");
    assert_eq!(employs.to, "Person");
}

#[test]
fn attaches_domain_properties_to_nodes() {
    let ontology = sample_ontology();

    let person = ontology.node("Person").expect("person node");
    assert_eq!(person.properties.len(), 3);
    assert_eq!(person.properties[0].range, PropertyRange::Reference("https://example.com/ontology/Company".to_string()));
    assert!(matches!(person.properties[1].range, PropertyRange::Reference(_)));
    assert_eq!(person.properties[2].range, PropertyRange::Scalar("xsd:string".to_string()));

    let company = ontology.node("Company").expect("company node");
    assert_eq!(company.properties.len(), 1);
    assert_eq!(company.properties[0].name, "employs");
}

#[test]
fn generates_keyed_schema() {
    let ontology = sample_ontology();
    let schema = ontology.to_json();

    let person = &schema["NodePerson"];
    assert_eq!(person["type"], "object");
    assert_eq!(person["properties"]["name"]["type"], "string");
    assert_eq!(person["properties"]["description"]["type"], "string");
    assert_eq!(person["properties"]["personId"]["type"], "string");
    assert_eq!(person["properties"]["worksFor"]["type"], "array");
    assert_eq!(person["properties"]["worksFor"]["items"]["$ref"], "#/companyId");
    assert_eq!(person["properties"]["hasSkill"]["items"]["$ref"], "#/skillId");
    assert_eq!(person["properties"]["skillLevel"]["type"], "string");

    let company = &schema["NodeCompany"];
    assert_eq!(company["properties"]["employs"]["items"]["$ref"], "#/personId");

    let skill = &schema["NodeSkill"];
    assert_eq!(skill["properties"].as_object().map(|m| m.len()), Some(3));

    // Each node has an id definition that edges resolve to.
    assert_eq!(schema["personId"], json!({"type": "string", "description": "id of the node"}));
    assert_eq!(schema["companyId"], json!({"type": "string", "description": "id of the node"}));
    assert_eq!(schema["skillId"], json!({"type": "string", "description": "id of the node"}));
}

#[test]
fn maps_xsd_scalars_to_json_types() {
    let value = json!({
        "@graph": [
            {
                "@id": "ex:Record",
                "@type": "owl:Class",
                "rdfs:label": "Record",
                "rdfs:domain": { "@id": "ex:Record" }
            },
            {
                "@id": "ex:count",
                "@type": "owl:DatatypeProperty",
                "rdfs:label": "count",
                "rdfs:domain": { "@id": "ex:Record" },
                "rdfs:range": { "@id": "xsd:integer" }
            },
            {
                "@id": "ex:ratio",
                "@type": "owl:DatatypeProperty",
                "rdfs:label": "ratio",
                "rdfs:domain": { "@id": "ex:Record" },
                "rdfs:range": { "@id": "xsd:float" }
            },
            {
                "@id": "ex:active",
                "@type": "owl:DatatypeProperty",
                "rdfs:label": "active",
                "rdfs:domain": { "@id": "ex:Record" },
                "rdfs:range": { "@id": "xsd:boolean" }
            }
        ]
    });

    let ontology = Ontology::from_jsonld(&value).expect("parses");
    let schema = ontology.to_json();

    assert_eq!(schema["NodeRecord"]["properties"]["count"]["type"], "integer");
    assert_eq!(schema["NodeRecord"]["properties"]["ratio"]["type"], "number");
    assert_eq!(schema["NodeRecord"]["properties"]["active"]["type"], "boolean");
}

#[test]
fn rejects_reference_to_unknown_class() {
    let value = json!({
        "@context": { "ex": "https://example.com/ontology/" },
        "@graph": [
            {
                "@id": "ex:A",
                "@type": "owl:Class",
                "rdfs:label": "A"
            },
            {
                "@id": "ex:link",
                "@type": "owl:ObjectProperty",
                "rdfs:label": "link",
                "rdfs:domain": { "@id": "ex:A" },
                "rdfs:range": { "@id": "ex:C" }
            }
        ]
    });

    let err = matches!(
        Ontology::from_jsonld(&value),
        Err(OntologyError::UnknownReference { property, reference, .. })
            if property == "link" && reference == "https://example.com/ontology/C"
    );
    assert!(err);
}

#[test]
fn errors_on_missing_graph() {
    let value = json!({ "@context": {} });
    assert!(matches!(Ontology::from_jsonld(&value), Err(OntologyError::MissingGraph)));
}
