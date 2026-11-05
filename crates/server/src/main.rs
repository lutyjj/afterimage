use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;

use afterimage_server::clock::SystemClock;
use afterimage_server::config::Config;
use afterimage_server::device_api;
use afterimage_server::devices::Registry;
use afterimage_server::secrets::OsEntropy;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    // Colour only for a terminal: a service journal would store the escape codes.
    tracing_subscriber::fmt()
        .with_ansi(std::io::stdout().is_terminal())
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    std::fs::create_dir_all(&config.data_dir)?;
    let registry = Registry::open(
        &config.data_dir.join("afterimage.db"),
        Arc::new(SystemClock),
        Arc::new(OsEntropy),
    )
    .await?;
    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(bind = %config.bind, data = %config.data_dir.display(), "serving the TRMNL device API");
    device_api::serve(listener, Arc::new(registry), shutdown()).await?;
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
