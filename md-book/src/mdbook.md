# mdBook Instructions

This project uses **mdBook** to generate documentation.

## Prerequisites

To build the documentation, you need to install `mdbook` and the `mdbook-mermaid` preprocessor for diagrams.

### 1. Install mdBook

You can install `mdbook` using Cargo (Rust's package manager):

```bash
cargo install mdbook
```

Alternatively, you can download binaries from the [mdBook releases page](https://github.com/rust-lang/mdBook/releases).

### 2. Install mdbook-mermaid

This project uses Mermaid for diagrams. Install the preprocessor:

```bash
cargo install mdbook-mermaid
```

## Building the Book

The book is configured to build its output into the `/docs` directory at the project root.

### Build Command

To generate the HTML version of the book, run the following command from the `md-book` directory:

```bash
mdbook build
```

The generated files will be placed in `../docs`.

### Live Development

To preview changes as you edit, you can use the `serve` command:

```bash
mdbook serve --open
```

This will start a local web server and open the book in your default browser. It automatically reloads when you save changes to the markdown files.

## Project Structure

- `md-book/book.toml`: Configuration file for the book.
- `md-book/src/`: Contains the markdown source files.
- `md-book/src/SUMMARY.md`: The table of contents for the book.
- `docs/`: The generated HTML documentation (output directory).