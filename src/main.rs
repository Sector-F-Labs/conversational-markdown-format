use clap::{Parser, Subcommand};
use cmf::terminal_renderer::MarkdownRenderer;
use cmf::Document;
use std::fs;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "cmf")]
#[command(about = "Conversational Markdown Format - parse and convert LLM conversations")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect if a file contains CMF content
    Detect {
        /// Path to the markdown file
        file: String,
    },
    /// Check CMF conformance
    Check {
        /// Path to the markdown file
        file: String,
    },
    /// Render markdown to terminal with ANSI colors
    Render {
        /// Path to the markdown file
        file: String,
    },
    /// Convert to OpenAI Chat Completions format
    #[command(name = "to-openai-chat")]
    ToOpenaiChat {
        /// Path to the markdown file
        file: String,
    },
    /// Convert to OpenAI Responses API format
    #[command(name = "to-openai-responses")]
    ToOpenaiResponses {
        /// Path to the markdown file
        file: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Detect { file } => cmd_detect(&file),
        Commands::Check { file } => cmd_check(&file),
        Commands::Render { file } => cmd_render(&file),
        Commands::ToOpenaiChat { file } => cmd_to_openai_chat(&file),
        Commands::ToOpenaiResponses { file } => cmd_to_openai_responses(&file),
    }
}

fn read_file(path: &str) -> Result<String, ExitCode> {
    fs::read_to_string(path).map_err(|e| {
        eprintln!("error: {}: {}", path, e);
        ExitCode::FAILURE
    })
}

fn cmd_detect(file: &str) -> ExitCode {
    let content = match read_file(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if Document::is_valid_cmf(&content) {
        let doc = Document::parse(&content);
        println!("{} turns", doc.turns.len());
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn cmd_check(file: &str) -> ExitCode {
    let content = match read_file(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let issues = Document::check(&content);
    if issues.is_empty() {
        // Rule of Silence: say nothing on success
        ExitCode::SUCCESS
    } else {
        for issue in issues {
            eprintln!("{}:{}: {}", file, issue.line, issue.message);
        }
        ExitCode::FAILURE
    }
}

fn cmd_render(file: &str) -> ExitCode {
    let content = match read_file(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let renderer = MarkdownRenderer::new();
    let rendered = renderer.render(&content);
    print!("{}", rendered);
    ExitCode::SUCCESS
}

fn cmd_to_openai_chat(file: &str) -> ExitCode {
    let content = match read_file(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let doc = Document::parse(&content);
    let messages = doc.to_openai_chat();
    match serde_json::to_string_pretty(&messages) {
        Ok(json) => {
            println!("{}", json);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn cmd_to_openai_responses(file: &str) -> ExitCode {
    let content = match read_file(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let doc = Document::parse(&content);
    let messages = doc.to_openai_responses();
    match serde_json::to_string_pretty(&messages) {
        Ok(json) => {
            println!("{}", json);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
