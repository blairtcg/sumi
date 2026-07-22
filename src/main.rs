use std::{future::pending, net::SocketAddr, panic, sync::Arc, time::Duration};

use axum::{routing::get, Router, serve};
use mimalloc::MiMalloc;
#[cfg(unix)]
use tokio::signal::unix::{signal as unix_signal, SignalKind};
use tokio::{net::TcpListener, signal, time::timeout};
use tracing_subscriber::{fmt, EnvFilter};

use sumi::{
    config::Config,
    logger::LogFormatter,
    renderer::CardRenderer,
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
async fn main() {
    const COLOR_SUMI: &str = "\x1b[38;2;255;180;162m";
    const RESET: &str = "\x1b[0m";

    aegis();

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("sumi=debug,info"));

    fmt().with_env_filter(filter).event_format(LogFormatter).init();

    let welcomer = include_str!("ascii.txt");
    println!();
    for line in welcomer.lines() {
        println!("{COLOR_SUMI}{line}{RESET}");
    }

    let cfg = Config::from_env();

    let renderer = match CardRenderer::new(&cfg.cards_dir) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to wake sumi up..\n      reason: {}", e);
            std::process::exit(1);
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
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("sumi failed to bind to port..\n      reason: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = serve(listener, app).with_graceful_shutdown(nap()).await {
        tracing::error!("sumi's server crashed..\n      reason: {}", e);
        std::process::exit(1);
    }

    tracing::info!("sumi is going to sleep, finishing tasks..");
    if timeout(Duration::from_secs(10), state.wait_for_tasks_to_finish()).await.is_ok() {
        tracing::info!("sumi is sleeping.. zZz");
    } else {
        tracing::error!("sumi refused to sleep in time.. pulling the blanket anyway!");
    }
}

// https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
// https://tokio.rs/tokio/topics/shutdown
async fn nap() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("sumi couldn't set up ctrl+c..");
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sigterm = match unix_signal(SignalKind::terminate()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!("sumi couldn't hear the alarm..\n      reason: {e}");
                None
            }
        };

        let mut sigquit = match unix_signal(SignalKind::quit()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!("sumi couldn't hear the alarm..\n      reason: {e}");
                None
            }
        };

        tokio::select! {
            () = async {
                if let Some(ref mut sig) = sigterm {
                    sig.recv().await;
                } else {
                    pending::<()>().await;
                }
            } => {},
            () = async {
                if let Some(ref mut sig) = sigquit {
                    sig.recv().await;
                } else {
                    pending::<()>().await;
                }
            } => {},
        }
    };

    #[cfg(not(unix))]
    let terminate = pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("sumi is yawning, time to take a nap..");
}
