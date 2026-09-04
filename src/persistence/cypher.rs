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

    /// Escape a string value for safe embedding in a Cypher string literal.
    fn escape_value(value: &Value) -> String {
        match value {
            Value::String(s) => {
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t");
                format!("\"{escaped}\"")
            }
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
            Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(Self::escape_value).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Object(obj) => {
                let entries: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("{}: {}", Self::cypher_key(k), Self::escape_value(v)))
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
        }
    }

    /// Format a property key for use in Cypher map literals.
    /// Valid identifiers are emitted bare; others are backtick-quoted.
    fn cypher_key(key: &str) -> String {
        let is_valid_ident = !key.is_empty()
            && key.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && key.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if is_valid_ident {
            key.to_string()
        } else {
            let escaped = key.replace('`', "``");
            format!("`{escaped}`")
        }
    }

    /// Sanitize a string to be a valid Cypher identifier (label or relationship type).
    /// Replaces non-alphanumeric/underscore characters with underscores and ensures
    /// the result starts with a letter or underscore.
    fn sanitize_identifier(s: &str) -> String {
        let sanitized: String = s
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if sanitized.is_empty() || sanitized.starts_with(|c: char| c.is_ascii_digit()) {
            format!("_{sanitized}")
        } else {
            sanitized
        }
    }

    fn relationship_type(&self, from: &str, to: &str) -> String {
        self.rel_types
            .get(&(from.to_string(), to.to_string()))
            .cloned()
            .unwrap_or_else(|| "RELATED_TO".to_string())
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

        // --- Phase 2: MERGE nodes (batched by label with UNWIND) ---
        for (label, records) in entities {
            let valid_records: Vec<&Value> = Self::records(records)
                .into_iter()
                .filter(|r| {
                    r.get("id")
                        .and_then(Value::as_str)
                        .is_some()
                })
                .collect();

            if valid_records.is_empty() {
                continue;
            }

            let safe_label = Self::sanitize_identifier(label);

            // Build the list of maps for UNWIND.
            let maps: Vec<String> = valid_records
                .iter()
                .map(|record| {
                    let obj = record.as_object().unwrap();
                    let entries: Vec<String> = obj
                        .iter()
                        .map(|(k, v)| {
                            format!(
                                "{}: {}",
                                Self::cypher_key(k),
                                Self::escape_value(v)
                            )
                        })
                        .collect();
                    format!("{{{}}}", entries.join(", "))
                })
                .collect();

            output.push(format!(
                "UNWIND [{}] AS record\n\
                 MERGE (n:{safe_label} {{ id: record.id }})\n\
                 ON CREATE SET n += record\n\
                 ON MATCH SET n += record;",
                maps.join(", ")
            ));
        }

        // --- Phase 3: MERGE edges (batched by label pair with UNWIND) ---
        // Group edge data by (child_label, parent_label) for one UNWIND per
        // distinct relationship type.
        let mut edge_groups: HashMap<(&str, &str), Vec<(&str, &str)>> = HashMap::new();

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

                    edge_groups
                        .entry((label.as_str(), parent_label.as_str()))
                        .or_default()
                        .push((id, parent_id));
                }
            }
        }

        for ((child_label, parent_label), edges) in &edge_groups {
            let safe_child = Self::sanitize_identifier(child_label);
            let safe_parent = Self::sanitize_identifier(parent_label);
            let rel = self.relationship_type(child_label, parent_label);
            let safe_rel = Self::sanitize_identifier(&rel);

            let maps: Vec<String> = edges
                .iter()
                .map(|(child_id, parent_id)| {
                    format!(
                        "{{childId: \"{}\", parentId: \"{}\"}}",
                        child_id.replace('\\', "\\\\").replace('"', "\\\""),
                        parent_id.replace('\\', "\\\\").replace('"', "\\\"")
                    )
                })
                .collect();

            output.push(format!(
                "UNWIND [{}] AS edge\n\
                 MATCH (a:{safe_child} {{ id: edge.childId }}),\n\
                 (b:{safe_parent} {{ id: edge.parentId }})\n\
                 MERGE (a)-[:{safe_rel}]->(b);",
                maps.join(", ")
            ));
        }

        output.join("\n\n")
    }
}
