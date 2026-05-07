mod config;
mod renderer;
mod routes;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::renderer::CardRenderer;

// we use microsoft mimalloc as it handles memory better
// it will only help when tokio is running multi threads
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
        .block_on(async {
            let cfg = config::Config::load();
            log::info!("!! starting sumi on port {}", cfg.port);

            log::info!("baking in lexend deca font..");
            crate::renderer::canvas::init_font();

            let renderer =
                CardRenderer::new(cfg.cards_dir.clone()).expect("failed to wake sumi up..");
            let state = Arc::new(renderer);

            // prewarm cache di belakang
            state.card_cache.start_prewarm();

            let app = Router::new()
                .route("/health", get(|| async { "OK" }))
                .route("/metrics", get(routes::handle_metrics))
                .route("/render/drop", get(routes::handle_render_drop))
                .with_state(state);

            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], cfg.port));
            let listener = tokio::net::TcpListener::bind(addr).await?;

            log::info!("server ready at http://{}", addr);

            axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;

            Ok(())
        })
}

async fn shutdown() {
    tokio::signal::ctrl_c().await.expect("ctrl+c");
    log::info!("you gave blair way too much caffeine..");
}
