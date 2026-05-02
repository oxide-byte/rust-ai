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
        [_, command, question] if command == "ask" => {
            ask_question(question).await?;
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo run -p rag_sample -- load <path-to-pdf>");
    println!("  cargo run -p rag_sample -- ask \"<question>\"");
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
    send_sql(&client, &statements).await?;

    for (page_index, page_text) in pages.iter().enumerate() {
        let trimmed = page_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let embedding = get_embedding(&client, trimmed).await?;

        let record = json!({
            "source": pdf_name,
            "page": page_index + 1,
            "text": trimmed,
            "embedding": embedding,
        });

        let mut page_statements = vec![format!("USE NS {}; USE DB {};", NAMESPACE, DATABASE)];
        page_statements.push(format!("CREATE chunk CONTENT {};", record));
        send_sql(&client, &page_statements).await?;
        println!("Loaded page {}...", page_index + 1);
    }

    println!("Loaded all page chunks into SurrealDB.");
    println!("Ask questions with: cargo run -p rag_sample -- ask \"your question\"");
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
        println!("No relevant PDF chunks found for the question: \"{}\"", question);
        println!("Make sure the PDF has been loaded into SurrealDB first.");
        return Ok(());
    }

    let mut context = String::new();
    println!("Top matches for: \"{}\"", question);
    for (rank, row) in rows.into_iter().take(3).enumerate() {
        let source = row.get("source").and_then(Value::as_str).unwrap_or("unknown");
        let page = row.get("page").and_then(Value::as_u64).unwrap_or(0);
        let text = row.get("text").and_then(Value::as_str).unwrap_or("");
        
        context.push_str(&format!("\n--- Source: {}, Page: {} ---\n{}\n", source, page, text));

        let preview: String = text.lines().take(4).collect::<Vec<_>>().join(" ").chars().take(400).collect();
        println!("\nResult {} (source={}, page={}):", rank + 1, source, page);
        println!("{}", preview);
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
    let array = response
        .as_array()
        .ok_or("Unexpected SurrealDB response format")?;

    let last = array
        .last()
        .ok_or("SurrealDB returned an empty result array")?;

    let result = last
        .get("result")
        .or_else(|| last.get("results"))
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