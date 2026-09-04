//! Dynamic BAML extractor.
//!
//! The [`BamlExtractor`] derives a BAML schema from an [`Ontology`] at runtime
//! and injects it into the BAML `Result` class (which is declared `@@dynamic`)
//! so that extracted entities match the ontology's structure. The ontology's
//! JSON Schema representation ([`Ontology::to_json`]) is the source of truth
//! for every property's BAML type.

use crate::baml_client::{B, TypeBuilder};
use crate::ontology::{Ontology, PropertyRange};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Errors that can occur while generating the BAML schema or extracting.
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    #[error("BAML error: {0}")]
    Baml(String),
    #[error("failed to serialize extraction result: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl From<baml::BamlError> for ExtractionError {
    fn from(err: baml::BamlError) -> Self {
        ExtractionError::Baml(err.to_string())
    }
}

/// Generates a dynamic BAML schema from an ontology and uses it to extract
/// structured information from unstructured text.
pub struct BamlExtractor<'a> {
    ontology: &'a Ontology,
}

impl<'a> BamlExtractor<'a> {
    pub fn new(ontology: &'a Ontology) -> Self {
        Self { ontology }
    }

    /// Generate the BAML class definitions (as a string) for every node in the
    /// ontology, plus a `dynamic class Result` that aggregates one array of
    /// entities per node type.
    pub fn generate_schema(&self) -> String {
        let mut output = String::new();

        let mut nodes_sorted = self.ontology.nodes.clone();
        nodes_sorted.sort_by(|a, b| a.name.cmp(&b.name));

        for node in &nodes_sorted {
            output.push_str(&format!("class {} {{\n", node.name));
            output.push_str("  id string\n");
            let id_desc = match &node.description {
                Some(desc) => format!(
                    "A unique identifier for this {} entity. {}",
                    node.name,
                    desc.replace('#', "\\#").replace('"', "\\\"")
                ),
                None => format!("A unique identifier for this {} entity.", node.name),
            };
            output.push_str(&format!("  @description(#\"{}\"#)\n", id_desc));

            for property in &node.properties {
                let ty = self.map_property_type(property);
                output.push_str(&format!("  {} {}\n", property.name, ty));
                if let Some(description) = &property.description {
                    output.push_str(&format!(
                        "  @description(#\"{}\"#)\n",
                        description.replace('#', "\\#")
                    ));
                }
            }

            if let Some(description) = &node.description {
                // Emit the class description as a trailing BAML comment before
                // the closing brace is skipped; BAML descriptions go on fields.
                output.push_str(&format!(
                    "  // {}\n",
                    description.replace('\n', " ")
                ));
            }

            output.push_str("}\n\n");
        }

        output.push_str("class EntityRef {\n");
        output.push_str("  id string\n");
        output.push_str("  @description(#\"The id of the referenced entity\"#)\n");
        output.push_str("  type string\n");
        output.push_str("  @description(#\"The type (class name) of the referenced entity, e.g. Company, Skill\"#)\n");
        output.push_str("}\n\n");

        output.push_str("dynamic class ExtractionResult {\n");

        for node in &nodes_sorted {
            let field = BamlExtractor::field_name(&node.name);
            output.push_str(&format!("  {} {}[]\n", field, node.name));
            if let Some(description) = &node.description {
                output.push_str(&format!(
                    "  @description(#\"{}\"#)\n",
                    description.replace('#', "\\#")
                ));
            }
        }

        output.push_str("}\n");
        output
    }

    /// Map a property to a BAML type. Scalar XSD types are mapped via the
    /// ontology's JSON Schema mapping rules; references become an `EntityRef`
    /// object (the target class id + type), mirroring how the JSON Schema
    /// output represents references.
    fn map_property_type(&self, property: &crate::ontology::Property) -> String {
        match &property.range {
            PropertyRange::Scalar(xsd) => Self::map_scalar(xsd),
            PropertyRange::Reference(_) => "EntityRef".to_string(),
        }
    }

