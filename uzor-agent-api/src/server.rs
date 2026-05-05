//! axum server implementing the agent HTTP control plane.
//!
//! Routes (first pass):
//!
//! | Method | Path             | Body                           | Returns                |
//! |--------|------------------|--------------------------------|------------------------|
//! | GET    | `/health`        | —                              | `{"ok":true}`          |
//! | GET    | `/state/tree`    | —                              | [`AgentSnapshot`]      |
//! | GET    | `/state/widgets` | —                              | `Vec<WidgetSnapshot>`  |
//! | POST   | `/cmd`           | [`Command`] (tagged JSON enum) | [`CommandReply`]       |
//!
//! The server runs on its own tokio multi-thread runtime in a separate
//! OS thread spawned by [`spawn_server`].  Returning the
//! [`AgentApiHandle`] keeps the runtime alive for the lifetime of the
//! window manager.

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::JoinHandle;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Serialize;
use tokio::runtime::Builder;

use crate::command::{Command, CommandReply};
use crate::control::AgentControlArc;

// ── Shared state ─────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    control: AgentControlArc,
}

// ── Handle returned to the host (kept for lifetime) ──────────────────

/// Owned handle returned from [`spawn_server`].  Dropping it shuts down
/// the agent server.
pub struct AgentApiHandle {
    pub addr: SocketAddr,
    /// Tokio runtime thread.  Joined when the handle is dropped.
    join: Option<JoinHandle<()>>,
    /// Shutdown signal.
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl AgentApiHandle {
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for AgentApiHandle {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(t) = self.join.take() {
            let _ = t.join();
        }
    }
}

// ── Public entry point ───────────────────────────────────────────────

/// Spawn the agent API server on `127.0.0.1:port`.  Returns once the
/// listener is bound; the server runs on its own thread.
///
/// Errors only on bind failure (port already taken).
pub fn spawn_server(
    control: AgentControlArc,
    port: u16,
) -> std::io::Result<AgentApiHandle> {
    let (tx_addr, rx_addr) = std::sync::mpsc::channel::<std::io::Result<SocketAddr>>();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let join = std::thread::Builder::new()
        .name("uzor-agent-api".into())
        .spawn(move || {
            let rt = match Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .thread_name("uzor-agent-api-worker")
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx_addr.send(Err(e));
                    return;
                }
            };

            rt.block_on(async move {
                let addr: SocketAddr = ([127, 0, 0, 1], port).into();
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = tx_addr.send(Err(e));
                        return;
                    }
                };
                let bound_addr = match listener.local_addr() {
                    Ok(a) => a,
                    Err(e) => {
                        let _ = tx_addr.send(Err(e));
                        return;
                    }
                };
                eprintln!("[uzor-agent-api] listening on http://{}", bound_addr);
                let _ = tx_addr.send(Ok(bound_addr));

                let state = AppState { control };
                let app = Router::new()
                    .route("/health",         get(health))
                    .route("/state/tree",     get(state_tree))
                    .route("/state/widgets",  get(state_widgets))
                    .route("/cmd",            post(post_cmd))
                    // Convenience aliases for common commands.
                    .route("/input/click",    post(post_input_click))
                    .route("/input/hover",    post(post_input_hover))
                    .route("/input/scroll",   post(post_input_scroll))
                    .route("/window/spawn",   post(post_window_spawn))
                    .route("/window/close",   post(post_window_close))
                    .route("/lm/sync_mode",   post(post_set_sync_mode))
                    .route("/lm/style_preset",post(post_apply_style_preset))
                    .with_state(state);

                let mut shutdown_rx = shutdown_rx.clone();
                let server = axum::serve(listener, app);
                let _ = server
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_rx.changed().await;
                    })
                    .await;
            });
        })?;

    let addr = rx_addr.recv()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "agent-api thread died"))??;

    Ok(AgentApiHandle {
        addr,
        join: Some(join),
        shutdown: shutdown_tx,
    })
}

