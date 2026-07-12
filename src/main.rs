mod config;
mod error;
mod logger;
mod renderer;
mod routes;
mod stats;

use std::{error::Error, net::SocketAddr, panic, sync::Arc};

use axum::{Router, routing::get, serve};
use mimalloc::MiMalloc;
use tokio::{net::TcpListener, signal};
use tracing_subscriber::{EnvFilter, fmt};

use crate::{
    config::Config,
    logger::LogFormatter,
    renderer::{CardRenderer, print::init_font},
    routes::{handle_metrics, handle_render_drop},
};

// aegis sets up a panic hook so we can format sys errors cleanly
// as unexpected panics will give long unformatted backtraces.
fn aegis() {
    panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "sumi just crashed ..?"
        };

        if let Some(loc) = info.location() {
            tracing::error!(
                "sumi got sleepy..\n      reason: {}\n      location: {}:{}",
                msg,
                loc.file(),
                loc.line()
            );
        } else {
            tracing::error!("sumi got sleepy..\n      reason: {}", msg);
        }
    }));
}

// we use microsoft mimalloc as it handles memory better
// it will only help when tokio is running multi threads
// https://github.com/purpleprotocol/mimalloc_rust
// https://github.com/microsoft/mimalloc
// https://www.microsoft.com/en-us/research/wp-content/uploads/2019/06/mimalloc-tr-v1.pdf
#[global_allocator]
static ALLOC: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    aegis();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("sumi=debug,info"));

    fmt().with_env_filter(filter).event_format(LogFormatter).init();

    let welcomer = include_str!("ascii.txt");
    let colored_welcomer = welcomer.replace('\n', "\n\x1b[38;2;255;180;162m");
    println!("\n\x1b[38;2;255;180;162m{colored_welcomer}\x1b[0m");

    let cfg = Config::from_env();

    tracing::info!("baking in lexend deca font..");
    init_font();

    let renderer = match CardRenderer::new(&cfg.cards_dir) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to wake sumi up..\n      reason: {}", e);
            return Ok(());
        }
    };
    let state = Arc::new(renderer);

    state.card_cache.start_prewarm();

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/metrics", get(handle_metrics))
        .route("/render/drop", get(handle_render_drop))
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], cfg.port));
    let listener = TcpListener::bind(addr).await?;

    serve(listener, app).with_graceful_shutdown(nap()).await?;

    tracing::info!("sumi is going to sleep, finishing tasks..");
    state.wait_for_tasks_to_finish().await;

    Ok(())
}

async fn nap() {
    if let Err(e) = signal::ctrl_c().await {
        tracing::error!("sumi failed to sleep..?({})", e);
    }
    tracing::info!("you gave sumi way too much caffeine..");
}
