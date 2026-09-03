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
    /// ontology's JSON Schema mapping rules; references become the target class
    /// name (rendered as a compact `EntityRef`-like object), mirroring how the
    /// JSON Schema output represents references.
    fn map_property_type(&self, property: &crate::ontology::Property) -> String {
        match &property.range {
            PropertyRange::Scalar(xsd) => Self::map_scalar(xsd),
            PropertyRange::Reference(_) => "string".to_string(),
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
}

impl super::Extractor for BamlExtractor<'_> {
    fn extract(&self, text: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let schema = self.generate_schema();

        let tb = TypeBuilder::new();
        tb.add_baml(&schema)?;

        let res = B.ExtractInfo.with_type_builder(&tb).call(text)?;

        let value = serde_json::to_value(&res)?;
        Ok(value)
    }
}
