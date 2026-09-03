//! LLM-based structured extraction layer.
//!
//! This module turns an [`Ontology`](crate::ontology::Ontology) into a dynamic
//! BAML schema at runtime and uses it to extract structured entities from
//! unstructured text via a BAML-backed LLM client.

pub mod baml;

#[cfg(test)]
mod tests;

use serde_json::Value;

/// A type that can extract structured information from a piece of text.
pub trait Extractor {
    /// Extract structured data from `text`, returning a generic JSON value.
    fn extract(&self, text: &str) -> Result<Value, Box<dyn std::error::Error>>;
}

pub use baml::BamlExtractor;
