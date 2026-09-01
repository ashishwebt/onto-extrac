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
    assert!(works_for.properties.is_empty());

    let employs = &ontology.edges[1];
    assert_eq!(employs.name, "employs");
    assert_eq!(employs.from, "Company");
    assert_eq!(employs.to, "Person");
    assert!(employs.properties.is_empty());
}

#[test]
fn attaches_domain_properties_to_nodes() {
    let ontology = sample_ontology();

    let person = ontology.node("Person").expect("person node");
    assert_eq!(person.properties.len(), 3);
    assert_eq!(person.properties[0].range, PropertyRange::Reference("worksFor".to_string()));
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

    let person = &schema["Person"];
    assert_eq!(person["type"], "object");
    assert_eq!(person["properties"]["worksFor"]["type"], "array");
    assert_eq!(person["properties"]["worksFor"]["items"]["$ref"], "#/worksFor");
    assert_eq!(person["properties"]["worksFor"]["description"], json!("Person works for Company."));
    assert_eq!(person["properties"]["hasSkill"]["items"]["$ref"], "#/hasSkill");
    assert_eq!(person["properties"]["skillLevel"]["type"], "string");

    let company = &schema["Company"];
    assert_eq!(company["properties"]["employs"]["items"]["$ref"], "#/employs");

    // Edges are now top-level entries with source/target.
    let works_for_edge = &schema["worksFor"];
    assert_eq!(works_for_edge["type"], "object");
    assert_eq!(works_for_edge["properties"]["source"]["$ref"], "#/Person");
    assert_eq!(works_for_edge["properties"]["target"]["$ref"], "#/Company");
    assert_eq!(works_for_edge["description"], json!("Person works for Company."));

    let employs_edge = &schema["employs"];
    assert_eq!(employs_edge["properties"]["source"]["$ref"], "#/Company");
    assert_eq!(employs_edge["properties"]["target"]["$ref"], "#/Person");

    let has_skill_edge = &schema["hasSkill"];
    assert_eq!(has_skill_edge["properties"]["source"]["$ref"], "#/Person");
    assert_eq!(has_skill_edge["properties"]["target"]["$ref"], "#/Skill");
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

    assert_eq!(schema["Record"]["properties"]["count"]["type"], "integer");
    assert_eq!(schema["Record"]["properties"]["ratio"]["type"], "number");
    assert_eq!(schema["Record"]["properties"]["active"]["type"], "boolean");
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

fn nested_ontology() -> &'static str {
    // Mirrors the on-disk `ontology.jsonld`: classes carry a nested
    // "properties" array and the context aliases terms rather than absolute IRIs.
    r#"{
        "@context": {
            "schema": "http://schema.org/",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "company": "schema:Organization",
            "person": "schema:Person",
            "skill": "schema:DefinedTerm"
        },
        "@graph": [
            {
                "@id": "company:Company",
                "@type": "rdfs:Class",
                "rdfs:label": "Company",
                "rdfs:comment": "An organization that employs or is associated with persons.",
                "properties": [
                    { "@id": "company:name", "@type": "rdf:Property", "rdfs:label": "Company Name", "rdfs:comment": "The official name of the company." }
                ]
            },
            {
                "@id": "person:Person",
                "@type": "rdfs:Class",
                "rdfs:label": "Person",
                "rdfs:comment": "An individual associated with a company and possessing skills.",
                "properties": [
                    { "@id": "person:name", "@type": "rdf:Property", "rdfs:label": "Person Name", "rdfs:comment": "The full name of the person." },
                    {
                        "@id": "person:worksFor",
                        "@type": "rdf:Property",
                        "rdfs:label": "Works For",
                        "rdfs:comment": "The company the person belongs to.",
                        "rdfs:domain": { "@id": "person:Person" },
                        "rdfs:range": { "@id": "company:Company" }
                    },
                    {
                        "@id": "person:hasSkill",
                        "@type": "rdf:Property",
                        "rdfs:label": "Has Skill",
                        "rdfs:comment": "Specific skills the person possesses.",
                        "rdfs:domain": { "@id": "person:Person" },
                        "rdfs:range": { "@id": "skill:Skill" }
                    }
                ]
            },
            {
                "@id": "skill:Skill",
                "@type": "rdfs:Class",
                "rdfs:label": "Skill",
                "rdfs:comment": "A capability or area of knowledge.",
                "properties": [
                    { "@id": "skill:name", "@type": "rdf:Property", "rdfs:label": "Skill Name", "rdfs:comment": "The name of the skill, e.g., 'Python'." }
                ]
            }
        ]
    }"#
}

#[test]
fn parses_nested_properties_format() {
    let ontology =
        Ontology::from_jsonld_text(nested_ontology()).expect("nested ontology parses");

    assert_eq!(ontology.node_names(), vec!["Company", "Person", "Skill"]);
    assert_eq!(ontology.edge_names(), vec!["Works For", "Has Skill"]);

    let person = ontology.node("Person").expect("person node");
    // scalar "Person Name", reference "Works For" (Company), reference "Has Skill" (Skill)
    assert_eq!(person.properties.len(), 3);
    assert_eq!(person.properties[0].name, "Person Name");
    assert!(matches!(person.properties[0].range, PropertyRange::Scalar(_)));
    assert_eq!(person.properties[1].name, "Works For");
    assert_eq!(
        person.properties[1].range,
        PropertyRange::Reference("Works For".to_string())
    );
    assert_eq!(person.properties[2].name, "Has Skill");
    assert_eq!(
        person.properties[2].range,
        PropertyRange::Reference("Has Skill".to_string())
    );

    // Scalar properties on Company and Skill.
    assert_eq!(
        ontology.node("Company").expect("company node").properties.len(),
        1
    );
    assert_eq!(
        ontology.node("Skill").expect("skill node").properties.len(),
        1
    );

    let schema = ontology.to_json();
    assert_eq!(
        schema["Company"]["description"],
        json!("An organization that employs or is associated with persons.")
    );
    assert_eq!(
        schema["Person"]["description"],
        json!("An individual associated with a company and possessing skills.")
    );
    assert_eq!(
        schema["Person"]["properties"]["Works For"]["items"]["$ref"],
        "#/Works For"
    );
    assert_eq!(
        schema["Person"]["properties"]["Has Skill"]["items"]["$ref"],
        "#/Has Skill"
    );
    assert_eq!(
        schema["Company"]["properties"]["Company Name"]["type"],
        "string"
    );
    assert_eq!(
        schema["Company"]["properties"]["Company Name"]["description"],
        json!("The official name of the company.")
    );
    assert_eq!(
        schema["Person"]["properties"]["Works For"]["description"],
        json!("The company the person belongs to.")
    );

    // Edges are top-level entries.
    assert_eq!(
        schema["Works For"]["properties"]["source"]["$ref"],
        "#/Person"
    );
    assert_eq!(
        schema["Works For"]["properties"]["target"]["$ref"],
        "#/Company"
    );
    assert_eq!(
        schema["Has Skill"]["properties"]["source"]["$ref"],
        "#/Person"
    );
    assert_eq!(
        schema["Has Skill"]["properties"]["target"]["$ref"],
        "#/Skill"
    );
}

