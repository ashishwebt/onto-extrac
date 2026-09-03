use std::collections::HashMap;

use crate::ontology::Ontology;
use serde_json::Value;

pub struct CypherAdapter {
    /// Maps `(from_label, to_label)` -> relationship name, derived from ontology edges.
    rel_types: HashMap<(String, String), String>,
}

impl CypherAdapter {
    pub fn new(ontology: &Ontology) -> Self {
        let mut rel_types = HashMap::new();
        for edge in &ontology.edges {
            rel_types
                .entry((edge.from.clone(), edge.to.clone()))
                .or_insert_with(|| edge.name.clone());
        }
        Self { rel_types }
    }

    fn records(value: &Value) -> Vec<&Value> {
        match value {
            Value::Array(values) => values.iter().collect(),
            Value::Object(_) => vec![value],
            _ => Vec::new(),
        }
    }

    fn format_value(value: &Value) -> String {
        match value {
            Value::String(value) => format!("\"{}\"", value.replace('"', "\\\"")),
            Value::Number(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Null => "null".into(),
            other => format!("\"{}\"", other.to_string().replace('"', "\\\"")),
        }
    }

    fn relationship_type(&self, from: &str, to: &str) -> &str {
        self.rel_types
            .get(&(from.to_string(), to.to_string()))
            .map(|s| s.as_str())
            .unwrap_or("RELATED_TO")
    }
}

impl Default for CypherAdapter {
    fn default() -> Self {
        Self {
            rel_types: HashMap::new(),
        }
    }
}

impl super::PersistenceAdapter for CypherAdapter {
    fn generate_queries(&self, payload: &Value) -> String {
        let Value::Object(entities) = payload else {
            return String::new();
        };

        // --- Phase 1: build an id -> label index ---
        let mut ids: HashMap<String, String> = HashMap::new();
        for (label, records) in entities {
            for record in Self::records(records) {
                if let Some(id) = record.get("id").and_then(Value::as_str) {
                    ids.insert(id.to_string(), label.clone());
                }
            }
        }

        let mut output = Vec::new();

        // --- Phase 2: MERGE nodes ---
        for (label, records) in entities {
            for record in Self::records(records) {
                let Some(object) = record.as_object() else {
                    continue;
                };

                let Some(id) = object.get("id").and_then(Value::as_str) else {
                    continue;
                };

                output.push(format!("MERGE (n:{label} {{ id: \"{id}\" }});"));

                let properties: Vec<String> = object
                    .iter()
                    .filter(|(key, _)| *key != "id" && *key != "parent_source_ids")
                    .map(|(key, value)| format!("{key}: {}", Self::format_value(value)))
                    .collect();

                if !properties.is_empty() {
                    output.push(format!("SET n += {{ {} }};", properties.join(", ")));
                }
            }
        }

        // --- Phase 3: MERGE edges ---
        for (label, records) in entities {
            for record in Self::records(records) {
                let Some(object) = record.as_object() else {
                    continue;
                };

                let Some(id) = object.get("id").and_then(Value::as_str) else {
                    continue;
                };

                let Some(Value::Array(parents)) = object.get("parent_source_ids") else {
                    continue;
                };

                for parent in parents {
                    let Some(parent_id) = parent.as_str() else {
                        continue;
                    };

                    let Some(parent_label) = ids.get(parent_id) else {
                        continue;
                    };

                    let rel = self.relationship_type(label, parent_label);

                    // Check if the edge in the ontology carries properties.
                    let edge_has_props = self
                        .rel_types
                        .get(&(label.to_string(), parent_label.to_string()))
                        .is_some();

                    // For now, emit a simple MERGE without edge properties since
                    // the extraction payload doesn't carry per-relationship metadata.
                    output.push(format!(
                        "MATCH (a:{label} {{ id: \"{id}\" }}),\n      (b:{parent_label} {{ id: \"{parent_id}\" }})\nMERGE (a)-[:{rel}]->(b);"
                    ));

                    let _ = edge_has_props;
                }
            }
        }

        output.join("\n\n")
    }
}
