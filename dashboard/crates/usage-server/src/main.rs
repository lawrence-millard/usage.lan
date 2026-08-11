mod aggregate;
mod readers;

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use chrono::Local;
use serde::Deserialize;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use usage_core::models::UsageReport;

#[derive(Clone)]
struct AppState {
    reader: Arc<readers::Reader>,
}

#[derive(Deserialize)]
struct UsageParams {
    agent: Option<String>,
    days: Option<i64>,
}

async fn usage_handler(
    State(state): State<AppState>,
    Query(params): Query<UsageParams>,
) -> Json<UsageReport> {
    let agent = params.agent.unwrap_or_else(|| "all".to_string());
    let days = params.days.unwrap_or(30);
    let reader = state.reader.clone();
    let out = tokio::task::spawn_blocking(move || reader.read())
        .await
        .unwrap_or_default();
    let report =
        aggregate::build_report(&out.records, out.sources, &agent, days, Local::now());
    Json(report)
}

async fn health() -> &'static str {
    "ok"
}

fn header_layer(name: &'static str, value: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        HeaderName::from_static(name),
        HeaderValue::from_static(value),
    )
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let dist = std::env::var("USAGE_DIST").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../usage-ui/dist").to_string()
    });
    log::info!("serving static files from {dist}");

    let state = AppState {
        reader: Arc::new(readers::Reader::new()),
    };

    let app = Router::new()
        .route("/api/usage", get(usage_handler))
        .route("/api/health", get(health))
        .fallback_service(ServeDir::new(&dist).append_index_html_on_directories(true))
        .layer(header_layer("cross-origin-embedder-policy", "require-corp"))
        .layer(header_layer("cross-origin-opener-policy", "same-origin"))
        .with_state(state);

    // TLS is optional: if USAGE_TLS_CERT + USAGE_TLS_KEY are set, serve HTTPS.
    let cert = std::env::var("USAGE_TLS_CERT").ok();
    let key = std::env::var("USAGE_TLS_KEY").ok();

    let port_env = std::env::var("USAGE_PORT").unwrap_or_default();
    let port: u16 = if let Ok(p) = port_env.parse::<u16>() {
        p
    } else {
        443
    };
    let addr = format!("0.0.0.0:{port}");
    let scheme = if cert.is_some() && key.is_some() { "https" } else { "http" };

    println!();
    println!("  AI usage dashboard running");
    println!("    -> {scheme}://usage.local/");
    println!("    -> {scheme}://localhost/");
    println!("  API: {scheme}://usage.local/api/usage");
    println!();

    if let (Some(cert_path), Some(key_path)) = (cert, key) {
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            std::path::Path::new(&cert_path),
            std::path::Path::new(&key_path),
        )
        .await
        .expect("failed to load TLS cert/key");
        axum_server::bind_rustls(addr.parse::<std::net::SocketAddr>().unwrap(), tls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .unwrap_or_else(|e| panic!("could not bind {addr}: {e}"));
        axum::serve(listener, app).await.unwrap();
    }
}
