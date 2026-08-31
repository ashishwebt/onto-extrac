#[cfg(test)]
mod tests;

mod ontology;

pub use ontology::{Edge, Node, Ontology, OntologyError, Property, PropertyRange};
