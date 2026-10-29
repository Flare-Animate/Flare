//! `flare-mcp`: serves one Flare project to an AI agent over stdio.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use flare_mcp::{FlareServer, Permissions};
use rmcp::{ServiceExt, transport::stdio};

/// Serve a Flare project over the Model Context Protocol on stdin and stdout.
#[derive(Parser)]
#[command(version, about)]
struct Arguments {
    /// The .flare project to serve.
    project: PathBuf,
    /// Let the agent change the project and save it. Without this the server is read-only.
    #[arg(long)]
    allow_write: bool,
    /// Directory the agent may export PNGs into. Exports are off without it.
    #[arg(long)]
    export_dir: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let arguments = Arguments::parse();
    let permissions = Permissions {
        write: arguments.allow_write,
        export_dir: arguments.export_dir,
    };
    let server = FlareServer::open(arguments.project.clone(), permissions)
        .with_context(|| format!("cannot open {}", arguments.project.display()))?;
    // Stdout carries only protocol; anything for a person goes to stderr.
    let running = server
        .serve(stdio())
        .await
        .context("start the MCP session")?;
    running.waiting().await.context("MCP session")?;
    Ok(())
}
