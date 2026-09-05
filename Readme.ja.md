# onto-extrac

[English](./README.md)

`onto-extrac` は、ユーザー定義のオントロジーに基づいてテキストからエンティティと関係性を抽出し、その結果を Neo4j にナレッジグラフとして永続化する Rust 製の Web サービスです。

ドメインを JSON-LD オントロジー（クラス、プロパティ、関係）として記述し、サービスにテキストを渡すと、以下が行われます。

1. [BAML](https://www.boundaryml.com/) を介して LLM を利用し、オントロジーに準拠する形でテキストからエンティティと関係を抽出します。
2. 抽出したデータを Cypher 文に変換します。
3. 生成されたノードとエッジを Neo4j グラフデータベースに書き込みます。

[Axum](https://github.com/tokio-rs/axum) 上に構築された小さな HTTP API として公開されており、対話的な OpenAPI / Swagger ドキュメントも含まれています。

## 仕組み

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

## プロジェクト構成

```
.
├── Cargo.toml           # Rust クレートのマニフェスト
├── ontology.jsonld       # オントロジーの例（Company / Person / Skill）
├── neo4j/
│   └── docker-compose.yml  # ローカル開発用の Neo4j インスタンス
├── src/
│   ├── main.rs           # Axum HTTP サーバーとルートハンドラ
│   ├── lib.rs             # ライブラリのエントリポイント／公開エクスポート
│   ├── baml_client/       # BAML により生成された LLM クライアントコード
│   ├── extraction.rs      # BamlExtractor: テキスト → 構造化エンティティ
│   ├── ontology.rs        # オントロジーモデル、読み込みと検証
│   ├── persistence.rs     # CypherAdapter: エンティティ → Cypher クエリ
│   └── neo4j.rs            # Neo4jClient: Neo4j に対して Cypher を実行
├── tests/                # 統合テスト
├── test.http             # サンプル HTTP リクエスト（REST クライアントツール用）
└── .env.exm              # 環境変数のサンプル
```

## 前提条件

- [Rust](https://www.rust-lang.org/tools/install)（2024 エディションのツールチェーン）
- [Docker](https://www.docker.com/)（ローカルで Neo4j を実行する場合）、または既存の Neo4j インスタンス
- BAML エクストラクター用の LLM API キー（[設定](#設定)を参照）

## はじめに

### 1. リポジトリをクローンする

```bash
git clone https://github.com/ashishwebt/onto-extrac.git
cd onto-extrac
```

### 2. Neo4j を起動する

ローカル開発用の docker-compose ファイルが用意されています。

```bash
docker compose -f neo4j/docker-compose.yml up -d
```

これにより、以下の設定で Neo4j 5 Community Edition が起動します。

- ブラウザ UI: http://localhost:7474
- Bolt / HTTP エンドポイント: `7687` / `7474`
- デフォルトの認証情報: `neo4j` / `letmein123`

### 3. 環境変数を設定する

サンプルの env ファイルをコピーし、値を入力します。

```bash
cp .env.exm .env
```

```env
GOOGLE_API_KEY=your_api_key_here
```

サポートされている変数の一覧は下記の[設定](#設定)を参照してください。

### 4. サービスを起動する

```bash
cargo run
```

デフォルトでは、サーバーは `0.0.0.0:3200` でリッスンします。以下のように表示されます。

```
listening on 0.0.0.0:3200
OpenAPI spec available at http://0.0.0.0:3200/openapi.json
```

### 5. API を試す

対話的な Swagger UI は以下で利用できます。

```
http://localhost:3200/docs
```

生の OpenAPI 仕様は以下で提供されます。

```
http://localhost:3200/openapi.json
```

サンプルリクエストは [`test.http`](./test.http) にも用意されており、REST クライアント拡張機能を備えたエディタ（VS Code の REST Client や JetBrains の HTTP client など）から直接実行できます。

## 設定

このサービスは（`dotenvy` により `.env` から読み込まれる）環境変数から設定を読み取ります。

| 変数 | デフォルト | 説明 |
|---|---|---|
| `GOOGLE_API_KEY` | — | BAML 抽出クライアントで使用される API キー |
| `ONTOLOGY_PATH` | `ontology.jsonld` | 起動時に読み込み・検証する JSON-LD オントロジーファイルのパス |
| `NEO4J_URI` | `http://localhost:7474` | Cypher の実行に使用する Neo4j の HTTP エンドポイント |
| `NEO4J_USER` | `neo4j` | Neo4j のユーザー名 |
| `NEO4J_PASSWORD` | `letmein123` | Neo4j のパスワード |
| `LISTEN_ADDR` | `0.0.0.0:3200` | Axum サーバーがバインドするアドレス／ポート |

オントロジーは起動時に検証されます。ファイルが存在しない、または不正な場合、サービスはエラーで終了します。

## API エンドポイント

| メソッド | パス | 説明 |
|---|---|---|
| `GET` | `/health` | ヘルスチェック。`{"status": "ok"}` を返す |
| `POST` | `/extract` | オントロジーを用いて `text` からエンティティ・関係を抽出し、Cypher を生成して Neo4j に書き込む |
| `POST` | `/cypher` | 1 つ以上の生の Cypher `statements` を Neo4j に対して実行する |
| `GET` | `/graph` | 現在グラフに保存されているすべてのノードを取得する |
| `GET` | `/docs` | Swagger UI |
| `GET` | `/openapi.json` | 生の OpenAPI 仕様 |

### 例: テキストからエンティティを抽出する

```bash
curl -X POST http://localhost:3200/extract \
  -H "Content-Type: application/json" \
  -d '{"text": "Jane Doe works at Acme Corp and has skills in Python and Rust."}'
```

レスポンス:

```json
{
  "extracted": { "...": "オントロジーに一致するエンティティと関係" },
  "cypher": "MERGE (p:Person {name: 'Jane Doe'}) ..."
}
```

### 例: グラフを照会する

```bash
curl http://localhost:3200/graph
```

### 例: 生の Cypher を実行する

```bash
curl -X POST http://localhost:3200/cypher \
  -H "Content-Type: application/json" \
  -d '{"statements": ["MATCH (n) RETURN n LIMIT 10"]}'
```

## オントロジーについて

オントロジーは、クラスとそのプロパティ／関係を記述する JSON-LD ドキュメントとして定義されます。同梱の [`ontology.jsonld`](./ontology.jsonld) は、シンプルな採用／組織ドメインをモデル化したものです。

- **Company（会社）** — `name` を持つ
- **Person（人物）** — `name` を持ち、Company に `worksFor`（勤務）し、Skill を 1 つ以上 `hasSkill`（保有）する
- **Skill（スキル）** — `name` を持つ

独自のオントロジーに差し替える（あるいは `ONTOLOGY_PATH` で別のファイルを指定する）ことで、製品とサプライヤー、医療エンティティ、法的条項など、テキストから異なる種類のグラフを抽出できます。

## テスト

```bash
cargo test
```

## 技術スタック

- [Axum](https://github.com/tokio-rs/axum) — HTTP サーバーフレームワーク
- [BAML](https://www.boundaryml.com/) — 構造化 LLM 抽出
- [Neo4j](https://neo4j.com/) — グラフデータベース
- [utoipa](https://github.com/juhaku/utoipa) + [Swagger UI](https://github.com/juhaku/utoipa/tree/master/utoipa-swagger-ui) — OpenAPI ドキュメント
- [Tokio](https://tokio.rs/) — 非同期ランタイム
- [serde](https://serde.rs/) — シリアライゼーション

## ライセンス

MIT License の下でライセンスされています。詳細は [LICENSE](./LICENSE) を参照してください。
