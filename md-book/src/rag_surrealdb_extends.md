# RAG Sample: SurrealDB with Advanced Chunking

## Overview

This module, `rag_surrealdb_extends`, builds upon the basic SurrealDB RAG sample by implementing **Advanced Chunking Strategies**. 

While the basic version splits documents by page, this version supports:
1. **Fixed-Size Chunking with Overlap**
2. **Paragraph Splitting**

## Key Features

- **Global Text Extraction**: Combines all pages of a PDF into a single text stream.
- **Fixed-Size Chunks**: Uses a window of 1000 characters with a 200-character overlap.
- **Paragraph Chunks**: Splits text by double newlines (`\n\n`), preserving natural structural boundaries.
- **Improved Context**: Different strategies allow for balancing between precise retrieval and context preservation.

## How It Works

### The Chunking Algorithms

#### 1. Fixed-Size Sliding Window
The application implements a sliding window approach:

```rust
fn chunk_text_fixed(text: &str, size: usize, overlap: usize) -> Vec<String> {
    // ... logic to create chunks of 'size' with 'overlap' ...
}
```

#### 2. Paragraph Split
Splits the text into segments based on double newlines:

```rust
fn chunk_text_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        // ... trim and filter ...
}
```

### Comparison: Page-level vs. Advanced Strategies

| Feature | Page-Level (Basic) | Fixed-Size (Extended) | Paragraph (Extended) |
|---------|-------------------|-----------------------|----------------------|
| Granularity | Coarse (Entire Page) | Fine (1000 chars) | Variable (Paragraph) |
| Context Preservation | Poor (breaks at page end) | Good (Overlap) | Excellent (Natural) |
| DB Storage | One record per page | Multiple records | Multiple records |
| Best For | Simple docs | Dense, flat text | Structured narratives |

## Setup

### 1. Start SurrealDB
Ensure SurrealDB is running:
```bash
docker compose up -d surrealdb
```

### 2. Load a PDF (Fixed-Size Strategy)
```bash
cargo run -p rag_surrealdb_extends -- load data/the-tale-of-peter-rabbit.pdf fixed
```

### 3. Load a PDF (Paragraph Strategy)
```bash
cargo run -p rag_surrealdb_extends -- load data/the-tale-of-peter-rabbit.pdf paragraph
```
*Note: This will use the `extended` database in the `rag` namespace.*

### 4. Ask a Question
```bash
cargo run -p rag_surrealdb_extends -- ask "Who is Peter?"
```

## Execution

### Fixed

```text
cargo run -p rag_surrealdb_extends -- load data/the-tale-of-peter-rabbit.pdf fixed
   Compiling rag_surrealdb_extends v0.1.0 (/Users/qdart/projects/rust-ai/rag_surrealdb_extends)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.25s
     Running `target/debug/rag_surrealdb_extends load data/the-tale-of-peter-rabbit.pdf fixed`
Extracting text from data/the-tale-of-peter-rabbit.pdf...
Using Fixed-size chunking strategy (size=1000, overlap=200)...
Split PDF into 7 chunks
Loaded chunk 5/7...
Loaded chunk 7/7...
Loaded all chunks into SurrealDB (Extended).

cargo run -p rag_surrealdb_extends -- ask "Who is Peter?"
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s
     Running `target/debug/rag_surrealdb_extends ask 'Who is Peter?'`
Generating embedding for the question...
Searching for relevant chunks...
Top matches for: "Who is Peter?"

Result 1 (source=the-tale-of-peter-rabbit.pdf, chunk=6):
e wondered what he had done with his clothes. It  was the second little jacket and pair of shoes that Peter had lost in a fortnight!  I am sorry to say that Peter was not very well during the evening.  His mother put him to bed, and made some camomile tea; and she gave a dose of it to Peter!  “One t...

Result 2 (source=the-tale-of-peter-rabbit.pdf, chunk=5):
a wheelbarrow, and peeped over. The first  thing he saw was Mr. McGregor hoeing onions. His back was turned towards Peter, and beyond  him was the gate!    Peter got down very quietly off the wheelbarrow, and started running as fast as he could go,  along a straight walk behind some black-currant bu...

Result 3 (source=the-tale-of-peter-rabbit.pdf, chunk=0):
The Tale of Peter Rabbit Beatrix Potter  Once upon a time there were four little Rabbits, and their names were— Flopsy, Mopsy, Cotton- tail, and Peter.  They lived with their Mother in a sand-bank, underneath the root of a very big fir tree.    “Now, my dears,” said old Mrs. Rabbit one morning, “you...

Generating answer with Ollama...

Answer:
Peter is one of the four little rabbits who live with their mother in a sand-bank under the root of a big fir tree. He is described as "very naughty" and has a tendency to get into trouble by disobeying his mother's instructions not to go into Mr. McGregor's garden.
```

### Paragraph

```text
cargo run -p rag_surrealdb_extends -- load data/the-tale-of-peter-rabbit.pdf paragraph
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running `target/debug/rag_surrealdb_extends load data/the-tale-of-peter-rabbit.pdf paragraph`
Extracting text from data/the-tale-of-peter-rabbit.pdf...
Using Paragraph splitting strategy...
Split PDF into 38 chunks
Loaded chunk 5/38...
Loaded chunk 10/38...
Loaded chunk 15/38...
Loaded chunk 20/38...
Loaded chunk 25/38...
Loaded chunk 30/38...
Loaded chunk 35/38...
Loaded chunk 38/38...
Loaded all chunks into SurrealDB (Extended).

cargo run -p rag_surrealdb_extends -- ask "Who is Peter?"
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running `target/debug/rag_surrealdb_extends ask 'Who is Peter?'`
Generating embedding for the question...
Searching for relevant chunks...
Top matches for: "Who is Peter?"

Result 1 (source=the-tale-of-peter-rabbit.pdf, chunk=34):
“One table-spoonful to be taken at bed-time.”...

Result 2 (source=the-tale-of-peter-rabbit.pdf, chunk=4):
“Now run along, and don’t get into mischief. I am going out.”...

Result 3 (source=the-tale-of-peter-rabbit.pdf, chunk=9):
And then, feeling rather sick, he went to look for some parsley....

Generating answer with Ollama...

Answer:
The question doesn't ask about Peter's characteristics or actions, but rather who Peter is. Based on the context, it appears that Peter is the subject of "the-tale-of-peter-rabbit.pdf", which suggests that Peter is likely the main character in The Tale of Peter Rabbit.
```

## Next Steps

- **Recursive Character Splitting**: Improve chunking by splitting at natural boundaries like paragraphs and sentences first.
- **Metadata Enhancement**: Store the original page number for each chunk to allow referencing back to the source.