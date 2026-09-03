use serde_json::{json, Map, Value};
use std::collections::HashMap;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// A class in the ontology, holding its own scalar properties and references
/// to other classes.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub properties: Vec<Property>,
}

/// A single property (datatype or object) attached to a node.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub description: Option<String>,
    pub range: PropertyRange,
}

/// Describes what a property points to.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyRange {
    /// A basic xsd scalar (e.g. `xsd:string`).
    Scalar(String),
    /// A reference to another class (the target's `Node.id`).
    Reference(String),
}

/// A directed relationship between two nodes, derived from an object property.
/// An edge can carry its own scalar properties (e.g. `skillLevel` on `hasSkill`).
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub name: String,
    pub from: String,
    pub to: String,
    pub description: Option<String>,
    pub properties: Vec<Property>,
}

/// The parsed ontology: a set of nodes and the edges between them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ontology {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Error)]
pub enum OntologyError {
    #[error("failed to read '{0}': {1}")]
    Read(String, #[source] std::io::Error),

    #[error("invalid JSON-LD: {0}")]
    InvalidJsonLd(String),

    #[error("no '@graph' array found in ontology")]
    MissingGraph,

    #[error("ontology contains no nodes")]
    EmptyOntology,

    #[error("class '{0}' is missing a label")]
    MissingLabel(String),

    #[error("property '{0}' is missing a label")]
    MissingPropertyLabel(String),

    #[error("property '{property}' on class '{class}' references unknown class '{reference}'")]
    UnknownReference {
        property: String,
        class: String,
        reference: String,
    },
}

// ---------------------------------------------------------------------------
// Parsing (JSON-LD)
// ---------------------------------------------------------------------------

const RDFS_CLASS: &str = "rdfs:Class";
const OWL_CLASS: &str = "owl:Class";
const RDF_PROPERTY: &str = "rdf:Property";
const XSD_PREFIX: &str = "xsd:";

impl Ontology {
    /// Basic sanity check on the ontology.
    pub fn validate(&self) -> Result<(), OntologyError> {
        if self.nodes.is_empty() {
            return Err(OntologyError::EmptyOntology);
        }
        Ok(())
    }

