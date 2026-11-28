//! Klue Server: Klue on your own VPS, in a Docker container. It checks lines around the
//! clock, sends alerts and reports, and keeps every computer connected to it in step.
//! See docs/server.md.
//!
//!     klue-server            run (`--demo` for made-up lines, nothing saved)
//!     klue-server key        show the key to paste into Klue
//!     klue-server new-key    replace the key; every device needs the new one
//!     klue-server health     exit 0 if the server answers (Docker's health check)
//!
//! `KLUE_DATA` (default `/data`) is where it keeps everything, `KLUE_PORT` (8443) where it
//! listens, `KLUE_ADDRESS` the address devices reach it at, to put in the key.

mod api;
mod channels;
mod devices;
mod identity;
mod state;
mod sync;
mod vault;

use std::{net::SocketAddr, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use axum_server::{Handle, tls_rustls::RustlsConfig};
use klue_core::store::Store;
use klue_engine::{Engine, remote::PORT};
use tracing_subscriber::EnvFilter;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,klue_server=info,klue_core=info"))).init();
    let data_dir = PathBuf::from(std::env::var_os("KLUE_DATA").unwrap_or_else(|| "/data".into()));
    let port: u16 = std::env::var("KLUE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(PORT);
    let demo = args.iter().any(|a| a == "--demo");
    let result = match args.iter().find(|a| !a.starts_with("--")).map(String::as_str) {
        None | Some("run") => run(data_dir, port, demo),
        Some("key") => show_key(&data_dir, port, false),
        Some("new-key") => show_key(&data_dir, port, true),
        Some("health") => health(port),
        Some(other) => Err(format!("unknown command {other}; try key, new-key or health")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("klue-server: {e}");
            ExitCode::FAILURE
        }
    }
}

fn show_key(data_dir: &std::path::Path, port: u16, new: bool) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let id = identity::certificate(data_dir).map_err(|e| e.to_string())?;
    let secret = if new { identity::new_secret(data_dir) } else { identity::secret(data_dir) }.map_err(|e| e.to_string())?;
    println!("{}", identity::banner(&identity::key(&secret, &id.fingerprint), port));
    if new {
        println!("Restart the server for the new key to take effect: docker restart klue-server");
    }
    Ok(())
}

/// Answers if something listens on the port: enough for Docker to know it's up.
fn health(port: u16) -> Result<(), String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map(|_| ()).map_err(|e| format!("nothing answers on {addr}: {e}"))
}

fn run(data_dir: PathBuf, port: u16, demo: bool) -> Result<(), String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("can't use {}: {e}", data_dir.display()))?;
    let id = identity::certificate(&data_dir).map_err(|e| format!("can't make the certificate: {e}"))?;
    let secret = identity::secret(&data_dir).map_err(|e| format!("can't make the key: {e}"))?;
    let vault = Arc::new(vault::Vault::open(&data_dir).map_err(|e| format!("can't open the vault: {e}"))?);
    // Demo mode keeps its made-up lines in memory only.
    let lines_dir = if demo { std::env::temp_dir().join("klue-server-demo") } else { data_dir.clone() };
    let store = Store::open(&lines_dir).map_err(|e| format!("can't use {}: {e}", lines_dir.display()))?;

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let engine = Engine::new(store, Box::new(vault::Passwords(vault.clone())), demo, VERSION);
        let state = state::State::new(engine, vault, secret.clone(), data_dir);
        println!("{}", identity::banner(&identity::key(&secret, &id.fingerprint), port));
        tracing::info!(version = VERSION, demo, "Klue Server starting on port {port}");

        tokio::spawn(state::follow(state.clone()));
        tokio::spawn(state::schedule(state.clone()));
        tokio::spawn(state::relay_captchas(state.clone()));
        channels::start(&state);

        let tls = RustlsConfig::from_pem(id.cert_pem.into_bytes(), id.key_pem.into_bytes()).await.map_err(|e| format!("bad certificate: {e}"))?;
        let handle = Handle::new();
        tokio::spawn(shutdown(handle.clone()));
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        axum_server::bind_rustls(addr, tls).handle(handle).serve(api::router(state).into_make_service()).await.map_err(|e| format!("can't listen on {addr}: {e}"))
    })
}

/// Stops on Ctrl+C or `docker stop`, letting requests in flight finish.
async fn shutdown(handle: Handle<SocketAddr>) {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("stopping");
    handle.graceful_shutdown(Some(Duration::from_secs(10)));
}
