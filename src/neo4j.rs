use reqwest::Client;
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Neo4jError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Neo4j error: {0}")]
    Neo4j(String),
    #[error("unexpected response format")]
    UnexpectedResponse,
}

pub struct Neo4jClient {
    client: Client,
    url: String,
    auth: (String, String),
}

impl Neo4jClient {
    pub fn new(uri: &str, user: &str, password: &str) -> Self {
        Self {
            client: Client::new(),
            url: format!("{uri}/db/neo4j/tx/commit"),
            auth: (user.to_string(), password.to_string()),
        }
    }

    pub async fn execute(&self, cypher: &str) -> Result<Value, Neo4jError> {
        if cypher.trim().is_empty() {
            return Ok(json!({ "results": [], "summary": {} }));
        }

        let statements: Vec<Value> = cypher
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|statement| json!({ "statement": statement }))
            .collect();

        let body = json!({ "statements": statements });

        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.auth.0, Some(&self.auth.1))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await?;

        let result: Value = resp.json().await?;

        if let Some(errors) = result.get("errors").and_then(Value::as_array)
            && !errors.is_empty()
        {
            let msg = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(Neo4jError::Neo4j(msg));
        }

        Ok(result)
    }

    pub async fn fetch_all(&self) -> Result<Vec<Value>, Neo4jError> {
        let cypher = "MATCH (n) OPTIONAL MATCH (n)-[r]->(m) RETURN n, r, m";
        let result = self.execute(cypher).await?;

        let mut nodes = Vec::new();

        if let Some(results) = result.get("results").and_then(Value::as_array) {
            for row in results {
                if let Some(data) = row.get("data").and_then(Value::as_array) {
                    for entry in data {
                        if let Some(row_data) = entry.get("row").and_then(Value::as_array) {
                            // row_data[0] = source node, row_data[1] = relationship, row_data[2] = target node
                            let source = row_data.first().cloned().unwrap_or(Value::Null);
                            let rel = row_data.get(1).cloned().unwrap_or(Value::Null);
                            let target = row_data.get(2).cloned().unwrap_or(Value::Null);

                            if !source.is_null() {
                                nodes.push(json!({
                                    "source": source,
                                    "relationship": rel,
                                    "target": target
                                }));
                            }
                        }
                    }
                }
            }
        }

        Ok(nodes)
    }
}
