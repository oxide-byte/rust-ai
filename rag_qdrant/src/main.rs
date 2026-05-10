use std::{env, error::Error, path::Path};

use pdf_extract::extract_text_by_pages;
use reqwest::Client;
use serde_json::{json, Value};

const QDRANT_URL: &str = "http://127.0.0.1:6333";
const COLLECTION_NAME: &str = "pdf_chunks";

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
    println!("  cargo run -p rag_qdrant -- load <path-to-pdf>");
    println!("  cargo run -p rag_qdrant -- ask \"<question>\"");
}

async fn ensure_collection(client: &Client) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/collections/{}", QDRANT_URL, COLLECTION_NAME);
    let response = client.get(&url).send().await?;

    if response.status() == 404 {
        println!("Creating collection '{}'...", COLLECTION_NAME);
        let create_response = client
            .put(&url)
            .json(&json!({
                "vectors": {
                    "size": 4096,
                    "distance": "Cosine"
                }
            }))
            .send()
            .await?;

        if !create_response.status().is_success() {
            return Err(format!(
                "Failed to create collection: {}",
                create_response.text().await?
            )
            .into());
        }
    }
    Ok(())
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
    ensure_collection(&client).await?;

    let mut points = Vec::new();

    for (page_index, page_text) in pages.iter().enumerate() {
        let trimmed = page_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let embedding = get_embedding(&client, trimmed).await?;

        points.push(json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "vector": embedding,
            "payload": {
                "source": pdf_name,
                "page": page_index + 1,
                "text": trimmed,
            }
        }));
        
        if points.len() >= 10 {
             upload_points(&client, &points).await?;
             points.clear();
        }
        println!("Processed page {}...", page_index + 1);
    }

    if !points.is_empty() {
        upload_points(&client, &points).await?;
    }

    println!("Loaded all page chunks into Qdrant.");
    println!("Ask questions with: cargo run -p rag_qdrant -- ask \"your question\"");
    Ok(())
}

async fn upload_points(client: &Client, points: &[Value]) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/collections/{}/points?wait=true", QDRANT_URL, COLLECTION_NAME);
    let response = client
        .put(&url)
        .json(&json!({ "points": points }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to upload points: {}", response.text().await?).into());
    }
    Ok(())
}

async fn ask_question(question: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    
    println!("Generating embedding for the question...");
    let q_embedding = get_embedding(&client, question).await?;

    println!("Searching for relevant chunks in Qdrant...");
    let url = format!("{}/collections/{}/points/search", QDRANT_URL, COLLECTION_NAME);
    let response = client
        .post(&url)
        .json(&json!({
            "vector": q_embedding,
            "limit": 3,
            "with_payload": true
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Qdrant search failed: {}", response.text().await?).into());
    }

    let search_result: Value = response.json().await?;
    let hits = search_result["result"]
        .as_array()
        .ok_or("Unexpected Qdrant search response format")?;

    if hits.is_empty() {
        println!("No relevant PDF chunks found for the question: \"{}\"", question);
        return Ok(());
    }

    let mut context = String::new();
    println!("Top matches for: \"{}\"", question);
    for (rank, hit) in hits.iter().enumerate() {
        let payload = &hit["payload"];
        let source = payload.get("source").and_then(Value::as_str).unwrap_or("unknown");
        let page = payload.get("page").and_then(Value::as_u64).unwrap_or(0);
        let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
        
        context.push_str(&format!("\n--- Source: {}, Page: {} ---\n{}\n", source, page, text));

        let preview: String = text.lines().take(4).collect::<Vec<_>>().join(" ").chars().take(400).collect();
        println!("\nResult {} (source={}, page={}, score={}):", rank + 1, source, page, hit["score"]);
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