// ── handlers ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OkPayload {
    ok: bool,
}

async fn health() -> impl IntoResponse {
    Json(OkPayload { ok: true })
}

async fn state_tree(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.control.latest_snapshot())
}

async fn state_widgets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.control.widget_list())
}

async fn post_cmd(
    State(s): State<AppState>,
    Json(cmd): Json<Command>,
) -> impl IntoResponse {
    let rx = s.control.dispatch(cmd);
    match rx.await {
        Ok(reply) => (StatusCode::OK, Json(reply)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandReply::err("manager dropped reply channel")),
        )
            .into_response(),
    }
}

// ── convenience aliases (each accepts the inner payload directly) ───

#[derive(serde::Deserialize)]
struct ClickBody { window: String, x: f64, y: f64, button: Option<crate::command::MouseButton> }

async fn post_input_click(
    State(s): State<AppState>,
    Json(b): Json<ClickBody>,
) -> impl IntoResponse {
    let cmd = Command::InjectClick {
        window: b.window,
        x: b.x,
        y: b.y,
        button: b.button.unwrap_or(crate::command::MouseButton::Left),
    };
    forward(s, cmd).await
}

#[derive(serde::Deserialize)]
struct HoverBody { window: String, x: f64, y: f64 }

async fn post_input_hover(
    State(s): State<AppState>,
    Json(b): Json<HoverBody>,
) -> impl IntoResponse {
    let cmd = Command::InjectHover { window: b.window, x: b.x, y: b.y };
    forward(s, cmd).await
}

#[derive(serde::Deserialize)]
struct ScrollBody { window: String, dx: f64, dy: f64 }

async fn post_input_scroll(
    State(s): State<AppState>,
    Json(b): Json<ScrollBody>,
) -> impl IntoResponse {
    let cmd = Command::InjectScroll { window: b.window, dx: b.dx, dy: b.dy };
    forward(s, cmd).await
}

#[derive(serde::Deserialize)]
struct SpawnWindowBody {
    key: String,
    title: String,
    width: u32,
    height: u32,
    background: Option<u32>,
    decorations: Option<bool>,
}

async fn post_window_spawn(
    State(s): State<AppState>,
    Json(b): Json<SpawnWindowBody>,
) -> impl IntoResponse {
    let cmd = Command::SpawnWindow {
        key: b.key, title: b.title, width: b.width, height: b.height,
        background: b.background, decorations: b.decorations,
    };
    forward(s, cmd).await
}

#[derive(serde::Deserialize)]
struct CloseWindowBody { key: String }

async fn post_window_close(
    State(s): State<AppState>,
    Json(b): Json<CloseWindowBody>,
) -> impl IntoResponse {
    forward(s, Command::CloseWindow { key: b.key }).await
}

#[derive(serde::Deserialize)]
struct SyncModeBody { node_id: String, mode: String, group_id: Option<u64> }

async fn post_set_sync_mode(
    State(s): State<AppState>,
    Json(b): Json<SyncModeBody>,
) -> impl IntoResponse {
    forward(s, Command::SetSyncMode {
        node_id: b.node_id, mode: b.mode, group_id: b.group_id
    }).await
}

#[derive(serde::Deserialize)]
struct PresetBody { name: String }

async fn post_apply_style_preset(
    State(s): State<AppState>,
    Json(b): Json<PresetBody>,
) -> impl IntoResponse {
    forward(s, Command::ApplyStylePreset { name: b.name }).await
}

async fn forward(s: AppState, cmd: Command) -> axum::response::Response {
    let rx = s.control.dispatch(cmd);
    match rx.await {
        Ok(reply) => (StatusCode::OK, Json(reply)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandReply::err("manager dropped reply channel")),
        ).into_response(),
    }
}

#[allow(dead_code)]
fn _force_arc_use(_: &Arc<()>) {}
