# RAG Sample: SurrealDB with Relational & Hybrid Search

## Overview

This module, `rag_surrealdb_with_db`, takes our SurrealDB integration to the next level by utilizing the database's **Relational (Graph)** and **Full-Text Search (FTS)** capabilities.

Instead of just storing chunks as flat records, we now:
1. Track **Documents** as first-class entities.
2. Create **Relationships** (`RELATE`) between documents and their chunks.
3. Use **Hybrid Search** (Vector + Full-Text) to improve retrieval accuracy.

## Key Features

- **Relational Data Model**: Uses `SCHEMAFULL` tables and graph relations to link chunks to their parent document.
- **Full-Text Search Indexing**: Implements `FULLTEXT ANALYZER` for keyword-based retrieval.
- **Hybrid Retrieval**: Combines results from both K-Nearest Neighbor (KNN) vector search and Full-Text Search.
- **Context Labeling**: The AI model is informed whether a piece of context was found via vector similarity or keyword match.

## How It Works

### 1. Relational Schema & Indexing
We define a formal schema for documents and indexes to support relational data and hybrid search:

```sql
-- Schema for documents
DEFINE TABLE document SCHEMAFULL;
DEFINE FIELD name ON document TYPE string;

-- Define an analyzer for Full-Text Search
DEFINE ANALYZER ascii TOKENIZERS blank FILTERS ascii, lowercase;

-- Full-Text index
DEFINE INDEX chunk_text ON chunk FIELDS text FULLTEXT ANALYZER ascii BM25 HIGHLIGHTS;

-- Vector index
DEFINE INDEX chunk_embedding ON chunk FIELDS embedding HNSW DIMENSION 4096 DISTANCE COSINE;
```

### 2. Hybrid Search Query
The `ask` command now executes two different types of searches in a single request:

```sql
-- Full-Text Search
SELECT *, search::score(1) AS score FROM chunk WHERE text @1@ 'question' ORDER BY score DESC;

-- Vector Search
SELECT *, vector::distance::knn() AS distance FROM chunk WHERE embedding <|3,40|> [vector] ORDER BY distance ASC;
```

## Setup

### 1. Start SurrealDB
Ensure SurrealDB is running:
```bash
docker compose up -d surrealdb
```

### 2. Initialize Database with Sample Data
You can load a sample description of Peter to test the search immediately:
```bash
cargo run -p rag_surrealdb_with_db -- init_db
```

### 3. Load a PDF
The `load` command now creates a document record and relates all chunks to it:
```bash
cargo run -p rag_surrealdb_with_db -- load data/the-tale-of-peter-rabbit.pdf
```

### 4. Ask a Question
The `ask` command will show you matches from both search methods:
```bash
cargo run -p rag_surrealdb_with_db -- ask "Who is Peter?"
```

## Execution

```text
cargo run -p rag_surrealdb_with_db -- init_db
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
     Running `target/debug/rag_surrealdb_with_db init_db`
Generating embedding for sample text...
Loaded sample data for Peter the rabbit into SurrealDB.
qdart@MacBookPro rust-ai % cargo run -p rag_surrealdb_with_db -- load data/the-tale-of-peter-rabbit.pdf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running `target/debug/rag_surrealdb_with_db load data/the-tale-of-peter-rabbit.pdf`
Extracting text from data/the-tale-of-peter-rabbit.pdf...
Loaded page 1...
Loaded page 2...
Loaded page 3...
Loaded page 4...
Loaded page 5...
Loaded page 6...
Loaded page 7...
Loaded page 8...
Loaded page 9...
Loaded page 10...
Loaded page 11...
Loaded page 12...
Loaded page 13...
Loaded page 14...
Loaded all page chunks into SurrealDB.
Ask questions with: cargo run -p rag_surrealdb_with_db -- ask "your question"

cargo run -p rag_surrealdb_with_db -- ask "Who is Peter?"

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running `target/debug/rag_surrealdb_with_db ask 'Who is Peter?'`
Searching for matches using full-text search...
Generating embedding for the question...

--- Full-Text Search Matches ---

--- Vector Search Matches ---
Vector Match 1: Peter is a small, adventurous rabbit who wears a blue jacket and lives in a sand-bank under a fir tr (source: manual_entry)
Vector Match 2: And then, feeling rather sick, he went to look for some parsley.

But round the end of a cucumber fr (source: the-tale-of-peter-rabbit.pdf)
Vector Match 3: Mr. McGregor was quite sure that Peter was somewhere in the toolshed, perhaps hidden 
underneath a f (source: the-tale-of-peter-rabbit.pdf)

Generating answer with Ollama...

Answer:
Peter is a rabbit.
cargo run -p rag_surrealdb_with_db -- ask "Describe Peter?"

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
     Running `target/debug/rag_surrealdb_with_db ask 'Describe Peter?'`
Searching for matches using full-text search...
Generating embedding for the question...

--- Full-Text Search Matches ---

--- Vector Search Matches ---
Vector Match 1: Peter is a small, adventurous rabbit who wears a blue jacket and lives in a sand-bank under a fir tr (source: manual_entry)
Vector Match 2: And then, feeling rather sick, he went to look for some parsley.

But round the end of a cucumber fr (source: the-tale-of-peter-rabbit.pdf)
Vector Match 3: Mr. McGregor was quite sure that Peter was somewhere in the toolshed, perhaps hidden 
underneath a f (source: the-tale-of-peter-rabbit.pdf)

Generating answer with Ollama...

Answer:
According to the context, Peter is described as a "small, adventurous rabbit" who wears a blue jacket and lives with his family in a sand-bank under a fir tree.
```

## Why Hybrid Search?

Vector search is great at finding "meaning" but sometimes misses specific keywords (like "fortnight" or "jacket" if they aren't weighted heavily in the embedding). Full-text search excels at exact matches. By combining them, we get the best of both worlds.