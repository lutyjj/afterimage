use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use afterimage_server::app;
use afterimage_server::clock::SystemClock;
use afterimage_server::config::Config;
use afterimage_server::keys::OsEntropy;
use afterimage_server::secrets::Secrets;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let config = Config::parse();
    // Colour only for a terminal: a service journal would store the escape codes.
    tracing_subscriber::fmt()
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    match run(&config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let app = app::build(
        config,
        Secrets::from_env(),
        Arc::new(SystemClock),
        Arc::new(OsEntropy),
    )
    .await?;
    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(
        bind = %config.bind,
        data = %config.data_dir.display(),
        web = ?config.web_dir,
        "serving"
    );
    // The generated `serve` bounds how long a client may take to send its request
    // head, which matters for devices on an untrusted network.
    afterimage_device_api::serve(listener, tower::make::Shared::new(app))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Completes on SIGINT or SIGTERM.
async fn shutdown() {
    let interrupt = tokio::signal::ctrl_c();
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("the process can listen for SIGTERM");
    tokio::select! {
        _ = interrupt => {}
        _ = terminate.recv() => {}
    }
    tracing::info!("shutting down");
}
