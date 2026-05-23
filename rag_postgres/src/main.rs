use std::{env, error::Error, path::Path};

use pdf_extract::extract_text_by_pages;
use pgvector::Vector;
use reqwest::Client;
use serde_json::{json, Value};
use tokio_postgres::NoTls;

const DATABASE_URL: &str = "host=127.0.0.1 port=5432 user=postgres password=postgres dbname=rag";

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
    println!("  cargo run -p rag_postgres -- load <path-to-pdf>");
    println!("  cargo run -p rag_postgres -- ask \"<question>\"");
}

async fn ensure_schema() -> Result<tokio_postgres::Client, Box<dyn Error>> {
    let (db_client, connection) = tokio_postgres::connect(DATABASE_URL, NoTls).await?;

    tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("Postgres connection error: {}", err);
        }
    });

    db_client.execute("CREATE EXTENSION IF NOT EXISTS vector", &[]).await?;
    db_client
        .execute(
            "
            CREATE TABLE IF NOT EXISTS pdf_chunks (
                id UUID PRIMARY KEY,
                source TEXT NOT NULL,
                page INTEGER NOT NULL,
                text TEXT NOT NULL,
                embedding vector(4096) NOT NULL
            )
            ",
            &[],
        )
        .await?;

    Ok(db_client)
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
        .unwrap_or("unknown.pdf")
        .to_string();

    let ollama_client = Client::new();
    let db_client = ensure_schema().await?;

    for (page_index, page_text) in pages.iter().enumerate() {
        let trimmed = page_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let embedding = get_embedding(&ollama_client, trimmed).await?;
        let vec = embedding
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect::<Vec<f32>>()
            })
            .unwrap_or_default();

        db_client
            .execute(
                "
                INSERT INTO pdf_chunks (id, source, page, text, embedding)
                VALUES ($1, $2, $3, $4, $5)
                ",
                &[
                    &uuid::Uuid::new_v4(),
                    &pdf_name,
                    &((page_index + 1) as i32),
                    &trimmed,
                    &Vector::from(vec),
                ],
            )
            .await?;

        println!("Processed page {}...", page_index + 1);
    }

    println!("Loaded all page chunks into PostgreSQL.");
    println!("Ask questions with: cargo run -p rag_postgres -- ask \"your question\"");
    Ok(())
}

async fn ask_question(question: &str) -> Result<(), Box<dyn Error>> {
    let ollama_client = Client::new();
    let db_client = ensure_schema().await?;

    println!("Generating embedding for the question...");
    let q_embedding = get_embedding(&ollama_client, question).await?;
    let vec = q_embedding
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect::<Vec<f32>>()
        })
        .unwrap_or_default();

    println!("Searching for relevant chunks in PostgreSQL...");
    let rows = db_client
        .query(
            "
            SELECT source, page, text, 1 - (embedding <=> $1) AS score
            FROM pdf_chunks
            ORDER BY embedding <=> $1
            LIMIT 3
            ",
            &[&Vector::from(vec)],
        )
        .await?;

    if rows.is_empty() {
        println!("No relevant PDF chunks found for the question: \"{}\"", question);
        return Ok(());
    }

    let mut context = String::new();
    println!("Top matches for: \"{}\"", question);
    for (rank, row) in rows.iter().enumerate() {
        let source: String = row.get("source");
        let page: i32 = row.get("page");
        let text: String = row.get("text");
        let score: f64 = row.get("score");

        context.push_str(&format!("\n--- Source: {}, Page: {} ---\n{}\n", source, page, text));

        let preview: String = text
            .lines()
            .take(4)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(400)
            .collect();
        println!("\nResult {} (source={}, page={}, score={}):", rank + 1, source, page, score);
        println!("{}", preview);
    }

    println!("\nGenerating answer with Ollama...");
    let answer = generate_answer(&ollama_client, question, &context).await?;
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