    /// Look up a node by its (short) name.
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.name == name)
    }

    /// All node names, in order.
    pub fn node_names(&self) -> Vec<&str> {
        self.nodes.iter().map(|node| node.name.as_str()).collect()
    }

    /// All edge names, in order.
    pub fn edge_names(&self) -> Vec<&str> {
        self.edges.iter().map(|edge| edge.name.as_str()).collect()
    }

    /// Load the ontology from the default `ontology.jsonld` file.
    pub fn load() -> Result<Self, OntologyError> {
        Self::from_file("ontology.jsonld")
    }

    /// Load the ontology from a JSON-LD file on disk.
    pub fn from_file(path: &str) -> Result<Self, OntologyError> {
        let text =
            std::fs::read_to_string(path).map_err(|err| OntologyError::Read(path.to_string(), err))?;
        Self::from_jsonld_text(&text)
    }

    /// Parse an ontology from a JSON-LD string.
    pub fn from_jsonld_text(text: &str) -> Result<Self, OntologyError> {
        let value: Value = serde_json::from_str(text)
            .map_err(|err| OntologyError::InvalidJsonLd(err.to_string()))?;
        Self::from_jsonld(&value)
    }

    /// Parse an ontology from an already-parsed JSON-LD [`Value`].
    pub fn from_jsonld(value: &Value) -> Result<Self, OntologyError> {
        let root = value
            .as_object()
            .ok_or_else(|| OntologyError::InvalidJsonLd("root must be an object".to_string()))?;

        let context = root.get("@context").and_then(Value::as_object);
        let graph = root
            .get("@graph")
            .and_then(Value::as_array)
            .ok_or(OntologyError::MissingGraph)?;

        let resolver = PrefixResolver::new(context);

        // First pass: collect class nodes, and hoist any properties nested
        // under a class's "properties" array into standalone property entries
        // (domain defaults to the containing class when not specified).
        let mut nodes = Vec::new();
        let mut nested_properties = Vec::new();
        for item in graph {
            if is_class(item) {
                nodes.push(parse_node(item, &resolver)?);
                let class_id = item.get("@id").and_then(Value::as_str);
                if let Some(props) = item.get("properties").and_then(Value::as_array) {
                    for prop in props {
                        let mut entry = prop.clone();
                        if let Some(id) = class_id {
                            let has_domain = entry
                                .get("rdfs:domain")
                                .map(|d| !d.is_null())
                                .unwrap_or(false);
                            if !has_domain {
                                entry["rdfs:domain"] = json!({ "@id": id });
                            }
                        }
                        nested_properties.push(entry);
                    }
                }
            }
        }

        if nodes.is_empty() {
            return Err(OntologyError::EmptyOntology);
        }

        let id_to_name: HashMap<&str, &str> = nodes
            .iter()
            .map(|node| (node.id.as_str(), node.name.as_str()))
            .collect();

        // Combine any free-standing property entries with the nested ones so
        // both formats are handled by the same code path.
        let mut property_entries: Vec<Value> = nested_properties;
        property_entries.extend(graph.iter().cloned());

        // Second pass: derive edges from reference-type properties, and map
        // any object/datatype properties onto their domain class.
        let (edges, properties) =
            build_properties_and_edges(&property_entries, &resolver, &id_to_name)?;

        // Attach domain-scoped properties to nodes.
        for node in &mut nodes {
            if let Some(props) = properties.get(&node.id) {
                node.properties = props.clone();
            }
        }

        Ok(Ontology { nodes, edges })
    }

    // -----------------------------------------------------------------------
    // Schema generation
    // -----------------------------------------------------------------------

    /// Build a property schema with its `rdfs:comment` attached as `description`.
    fn with_description(schema: Value, property: &Property) -> Value {
        let mut schema = schema;
        if let Some(description) = &property.description {
            schema["description"] = json!(description);
        }
        schema
    }

    /// Convert the ontology into a standard JSON Schema document. Each class
    /// becomes an entry under `$defs`; object-to-object references are emitted
    /// as a `$ref` to a shared lightweight `EntityRef` (a small `id` + `type`
    /// object) rather than embedding or inlining the full target definition.
    pub fn to_json(&self) -> Value {
        let mut defs: Map<String, Value> = Map::new();

        for node in &self.nodes {
            let mut props: Map<String, Value> = Map::new();

            props.insert("id".to_string(), json!({ "type": "string" }));

            for property in &node.properties {
                match &property.range {
                    PropertyRange::Scalar(xsd) => {
                        props.insert(
                            property.name.clone(),
                            Self::with_description(scalar_schema(xsd), property),
                        );
                    }
                    PropertyRange::Reference(_) => {
                        props.insert(
                            property.name.clone(),
                            Self::with_description(
                                json!({ "$ref": "#/$defs/EntityRef" }),
                                property,
                            ),
                        );
                    }
                }
            }

            let mut node_schema = json!({ "type": "object", "properties": props });
            if let Some(description) = &node.description {
                node_schema["description"] = json!(description);
            }
            defs.insert(node.name.clone(), node_schema);
        }

        // A lightweight reference used wherever a class points to another class.
        defs.insert(
            "EntityRef".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "type": { "type": "string" }
                },
                "required": ["id", "type"]
            }),
        );

        json!({ "$defs": defs })
    }

    /// Convert the ontology into a pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_json())
    }
}

/// Resolves prefixed terms (e.g. `ex:Person`) to full IRIs and back to short names.
struct PrefixResolver {
    /// Direct prefix -> absolute IRI base (e.g. `schema` -> `http://schema.org/`).
    prefixes: HashMap<String, String>,
    /// Term alias -> term mapping (e.g. `company` -> `schema:Organization`).
    aliases: HashMap<String, String>,
    /// Full IRI -> short (context key) name.
    short_names: HashMap<String, String>,
}

