# Ollama Sample: Introduction to Local LLM Integration

## Welcome!

This introduction guide walks you through **ollama_sample**, a beginner-friendly Rust application that demonstrates how to integrate a local Ollama LLM into your Rust projects. By the end of this guide, you'll understand how to communicate with local AI models and build interactive applications.

## What You'll Build

A command-line Rust application that:
- Prompts you to enter 5 keywords
- Sends those keywords to a local Ollama AI model
- Generates a creative joke based on those keywords
- Displays the result in your terminal

This is a great starting point for learning:
- Async Rust programming with `tokio`
- HTTP API calls using `reqwest`
- JSON serialization with `serde`
- Prompt engineering for AI models

## System Requirements

Before you begin, ensure you have:

1. **Rust toolchain** - Install from [rustup.rs](https://rustup.rs)
2. **Ollama** - Download from [ollama.ai](https://ollama.ai)
3. **A language model** - Download one with: `ollama pull llama3.1` (or `llama2`)

## Quick Start

### Step 1: Start Ollama
Open a terminal and keep it running:
```bash
ollama serve
```

### Step 2: Run the Application
In a new terminal, navigate to your workspace and run:
```bash
cargo run -p ollama_sample
```

### Step 3: Enter Keywords
When prompted, type 5 keywords (one per line), for example:
```
Keyword 1: coffee
Keyword 2: robots
Keyword 3: pizza
Keyword 4: astronauts
Keyword 5: socks
```

### Step 4: See the Magic
The application will generate and display a funny joke using your keywords!

## How It Works Under the Hood

1. **User Input Collection**: The app reads 5 keywords from your terminal
2. **Prompt Construction**: Keywords are formatted into a natural language instruction
3. **API Request**: An async HTTP POST request is sent to Ollama's `/api/generate` endpoint
4. **Model Processing**: Your local LLM processes the prompt
5. **Response Handling**: The generated joke is displayed in your terminal

## Project Structure

```
ollama_sample/
├── Cargo.toml          # Package manifest with dependencies
└── src/
    └── main.rs         # Main application (async Rust code)
```

## Key Technologies

| Technology | Purpose |
|-----------|---------|
| `reqwest` | Making HTTP requests to Ollama |
| `tokio` | Async runtime for concurrent operations |
| `serde` & `serde_json` | JSON serialization and deserialization |

## Tips for Success

- **Model Selection**: `llama3.1` provides better joke quality than `llama2`; experiment with different models
- **Ollama Server**: Keep the `ollama serve` command running in a separate terminal
- **Customization**: Edit the system prompt in `main.rs` to change the AI's personality (e.g., "Act as a standup comedian")
- **Error Messages**: If you see connection errors, verify Ollama is running on `http://localhost:11434`

## What's Next?

Once comfortable with this example, try:
- Adding more sophisticated prompts
- Experimenting with different models
- Building a web interface using the same Ollama API
- Creating multi-turn conversations