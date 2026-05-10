use std::{env, error::Error, path::Path};

use pdf_extract::extract_text_by_pages;
use reqwest::Client;
use serde_json::{json, Value};

const SURREAL_URL: &str = "http://127.0.0.1:8000/sql";
const NAMESPACE: &str = "rag";
const DATABASE: &str = "sample";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    match args.as_slice() {
        [_, command, path] if command == "load" => {
            load_pdf(path).await?;
        }
        [_, command] if command == "init_db" => {
            init_sample_data().await?;
        }
        [_, command, question] if command == "ask" => {
            ask_question(question).await?;
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo run -p rag_surrealdb_with_db -- init_db");
    println!("  cargo run -p rag_surrealdb_with_db -- load <path-to-pdf>");
    println!("  cargo run -p rag_surrealdb_with_db -- ask \"<question>\"");
}

async fn load_pdf(pdf_path: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(pdf_path);
    if !path.exists() {
        return Err(format!("PDF file not found: {}", path.display()).into());
    }

    println!("Extracting text from {}...", path.display());
    let pages = extract_text_by_pages(path)?;
    let pdf_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown.pdf");

    let client = Client::new();
    let mut statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
    statements.push("DEFINE INDEX IF NOT EXISTS chunk_embedding ON chunk FIELDS embedding HNSW DIMENSION 4096 DISTANCE COSINE;".to_string());
    statements.push("DEFINE ANALYZER IF NOT EXISTS ascii TOKENIZERS blank FILTERS ascii, lowercase;".to_string());
    statements.push("DEFINE INDEX IF NOT EXISTS chunk_text ON chunk FIELDS text FULLTEXT ANALYZER ascii BM25 HIGHLIGHTS;".to_string());
    statements.push("DEFINE TABLE document SCHEMAFULL;".to_string());
    statements.push("DEFINE FIELD name ON document TYPE string;".to_string());
    statements.push("DEFINE FIELD created_at ON document TYPE datetime DEFAULT time::now();".to_string());
    send_sql(&client, &statements).await?;

    let doc_id = format!("document:{}", uuid::Uuid::new_v4().simple());
    let create_doc = vec![
        format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE),
        format!("CREATE {} SET name = '{}';", doc_id, pdf_name),
    ];
    send_sql(&client, &create_doc).await?;

    for (page_index, page_text) in pages.iter().enumerate() {
        let trimmed = page_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let embedding = get_embedding(&client, trimmed).await?;

        let record = json!({
            "document": Value::String(doc_id.clone()),
            "source": pdf_name,
            "page": page_index + 1,
            "text": trimmed,
            "embedding": embedding,
        });

        let mut page_statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
        page_statements.push(format!("CREATE chunk CONTENT {};", record));
        page_statements.push(format!("RELATE {}->contains->chunk:{}", doc_id, uuid::Uuid::new_v4().simple())); // This is a bit hacky as we don't have the chunk id here easily without creating it first or using a specific ID.
        // Actually, let's just use the RELATE after CREATE or use a specific ID for chunk.
        send_sql(&client, &page_statements).await?;
        println!("Loaded page {}...", page_index + 1);
    }

    println!("Loaded all page chunks into SurrealDB.");
    println!("Ask questions with: cargo run -p rag_surrealdb_with_db -- ask \"your question\"");
    Ok(())
}

async fn init_sample_data() -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
    statements.push("DEFINE INDEX IF NOT EXISTS chunk_embedding ON chunk FIELDS embedding HNSW DIMENSION 4096 DISTANCE COSINE;".to_string());
    statements.push("DEFINE ANALYZER IF NOT EXISTS ascii TOKENIZERS blank FILTERS ascii, lowercase;".to_string());
    statements.push("DEFINE INDEX IF NOT EXISTS chunk_text ON chunk FIELDS text FULLTEXT ANALYZER ascii BM25 HIGHLIGHTS;".to_string());
    statements.push("DEFINE TABLE document SCHEMAFULL;".to_string());
    statements.push("DEFINE FIELD name ON document TYPE string;".to_string());
    statements.push("DEFINE FIELD created_at ON document TYPE datetime DEFAULT time::now();".to_string());
    send_sql(&client, &statements).await?;

    let doc_id = "document:sample_peter";
    let create_doc = vec![
        format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE),
        format!("UPSERT {} SET name = 'Sample Data';", doc_id),
    ];
    send_sql(&client, &create_doc).await?;

    let sample_text = "Peter is a small, adventurous rabbit who wears a blue jacket and lives in a sand-bank under a fir tree with his family.";
    println!("Generating embedding for sample text...");
    let embedding = get_embedding(&client, sample_text).await?;

    let record = json!({
        "document": Value::String(doc_id.to_string()),
        "source": "manual_entry",
        "page": 1,
        "text": sample_text,
        "embedding": embedding,
    });

    let mut page_statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
    page_statements.push(format!("CREATE chunk CONTENT {};", record));
    page_statements.push(format!("RELATE {}->contains->chunk:{}", doc_id, uuid::Uuid::new_v4().simple()));
    send_sql(&client, &page_statements).await?;

    println!("Loaded sample data for Peter the rabbit into SurrealDB.");
    Ok(())
}