impl PrefixResolver {
    fn new(context: Option<&Map<String, Value>>) -> Self {
        let mut resolver = PrefixResolver {
            prefixes: HashMap::new(),
            aliases: HashMap::new(),
            short_names: HashMap::new(),
        };

        let Some(context) = context else {
            return resolver;
        };

        // First pass: collect prefix declarations and term aliases. A term that
        // is not itself prefixed is treated as a namespace prefix; its value is
        // either an absolute IRI (a direct prefix) or another term (an alias).
        for (key, val) in context {
            if let Value::String(s) = val {
                if key.split_once(':').is_none() {
                    if s.starts_with("http") || s.starts_with("https") {
                        resolver
                            .prefixes
                            .insert(key.clone(), s.trim_end_matches('/').to_string() + "/");
                    } else {
                        resolver.aliases.insert(key.clone(), s.clone());
                    }
                }
            }
        }

        // Second pass: resolve terms to full IRIs.
        for (key, val) in context {
            match val {
                Value::String(s) => {
                    if let Some(full) = resolver.expand(s) {
                        resolver.short_names.insert(full, key.clone());
                    }
                }
                Value::Object(obj) => {
                    if let Some(id_str) = obj.get("@id").and_then(Value::as_str) {
                        if let Some(full) = resolver.expand(id_str) {
                            resolver.short_names.insert(full, key.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        resolver
    }

    /// Resolve a namespace prefix to an absolute IRI base ending in `/`. Direct
    /// prefixes are used as-is; aliases are expanded recursively through the
    /// prefix table.
    fn resolve_base(&self, prefix: &str) -> Option<String> {
        if let Some(base) = self.prefixes.get(prefix) {
            return Some(base.clone());
        }
        if let Some(alias) = self.aliases.get(prefix) {
            return self
                .expand(alias)
                .map(|iri| iri.trim_end_matches('/').to_string() + "/");
        }
        None
    }

    fn expand(&self, term: &str) -> Option<String> {
        if term.starts_with("http://") || term.starts_with("https://") {
            return Some(term.to_string());
        }
        term.split_once(':')
            .and_then(|(prefix, local)| self.resolve_base(prefix).map(|base| format!("{base}{local}")))
    }

    /// Expand `term` to a full IRI, falling back to the raw term.
    fn resolve_iri(&self, term: &str) -> String {
        self.expand(term).unwrap_or_else(|| term.to_string())
    }

    /// Resolve `term` to a short display name, falling back to the raw term.
    fn resolve(&self, term: &str) -> String {
        let iri = self.resolve_iri(term);
        self.short_names
            .get(&iri)
            .cloned()
            .unwrap_or_else(|| term.to_string())
    }
}

fn is_class(item: &Value) -> bool {
    match item.get("@type") {
        Some(Value::String(value)) => value == RDFS_CLASS || value == OWL_CLASS,
        Some(Value::Array(values)) => values.iter().any(|value| {
            value.as_str() == Some(RDFS_CLASS) || value.as_str() == Some(OWL_CLASS)
        }),
        _ => false,
    }
}

fn parse_node(item: &Value, resolver: &PrefixResolver) -> Result<Node, OntologyError> {
    let id = resolver.resolve_iri(
        item.get("@id")
            .and_then(Value::as_str)
            .ok_or_else(|| OntologyError::InvalidJsonLd("class missing '@id'".to_string()))?,
    );

    let name = item
        .get("rdfs:label")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            item.get("@id")
                .and_then(Value::as_str)
                .map(|id| resolver.resolve(id))
        })
        .ok_or_else(|| OntologyError::MissingLabel(id.clone()))?;

    let description = item.get("rdfs:comment").and_then(Value::as_str).map(str::to_string);

    Ok(Node {
        id,
        name,
        description,
        properties: Vec::new(),
    })
}

/// Extract scalar properties nested under an edge/property entry's `"properties"`
/// array.  These become the edge's own metadata (e.g. `skillLevel` on `hasSkill`).
fn extract_edge_properties(
    item: &Value,
    resolver: &PrefixResolver,
) -> Result<Vec<Property>, OntologyError> {
    let nested = match item.get("properties").and_then(Value::as_array) {
        Some(arr) => arr,
        None => return Ok(Vec::new()),
    };

    let mut edge_props = Vec::new();
    for prop in nested {
        let label = prop
            .get("rdfs:label")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                prop.get("@id")
                    .and_then(Value::as_str)
                    .map(|id| resolver.resolve(id))
            })
            .ok_or_else(|| {
                OntologyError::MissingPropertyLabel(
                    prop.get("@id")
                        .and_then(Value::as_str)
                        .unwrap_or("<unknown>")
                        .to_string(),
                )
            })?;

        let description = prop.get("rdfs:comment").and_then(Value::as_str).map(str::to_string);

        let range = prop.get("rdfs:range");
        let (is_scalar, range_raw) = match range {
            Some(Value::String(s)) => (s.starts_with(XSD_PREFIX), s.clone()),
            Some(Value::Object(obj)) => {
                let raw = obj.get("@id").and_then(Value::as_str).ok_or_else(|| {
                    OntologyError::InvalidJsonLd(format!(
                        "edge property '{label}' has object range without '@id'"
                    ))
                })?;
                (raw.starts_with(XSD_PREFIX), raw.to_string())
            }
            _ => (true, "xsd:string".to_string()),
        };

        if is_scalar {
            edge_props.push(Property {
                name: label,
                description,
                range: PropertyRange::Scalar(range_raw),
            });
        }
    }

    Ok(edge_props)
}

/// Collect properties scoped by `rdfs:domain` and edges from reference ranges.
fn build_properties_and_edges(
    graph: &[Value],
    resolver: &PrefixResolver,
    id_to_name: &HashMap<&str, &str>,
) -> Result<(Vec<Edge>, HashMap<String, Vec<Property>>), OntologyError> {
    let mut properties: HashMap<String, Vec<Property>> = HashMap::new();
    let mut edges = Vec::new();

    for item in graph {
        let node_type = item.get("@type").and_then(Value::as_str).unwrap_or("");
        if node_type != "owl:ObjectProperty"
            && node_type != "owl:DatatypeProperty"
            && node_type != RDF_PROPERTY
        {
            continue;
        }

        let label = item
            .get("rdfs:label")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                item.get("@id").and_then(Value::as_str).map(|id| resolver.resolve(id))
            })
            .ok_or_else(|| {
                OntologyError::MissingPropertyLabel(
                    item.get("@id")
                        .and_then(Value::as_str)
                        .unwrap_or("<unknown>")
                        .to_string(),
                )
            })?;

        let description = item.get("rdfs:comment").and_then(Value::as_str).map(str::to_string);

        let domain = item
            .get("rdfs:domain")
            .and_then(|v| v.get("@id"))
            .and_then(Value::as_str)
            .map(|id| resolver.resolve_iri(id))
            .unwrap_or_default();

        // Determine the range and whether it is a scalar or a reference.
        // Scalars keep their prefixed xsd form (e.g. `xsd:string`); references
        // are expanded to a full IRI so they can be matched against node ids.
        let range = item.get("rdfs:range");
        let (is_scalar, range_raw) = match range {
            Some(Value::String(s)) => (s.starts_with(XSD_PREFIX), s.clone()),
            Some(Value::Object(obj)) => {
                let raw = obj
                    .get("@id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        OntologyError::InvalidJsonLd(format!(
                            "property '{label}' has object range without '@id'"
                        ))
                    })?;
                (raw.starts_with(XSD_PREFIX), raw.to_string())
            }
            _ => (true, "xsd:string".to_string()),
        };
        let range_iri = resolver.resolve_iri(&range_raw);

        let edge_properties = extract_edge_properties(item, resolver)?;

        let property = if is_scalar {
            Property {
                name: label.clone(),
                description: description.clone(),
                range: PropertyRange::Scalar(range_raw),
            }
        } else {
            // Reference range -> an edge between the domain class and the range class.
            let target_found = id_to_name.get(range_iri.as_str()).is_some();
            if !target_found {
                return Err(OntologyError::UnknownReference {
                    property: label.clone(),
                    class: domain.clone(),
                    reference: range_iri.clone(),
                });
            }
            let to = id_to_name[range_iri.as_str()].to_string();
            let from = id_to_name
                .get(domain.as_str())
                .map(|name| (*name).to_string())
                .unwrap_or_else(|| domain.split('/').next().unwrap_or(&domain).to_string());

            edges.push(Edge {
                name: label.clone(),
                from,
                to,
                description: description.clone(),
                properties: edge_properties,
            });

            Property {
                name: label.clone(),
                description: description.clone(),
                range: PropertyRange::Reference(label.clone()),
            }
        };

        if !domain.is_empty() {
            properties.entry(domain).or_default().push(property);
        }
    }

    Ok((edges, properties))
}

fn scalar_schema(xsd: &str) -> Value {
    let schema_type = match xsd {
        "xsd:integer" | "xsd:int" | "xsd:long" | "xsd:short" => "integer",
        "xsd:decimal" | "xsd:float" | "xsd:double" => "number",
        "xsd:boolean" => "boolean",
        _ => "string",
    };
    json!({ "type": schema_type })
}
