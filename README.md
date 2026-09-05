# onto-extrac

[日本語](./README.ja.md)

`onto-extrac` is a Rust web service that extracts entities and relationships from free text according to a user-defined ontology, and persists the result as a knowledge graph in Neo4j.

You describe your domain as a JSON-LD ontology (classes, properties, and relations), point the service at some text, and it:

1. Uses an LLM (via [BAML](https://www.boundaryml.com/)) to extract entities and relations from the text that conform to your ontology.
2. Translates the extracted data into Cypher statements.
3. Writes the resulting nodes and edges into a Neo4j graph database.

It's exposed as a small HTTP API built on [Axum](https://github.com/tokio-rs/axum), with interactive OpenAPI/Swagger docs included.

## How it works

```
        text                ontology.jsonld
          │                        │
          ▼                        ▼
   ┌─────────────┐          ┌─────────────┐
   │ BAML         │  uses   │ Ontology     │
   │ Extractor    │◄────────│ (classes,    │
   │ (LLM-based)  │         │ properties)  │
   └──────┬───────┘         └─────────────┘
          │ extracted entities/relations (JSON)
          ▼
   ┌─────────────┐
   │ Cypher       │
   │ Adapter      │
   └──────┬───────┘
          │ generated Cypher
          ▼
   ┌─────────────┐
   │  Neo4j       │
   │  (graph DB)  │
   └─────────────┘
```

## Project layout

```
.
├── Cargo.toml           # Rust crate manifest
├── ontology.jsonld       # Example ontology (Company / Person / Skill)
├── neo4j/
│   └── docker-compose.yml  # Local Neo4j instance for development
├── src/
│   ├── main.rs           # Axum HTTP server & route handlers
│   ├── lib.rs             # Library entry point / public exports
│   ├── baml_client/       # Generated/BAML LLM client code
│   ├── extraction.rs      # BamlExtractor: text -> structured entities
│   ├── ontology.rs        # Ontology model, loading & validation
│   ├── persistence.rs     # CypherAdapter: entities -> Cypher queries
│   └── neo4j.rs            # Neo4jClient: executes Cypher against Neo4j
├── tests/                # Integration tests
├── test.http             # Sample HTTP requests (for REST client tooling)
└── .env.exm              # Example environment variables
```

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (2024 edition toolchain)
- [Docker](https://www.docker.com/) (to run Neo4j locally), or an existing Neo4j instance
- An LLM API key for the BAML extractor (see [Configuration](#configuration))

## Getting started

### 1. Clone the repository

```bash
git clone https://github.com/ashishwebt/onto-extrac.git
cd onto-extrac
```

### 2. Start Neo4j

A docker-compose file is provided for local development:

```bash
docker compose -f neo4j/docker-compose.yml up -d
```

This starts Neo4j 5 Community Edition with:
- Browser UI: http://localhost:7474
- Bolt/HTTP endpoint: `7687` / `7474`
- Default credentials: `neo4j` / `letmein123`

### 3. Configure environment variables

Copy the example env file and fill in your values:

```bash
cp .env.exm .env
```

```env
GOOGLE_API_KEY=your_api_key_here
```

See [Configuration](#configuration) below for the full list of supported variables.

### 4. Run the service

```bash
cargo run
```

By default the server listens on `0.0.0.0:3200`. You should see:

```
listening on 0.0.0.0:3200
OpenAPI spec available at http://0.0.0.0:3200/openapi.json
```

### 5. Explore the API

Interactive Swagger UI is available at:

```
http://localhost:3200/docs
```

The raw OpenAPI spec is served at:

```
http://localhost:3200/openapi.json
```

Sample requests are also provided in [`test.http`](./test.http), which you can run directly from editors with a REST client extension (e.g. VS Code's REST Client, or JetBrains' HTTP client).

## Configuration

The service reads configuration from environment variables (loaded from `.env` via `dotenvy`):

| Variable | Default | Description |
|---|---|---|
| `GOOGLE_API_KEY` | — | API key used by the BAML extraction client |
| `ONTOLOGY_PATH` | `ontology.jsonld` | Path to the JSON-LD ontology file to load and validate on startup |
| `NEO4J_URI` | `http://localhost:7474` | Neo4j HTTP endpoint used for executing Cypher |
| `NEO4J_USER` | `neo4j` | Neo4j username |
| `NEO4J_PASSWORD` | `letmein123` | Neo4j password |
| `LISTEN_ADDR` | `0.0.0.0:3200` | Address/port the Axum server binds to |

The ontology is validated on startup; if it's missing or invalid, the service exits with an error.

## API endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Health check, returns `{"status": "ok"}` |
| `POST` | `/extract` | Extract entities/relations from `text` using the ontology, generate Cypher, and write it to Neo4j |
| `POST` | `/cypher` | Execute one or more raw Cypher `statements` against Neo4j |
| `GET` | `/graph` | Fetch all nodes currently stored in the graph |
| `GET` | `/docs` | Swagger UI |
| `GET` | `/openapi.json` | Raw OpenAPI spec |

### Example: extract entities from text

```bash
curl -X POST http://localhost:3200/extract \
  -H "Content-Type: application/json" \
  -d '{"text": "Jane Doe works at Acme Corp and has skills in Python and Rust."}'
```

Response:

```json
{
  "extracted": { "...": "entities and relations matching the ontology" },
  "cypher": "MERGE (p:Person {name: 'Jane Doe'}) ..."
}
```

### Example: query the graph

```bash
curl http://localhost:3200/graph
```

### Example: run raw Cypher

```bash
curl -X POST http://localhost:3200/cypher \
  -H "Content-Type: application/json" \
  -d '{"statements": ["MATCH (n) RETURN n LIMIT 10"]}'
```

## The ontology

Ontologies are defined as JSON-LD documents describing classes and their properties/relations. The bundled [`ontology.jsonld`](./ontology.jsonld) models a simple recruiting/org domain:

- **Company** — has a `name`
- **Person** — has a `name`, `worksFor` a Company, and `hasSkill` (one or more Skills)
- **Skill** — has a `name`

Swap in your own ontology (or point `ONTOLOGY_PATH` at a different file) to extract a different kind of graph from text — e.g. products and suppliers, medical entities, legal clauses, etc.

## Testing

```bash
cargo test
```

## Tech stack

- [Axum](https://github.com/tokio-rs/axum) — HTTP server framework
- [BAML](https://www.boundaryml.com/) — structured LLM extraction
- [Neo4j](https://neo4j.com/) — graph database
- [utoipa](https://github.com/juhaku/utoipa) + [Swagger UI](https://github.com/juhaku/utoipa/tree/master/utoipa-swagger-ui) — OpenAPI docs
- [Tokio](https://tokio.rs/) — async runtime
- [serde](https://serde.rs/) — serialization

## License

Licensed under the MIT License — see [LICENSE](./LICENSE) for details.
