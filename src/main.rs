use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use onto_extra::{
    BamlExtractor, CypherAdapter, Extractor, Neo4jClient, Ontology, PersistenceAdapter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

struct AppState {
    ontology: Ontology,
    neo4j: Neo4jClient,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct ExtractRequest {
    /// Text to extract entities and relations from
    text: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct CypherRequest {
    /// Cypher statements to execute
    statements: Vec<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ExtractResponse {
    /// Extracted entities and relations
    extracted: Value,
    /// Generated Cypher query
    cypher: String,
}

#[derive(Serialize, utoipa::ToSchema)]
struct GraphResponse {
    /// Graph nodes from Neo4j
    nodes: Vec<Value>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct CypherResponse {
    /// Result from Cypher execution
    result: Value,
}

/// Extract entities and relations from text using BAML
#[utoipa::path(
    post,
    path = "/extract",
    request_body = ExtractRequest,
    responses(
        (status = 200, description = "Extraction result", body = ExtractResponse),
        (status = 500, description = "Internal server error")
    )
)]
async fn extract_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExtractRequest>,
) -> Result<Json<ExtractResponse>, (StatusCode, String)> {
    let extractor = BamlExtractor::new(&state.ontology);
    let extracted = extractor
        .extract(&payload.text)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("extraction error: {e}")))?;

    let adapter = CypherAdapter::new(&state.ontology);
    let cypher = adapter.generate_queries(&extracted);

    if !cypher.is_empty() {
        state
            .neo4j
            .execute(&cypher)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("neo4j error: {e}")))?;
    }

    Ok(Json(ExtractResponse { extracted, cypher }))
}

/// Execute Cypher statements against Neo4j
#[utoipa::path(
    post,
    path = "/cypher",
    request_body = CypherRequest,
    responses(
        (status = 200, description = "Execution result", body = CypherResponse),
        (status = 500, description = "Internal server error")
    )
)]
async fn cypher_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CypherRequest>,
) -> Result<Json<CypherResponse>, (StatusCode, String)> {
    let combined = payload.statements.join(";\n");
    let result = state
        .neo4j
        .execute(&combined)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("neo4j error: {e}")))?;

    Ok(Json(CypherResponse { result }))
}

/// Fetch all nodes from the graph
#[utoipa::path(
    get,
    path = "/graph",
    responses(
        (status = 200, description = "Graph nodes", body = GraphResponse),
        (status = 500, description = "Internal server error")
    )
)]
async fn graph_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GraphResponse>, (StatusCode, String)> {
    let nodes = state
        .neo4j
        .fetch_all()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("neo4j error: {e}")))?;

    Ok(Json(GraphResponse { nodes }))
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "OK"))
)]
async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[derive(OpenApi)]
#[openapi(
    paths(extract_handler, cypher_handler, graph_handler, health_handler),
    components(schemas(ExtractRequest, ExtractResponse, CypherRequest, CypherResponse, GraphResponse))
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let ontology_path =
        std::env::var("ONTOLOGY_PATH").unwrap_or_else(|_| "ontology.jsonld".into());
    let neo4j_uri =
        std::env::var("NEO4J_URI").unwrap_or_else(|_| "http://localhost:7474".into());
    let neo4j_user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".into());
    let neo4j_pass =
        std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "letmein123".into());
    let listen_addr =
        std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3200".into());

    let ontology = Ontology::from_file(&ontology_path).unwrap_or_else(|err| {
        eprintln!("error loading ontology: {err}");
        std::process::exit(1);
    });
    ontology.validate().unwrap_or_else(|err| {
        eprintln!("error validating ontology: {err}");
        std::process::exit(1);
    });

    let neo4j = Neo4jClient::new(&neo4j_uri, &neo4j_user, &neo4j_pass);

    let state = Arc::new(AppState { ontology, neo4j });

    let app = Router::new()
        .merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .route("/health", get(health_handler))
        .route("/extract", post(extract_handler))
        .route("/cypher", post(cypher_handler))
        .route("/graph", get(graph_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .unwrap_or_else(|err| {
            eprintln!("error binding to {listen_addr}: {err}");
            std::process::exit(1);
        });

    println!("listening on {listen_addr}");
    println!("OpenAPI spec available at http://{listen_addr}/openapi.json");
    axum::serve(listener, app).await.unwrap();
}