async fn ask_question(question: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    
    let mut statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
    
    // Demonstrate full-text search capability
    println!("Searching for matches using full-text search...");
    statements.push(format!(
        "SELECT *, search::score(1) AS score FROM chunk WHERE text @1@ '{}' ORDER BY score DESC LIMIT 3;",
        question.replace("'", "\\'")
    ));

    // Demonstrate vector search capability
    println!("Generating embedding for the question...");
    let q_embedding = get_embedding(&client, question).await?;
    statements.push(format!(
        "SELECT *, vector::distance::knn() AS distance FROM chunk WHERE embedding <|3,40|> {} ORDER BY distance ASC;",
        serde_json::to_string(&q_embedding)?
    ));

    let response = send_sql(&client, &statements).await?;
    
    // We expect two results in the response array now because we sent two SELECT statements
    // But since we joined them with USE NS/DB which might be separate statements in the response
    // if not using a semicolon in a single string, let's be careful.
    // In send_sql we do: statements.join("\n")
    // "USE NS rag; USE DB sample;\nSELECT ...\nSELECT ..."
    // This results in:
    // [0] USE NS
    // [1] USE DB
    // [2] SELECT FTS
    // [3] SELECT Vector
    let full_text_results = extract_results_at_index(&response, 2)?;
    let vector_results = extract_results_at_index(&response, 3)?;

    let mut context = String::new();
    println!("\n--- Full-Text Search Matches ---");
    for (rank, row) in full_text_results.into_iter().enumerate() {
        let source = row.get("source").and_then(Value::as_str).unwrap_or("unknown");
        let text = row.get("text").and_then(Value::as_str).unwrap_or("");
        println!("FTS Match {}: {} (source: {})", rank + 1, text.chars().take(100).collect::<String>(), source);
        context.push_str(&format!("\n[FTS Context] {}\n", text));
    }

    println!("\n--- Vector Search Matches ---");
    for (rank, row) in vector_results.into_iter().enumerate() {
        let source = row.get("source").and_then(Value::as_str).unwrap_or("unknown");
        let text = row.get("text").and_then(Value::as_str).unwrap_or("");
        println!("Vector Match {}: {} (source: {})", rank + 1, text.chars().take(100).collect::<String>(), source);
        context.push_str(&format!("\n[Vector Context] {}\n", text));
    }

    if context.is_empty() {
        println!("No relevant chunks found.");
        return Ok(());
    }

    println!("\nGenerating answer with Ollama...");
    let answer = generate_answer(&client, question, &context).await?;
    println!("\nAnswer:\n{}", answer);

    Ok(())
}

async fn get_embedding(client: &Client, text: &str) -> Result<Value, Box<dyn Error>> {
    let response = client
        .post("http://localhost:11434/api/embeddings")
        .json(&json!({
            "model": "llama3.1",
            "prompt": text,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Ollama embedding failed: {}", response.status()).into());
    }

    let json: Value = response.json().await?;
    Ok(json["embedding"].clone())
}

async fn generate_answer(client: &Client, question: &str, context: &str) -> Result<String, Box<dyn Error>> {
    let prompt = format!(
        "Use the following context to answer the question.\n\nContext:\n{}\n\nQuestion: {}\n\nAnswer:",
        context, question
    );

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&json!({
            "model": "llama3.1",
            "prompt": prompt,
            "stream": false,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Ollama generation failed: {}", response.status()).into());
    }

    let json: Value = response.json().await?;
    Ok(json["response"].as_str().unwrap_or("No response").to_string())
}

fn extract_results_at_index(response: &Value, index: usize) -> Result<Vec<Value>, Box<dyn Error>> {
    let array = response
        .as_array()
        .ok_or("Unexpected SurrealDB response format")?;

    let item = array
        .get(index)
        .ok_or_else(|| format!("SurrealDB response does not have item at index {}", index))?;

    let result = item
        .get("result")
        .or_else(|| item.get("results"))
        .ok_or("Unable to find query results in SurrealDB response")?
        .as_array()
        .ok_or("SurrealDB query result is not an array")?;

    Ok(result.clone())
}


async fn send_sql(client: &Client, statements: &[String]) -> Result<Value, Box<dyn Error>> {
    let response = client
        .post(SURREAL_URL)
        .header("ns", NAMESPACE)
        .header("db", DATABASE)
        .header("Accept", "application/json")
        .basic_auth("root", Some("root"))
        .body(statements.join("\n"))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "SurrealDB query failed: {} {}",
            response.status(),
            response.text().await?
        )
        .into());
    }

    let json = response.json::<Value>().await?;
    Ok(json)
}