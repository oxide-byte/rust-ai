use std::io::{self, Write};

#[derive(serde::Deserialize)]
struct OllamaResponse {
    response: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Enter 5 keywords for the joke:");

    let mut keywords = Vec::new();
    for i in 1..=5 {
        print!("Keyword {}: ", i);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        keywords.push(input.trim().to_string());
    }

    let prompt = format!("Create a funny joke using these keywords: {}", keywords.join(", "));

    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({
            "model": "llama3.1",  // Assuming llama3.1 is running; user can change if needed
            "system": "You are a hilarious standup comedian. Deliver jokes in an exaggerated, witty, and crowd-pleasing style, like a professional comedian on stage.",
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.7
            }
        }))
        .send()
        .await?;

    if response.status().is_success() {
        let ollama_resp: OllamaResponse = response.json().await?;
        println!("\nGenerated Joke:\n{}", ollama_resp.response);
    } else {
        println!("Error: Failed to get response from Ollama. Make sure Ollama is running and the model 'llama2' is available.");
    }

    Ok(())
}