use std::{env, error::Error, path::Path};

use pdf_extract::extract_text_by_pages;
use reqwest::Client;
use serde_json::{json, Value};

const SURREAL_URL: &str = "http://127.0.0.1:8000/sql";
const NAMESPACE: &str = "rag";
const DATABASE: &str = "extended"; // Use a different DB to avoid conflicts
const CHUNK_SIZE: usize = 1000;    // Target characters per chunk
const CHUNK_OVERLAP: usize = 200; // Overlap characters

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    match args.as_slice() {
        [_, command, path] if command == "load" => {
            load_pdf(path, "fixed").await?;
        }
        [_, command, path, strategy] if command == "load" => {
            load_pdf(path, strategy).await?;
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
    println!("  cargo run -p rag_surrealdb_extends -- load <path-to-pdf> [strategy]");
    println!("  cargo run -p rag_surrealdb_extends -- ask \"<question>\"");
    println!("\nStrategies:");
    println!("  fixed (default): Fixed-size chunking with overlap");
    println!("  paragraph: Split by paragraphs");
}

/// Simple fixed-size chunking with overlap
fn chunk_text_fixed(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    
    if chars.is_empty() {
        return chunks;
    }

    let mut start = 0;
    while start < chars.len() {
        let end = (start + size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);
        
        if end == chars.len() {
            break;
        }
        
        start += size - overlap;
    }
    
    chunks
}

/// Split text by double newlines (paragraphs)
fn chunk_text_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

async fn load_pdf(pdf_path: &str, strategy: &str) -> Result<(), Box<dyn Error>> {
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

    // Combine all pages into one large text for global chunking
    let full_text = pages.join("\n\n");
    
    let chunks = match strategy {
        "paragraph" => {
            println!("Using Paragraph splitting strategy...");
            chunk_text_paragraphs(&full_text)
        }
        _ => {
            println!("Using Fixed-size chunking strategy (size={}, overlap={})...", CHUNK_SIZE, CHUNK_OVERLAP);
            chunk_text_fixed(&full_text, CHUNK_SIZE, CHUNK_OVERLAP)
        }
    };
    
    println!("Split PDF into {} chunks", chunks.len());

    let client = Client::new();
    let mut setup_statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
    setup_statements.push("DEFINE INDEX IF NOT EXISTS chunk_embedding ON chunk FIELDS embedding HNSW DIMENSION 4096 DISTANCE COSINE;".to_string());
    send_sql(&client, &setup_statements).await?;

    for (i, chunk) in chunks.iter().enumerate() {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }

        let embedding = get_embedding(&client, trimmed).await?;

        let record = json!({
            "source": pdf_name,
            "chunk_index": i,
            "strategy": strategy,
            "text": trimmed,
            "embedding": embedding,
        });

        let mut load_statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
        load_statements.push(format!("CREATE chunk CONTENT {};", record));
        send_sql(&client, &load_statements).await?;
        
        if (i + 1) % 5 == 0 || i + 1 == chunks.len() {
            println!("Loaded chunk {}/{}...", i + 1, chunks.len());
        }
    }

    println!("Loaded all chunks into SurrealDB (Extended).");
    Ok(())
}

async fn ask_question(question: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    
    println!("Generating embedding for the question...");
    let q_embedding = get_embedding(&client, question).await?;

    let statements = vec![
        format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE),
        format!(
            "SELECT *, vector::distance::knn() AS distance FROM chunk WHERE embedding <|3,40|> {} ORDER BY distance ASC;",
            serde_json::to_string(&q_embedding)?
        ),
    ];

    println!("Searching for relevant chunks...");
    let response = send_sql(&client, &statements).await?;
    let rows = extract_rows(&response)?;

    if rows.is_empty() {
        println!("No relevant chunks found for: \"{}\"", question);
        return Ok(());
    }

    let mut context = String::new();
    println!("Top matches for: \"{}\"", question);
    for (rank, row) in rows.into_iter().take(3).enumerate() {
        let source = row.get("source").and_then(Value::as_str).unwrap_or("unknown");
        let chunk_index = row.get("chunk_index").and_then(Value::as_u64).unwrap_or(0);
        let text = row.get("text").and_then(Value::as_str).unwrap_or("");
        
        context.push_str(&format!("\n--- Source: {}, Chunk: {} ---\n{}\n", source, chunk_index, text));

        let preview: String = text.chars().take(300).collect();
        println!("\nResult {} (source={}, chunk={}):", rank + 1, source, chunk_index);
        println!("{}...", preview.replace('\n', " "));
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

fn extract_rows(response: &Value) -> Result<Vec<Value>, Box<dyn Error>> {
    let array = response.as_array().ok_or("Unexpected SurrealDB response format")?;
    let last = array.last().ok_or("SurrealDB returned an empty result array")?;
    let result = last.get("result").or_else(|| last.get("results")).ok_or("No results found")?.as_array().ok_or("Result is not an array")?;
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
        return Err(format!("SurrealDB query failed: {}", response.text().await?).into());
    }

    let json = response.json::<Value>().await?;
    Ok(json)
}