    /// Map an XSD scalar type name to its BAML equivalent.
    fn map_scalar(xsd: &str) -> String {
        let lower = xsd.to_ascii_lowercase();
        match lower.as_str() {
            "xsd:integer" | "xsd:int" | "xsd:long" | "xsd:short" | "int" | "integer" => "int".to_string(),
            "xsd:decimal" | "xsd:float" | "xsd:double" | "float" | "double" | "number" => "float".to_string(),
            "xsd:boolean" | "boolean" | "bool" => "bool".to_string(),
            "string[]" => "string[]".to_string(),
            "int[]" | "integer[]" => "int[]".to_string(),
            "float[]" => "float[]".to_string(),
            "bool[]" | "boolean[]" => "bool[]".to_string(),
            _ => "string".to_string(),
        }
    }

    /// Convert a node name into a valid (lowercased, alpha-numeric) BAML field.
    fn field_name(name: &str) -> String {
        let mut chars = name.chars();
        let mut out = match chars.next() {
            Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        };
        out = out
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            out.insert(0, '_');
        }
        out
    }

    /// Replace every entity's LLM-generated string id with a fresh UUID and
    /// rewrite every `EntityRef` so references keep pointing at the same target.
    /// This must run before the payload is persisted so graph nodes and edges
    /// share stable, collision-free identifiers.
    pub(crate) fn assign_uuids(payload: &mut Value) {
        let Value::Object(entities) = payload else {
            return;
        };

        // Pass 1: collect every entity id, mapping each to a new UUID.
        let mut id_map: HashMap<String, String> = HashMap::new();
        for records in entities.values() {
            for record in Self::records(records) {
                if let Some(id) = record.get("id").and_then(Value::as_str) {
                    id_map
                        .entry(id.to_string())
                        .or_insert_with(|| Uuid::new_v4().to_string());
                }
            }
        }

        // Pass 2: swap entity ids and rewrite EntityRef references in place.
        for records in entities.values_mut() {
            for record in Self::records_mut(records) {
                let Some(object) = record.as_object_mut() else {
                    continue;
                };
                if let Some(old_id) = object.get("id").and_then(Value::as_str)
                    && let Some(new_id) = id_map.get(old_id)
                {
                    object.insert("id".to_string(), Value::String(new_id.clone()));
                }
                // Rewrite EntityRef objects nested anywhere inside the record.
                for member in object.values_mut() {
                    Self::remap_ref_ids(member, &id_map);
                }
            }
        }
    }

    /// Recursively rewrite the `id` of any `EntityRef`-shaped object
    /// (`{ "id": ..., "type": ... }`) using `id_map`. Other values are left
    /// untouched.
    fn remap_ref_ids(value: &mut Value, id_map: &HashMap<String, String>) {
        match value {
            Value::Object(object) => {
                if object.contains_key("type") {
                    if let Some(id) = object.get("id").and_then(Value::as_str)
                        && let Some(new_id) = id_map.get(id)
                    {
                        object.insert("id".to_string(), Value::String(new_id.clone()));
                    }
                    return;
                }
                for member in object.values_mut() {
                    Self::remap_ref_ids(member, id_map);
                }
            }
            Value::Array(array) => {
                for member in array.iter_mut() {
                    Self::remap_ref_ids(member, id_map);
                }
            }
            _ => {}
        }
    }

    /// Iterate the records of a node array: either an array of entities or a
    /// single entity object.
    fn records(value: &Value) -> Vec<&Value> {
        match value {
            Value::Array(values) => values.iter().collect(),
            Value::Object(_) => vec![value],
            _ => Vec::new(),
        }
    }

    /// Iterate the records of a node array mutably.
    fn records_mut(value: &mut Value) -> Box<dyn Iterator<Item = &mut Value> + '_> {
        match value {
            Value::Array(array) => Box::new(array.iter_mut()),
            Value::Object(_) => Box::new(std::iter::once(value)),
            _ => Box::new(std::iter::empty()),
        }
    }
}

impl super::Extractor for BamlExtractor<'_> {
    fn extract(&self, text: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let schema = self.generate_schema();

        let tb = TypeBuilder::new();
        tb.add_baml(&schema)?;

        let res = B.ExtractInfo.with_type_builder(&tb).call(text)?;

        let mut value = serde_json::to_value(&res)?;
        // Swap the LLM's placeholder ids for stable UUIDs while keeping every
        // EntityRef reference pointing at the same target.
        Self::assign_uuids(&mut value);
        Ok(value)
    }
}
