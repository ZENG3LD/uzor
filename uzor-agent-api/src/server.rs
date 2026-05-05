//! axum server implementing the agent HTTP control plane.
//!
//! Routes (first pass):
//!
//! | Method | Path                  | Body / query                      | Returns                        |
//! |--------|-----------------------|-----------------------------------|--------------------------------|
//! | GET    | `/health`             | —                                 | `{"ok":true}`                  |
//! | GET    | `/state/tree`         | —                                 | [`AgentSnapshot`]              |
//! | GET    | `/state/widgets`      | —                                 | `Vec<WidgetSnapshot>`          |
//! | GET    | `/screenshot/:window` | —                                 | `image/png`                    |
//! | POST   | `/cmd`                | [`Command`] (tagged JSON enum)    | [`CommandReply`]               |
//! | POST   | `/input/click`        | `{window, x, y, button?}`         | [`CommandReply`]               |
//! | POST   | `/input/hover`        | `{window, x, y}`                  | [`CommandReply`]               |
//! | POST   | `/input/scroll`       | `{window, dx, dy}`                | [`CommandReply`]               |
//! | POST   | `/lm/click_widget`    | `{window, widget_id}`             | [`CommandReply`]               |
//! | POST   | `/lm/hover_widget`    | `{window, widget_id}`             | [`CommandReply`]               |
//! | POST   | `/lm/modal/open`      | `{window, modal_id}`              | [`CommandReply`]               |
//! | POST   | `/lm/modal/close`     | `{window, modal_id}`              | [`CommandReply`]               |
//! | POST   | `/lm/popup/open`      | `{window, popup_id}`              | [`CommandReply`]               |
//! | POST   | `/lm/popup/close`     | `{window, popup_id}`              | [`CommandReply`]               |
//! | POST   | `/lm/dropdown/open`   | `{window, dropdown_id}`           | [`CommandReply`]               |
//! | POST   | `/lm/dropdown/close`  | `{window, dropdown_id}`           | [`CommandReply`]               |
//! | POST   | `/lm/sidebar/toggle`  | `{window, sidebar_id}`            | [`CommandReply`]               |
//! | POST   | `/window/spawn`       | `{key, title, width, height, …}`  | [`CommandReply`]               |
//! | POST   | `/window/close`       | `{key}`                           | [`CommandReply`]               |
//! | POST   | `/lm/sync_mode`       | `{node_id, mode, group_id?}`      | [`CommandReply`]               |
//! | POST   | `/lm/style_preset`    | `{name}`                          | [`CommandReply`]               |
//!
//! The server runs on its own tokio multi-thread runtime in a
//! separate OS thread spawned by [`spawn_server`].

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::JoinHandle;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder;

use uzor::layout::agent::{AgentAction, AgentControlObj, Command, CommandReply, MouseButton};

// ── Shared state ─────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    control: AgentControlObj,
}

// ── Handle returned to the host ──────────────────────────────────────

pub struct AgentApiHandle {
    pub addr: SocketAddr,
    join: Option<JoinHandle<()>>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl AgentApiHandle {
    pub fn shutdown(self) { drop(self); }
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

pub fn spawn_server(
    control: AgentControlObj,
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
                Err(e) => { let _ = tx_addr.send(Err(e)); return; }
            };

            rt.block_on(async move {
                let addr: SocketAddr = ([127, 0, 0, 1], port).into();
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => { let _ = tx_addr.send(Err(e)); return; }
                };
                let bound_addr = match listener.local_addr() {
                    Ok(a) => a,
                    Err(e) => { let _ = tx_addr.send(Err(e)); return; }
                };
                eprintln!("[uzor-agent-api] listening on http://{}", bound_addr);
                let _ = tx_addr.send(Ok(bound_addr));

                let state = AppState { control };
                let app = Router::new()
                    .route("/health",                get(health))
                    .route("/state/tree",            get(state_tree))
                    .route("/state/widgets",         get(state_widgets))
                    .route("/log",                   get(log_since))
                    .route("/log/tail",              get(log_tail))
                    .route("/screenshot/:window",    get(screenshot_window))
                    .route("/cmd",                   post(post_cmd))
                    .route("/input/click",           post(post_input_click))
                    .route("/input/hover",           post(post_input_hover))
                    .route("/input/scroll",          post(post_input_scroll))
                    .route("/lm/click_widget",       post(post_click_widget))
                    .route("/lm/hover_widget",       post(post_hover_widget))
                    .route("/lm/modal/open",         post(post_open_modal))
                    .route("/lm/modal/close",        post(post_close_modal))
                    .route("/lm/popup/open",         post(post_open_popup))
                    .route("/lm/popup/close",        post(post_close_popup))
                    .route("/lm/dropdown/open",      post(post_open_dropdown))
                    .route("/lm/dropdown/close",     post(post_close_dropdown))
                    .route("/lm/sidebar/toggle",     post(post_toggle_sidebar))
                    .route("/window/spawn",          post(post_window_spawn))
                    .route("/window/close",          post(post_window_close))
                    .route("/lm/sync_mode",          post(post_set_sync_mode))
                    .route("/lm/style_preset",       post(post_apply_style_preset))
                    .route("/lm/window/tick_rate",   post(post_set_tick_rate))
                    // Blackbox routing — slot-id keyed.
                    .route("/blackboxes",                       get(blackbox_slots))
                    .route("/blackbox/:slot/widgets",           get(blackbox_widgets))
                    .route("/blackbox/:slot/state",             get(blackbox_state))
                    .route("/blackbox/:slot/action",            post(blackbox_action))
                    .route("/blackbox/:slot/click_widget",      post(blackbox_click_widget))
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

    Ok(AgentApiHandle { addr, join: Some(join), shutdown: shutdown_tx })
}

// ── handlers ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OkPayload { ok: bool }

async fn health() -> impl IntoResponse {
    Json(OkPayload { ok: true })
}

async fn state_tree(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.control.snapshot())
}

async fn state_widgets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.control.widgets())
}

#[derive(Deserialize)]
struct LogQuery {
    /// Return entries with `seq > since`.  Default 0.
    #[serde(default)]
    since: u64,
    /// Cap the response.  Default 500.
    #[serde(default = "default_log_limit")]
    limit: usize,
    /// Optional category prefix filter (e.g. `lm.`, `app.theme.`).
    #[serde(default)]
    prefix: Option<String>,
}

fn default_log_limit() -> usize { 500 }

async fn log_since(
    State(s): State<AppState>,
    Query(q): Query<LogQuery>,
) -> impl IntoResponse {
    let entries = s.control.log_since(q.since, q.limit);
    if let Some(p) = q.prefix.as_deref() {
        let filtered: Vec<_> = entries.into_iter()
            .filter(|e| e.category.starts_with(p))
            .collect();
        Json(filtered)
    } else {
        Json(entries)
    }
}

#[derive(Deserialize)]
struct LogTailQuery {
    /// Number of trailing entries.  Default 50.
    #[serde(default = "default_tail_n")]
    n: usize,
}

fn default_tail_n() -> usize { 50 }

async fn log_tail(
    State(s): State<AppState>,
    Query(q): Query<LogTailQuery>,
) -> impl IntoResponse {
    Json(s.control.log_tail(q.n))
}

async fn screenshot_window(
    State(s): State<AppState>,
    Path(window): Path<String>,
) -> Response {
    match s.control.screenshot_png(&window) {
        Some(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png")],
            bytes,
        ).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "screenshot not available for this window",
        ).into_response(),
    }
}

async fn post_cmd(
    State(s): State<AppState>,
    Json(cmd): Json<Command>,
) -> impl IntoResponse {
    forward(s, cmd).await
}

// ── pixel-mode input ────────────────────────────────────────────────

#[derive(Deserialize)]
struct ClickBody { window: String, x: f64, y: f64, button: Option<MouseButton> }

async fn post_input_click(State(s): State<AppState>, Json(b): Json<ClickBody>) -> impl IntoResponse {
    forward(s, Command::InjectClick {
        window: b.window, x: b.x, y: b.y,
        button: b.button.unwrap_or(MouseButton::Left),
    }).await
}

#[derive(Deserialize)]
struct HoverBody { window: String, x: f64, y: f64 }

async fn post_input_hover(State(s): State<AppState>, Json(b): Json<HoverBody>) -> impl IntoResponse {
    forward(s, Command::InjectHover { window: b.window, x: b.x, y: b.y }).await
}

#[derive(Deserialize)]
struct ScrollBody { window: String, dx: f64, dy: f64 }

async fn post_input_scroll(State(s): State<AppState>, Json(b): Json<ScrollBody>) -> impl IntoResponse {
    forward(s, Command::InjectScroll { window: b.window, dx: b.dx, dy: b.dy }).await
}

// ── semantic LM ops ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct WidgetBody { window: String, widget_id: String }

async fn post_click_widget(State(s): State<AppState>, Json(b): Json<WidgetBody>) -> impl IntoResponse {
    forward(s, Command::ClickWidget { window: b.window, widget_id: b.widget_id }).await
}

async fn post_hover_widget(State(s): State<AppState>, Json(b): Json<WidgetBody>) -> impl IntoResponse {
    forward(s, Command::HoverWidget { window: b.window, widget_id: b.widget_id }).await
}

#[derive(Deserialize)]
struct ModalBody { window: String, modal_id: String }
#[derive(Deserialize)]
struct PopupBody { window: String, popup_id: String }
#[derive(Deserialize)]
struct DropdownBody { window: String, dropdown_id: String }
#[derive(Deserialize)]
struct SidebarBody { window: String, sidebar_id: String }

async fn post_open_modal(State(s): State<AppState>, Json(b): Json<ModalBody>) -> impl IntoResponse {
    forward(s, Command::OpenModal { window: b.window, modal_id: b.modal_id }).await
}
async fn post_close_modal(State(s): State<AppState>, Json(b): Json<ModalBody>) -> impl IntoResponse {
    forward(s, Command::CloseModal { window: b.window, modal_id: b.modal_id }).await
}
async fn post_open_popup(State(s): State<AppState>, Json(b): Json<PopupBody>) -> impl IntoResponse {
    forward(s, Command::OpenPopup { window: b.window, popup_id: b.popup_id }).await
}
async fn post_close_popup(State(s): State<AppState>, Json(b): Json<PopupBody>) -> impl IntoResponse {
    forward(s, Command::ClosePopup { window: b.window, popup_id: b.popup_id }).await
}
async fn post_open_dropdown(State(s): State<AppState>, Json(b): Json<DropdownBody>) -> impl IntoResponse {
    forward(s, Command::OpenDropdown { window: b.window, dropdown_id: b.dropdown_id }).await
}
async fn post_close_dropdown(State(s): State<AppState>, Json(b): Json<DropdownBody>) -> impl IntoResponse {
    forward(s, Command::CloseDropdown { window: b.window, dropdown_id: b.dropdown_id }).await
}
async fn post_toggle_sidebar(State(s): State<AppState>, Json(b): Json<SidebarBody>) -> impl IntoResponse {
    forward(s, Command::ToggleSidebar { window: b.window, sidebar_id: b.sidebar_id }).await
}

// ── window lifecycle ────────────────────────────────────────────────

#[derive(Deserialize)]
struct SpawnWindowBody {
    key: String, title: String, width: u32, height: u32,
    background: Option<u32>, decorations: Option<bool>,
}

async fn post_window_spawn(State(s): State<AppState>, Json(b): Json<SpawnWindowBody>) -> impl IntoResponse {
    forward(s, Command::SpawnWindow {
        key: b.key, title: b.title, width: b.width, height: b.height,
        background: b.background, decorations: b.decorations,
    }).await
}

#[derive(Deserialize)]
struct CloseWindowBody { key: String }

async fn post_window_close(State(s): State<AppState>, Json(b): Json<CloseWindowBody>) -> impl IntoResponse {
    forward(s, Command::CloseWindow { key: b.key }).await
}

// ── LM-root ops ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SyncModeBody { node_id: String, mode: String, group_id: Option<u64> }

async fn post_set_sync_mode(State(s): State<AppState>, Json(b): Json<SyncModeBody>) -> impl IntoResponse {
    forward(s, Command::SetSyncMode { node_id: b.node_id, mode: b.mode, group_id: b.group_id }).await
}

#[derive(Deserialize)]
struct PresetBody { name: String }

async fn post_apply_style_preset(State(s): State<AppState>, Json(b): Json<PresetBody>) -> impl IntoResponse {
    forward(s, Command::ApplyStylePreset { name: b.name }).await
}

#[derive(Deserialize)]
struct TickRateBody { window: String, mode: String, fps: Option<u32> }

async fn post_set_tick_rate(
    State(s): State<AppState>,
    Json(b): Json<TickRateBody>,
) -> impl IntoResponse {
    forward(s, Command::SetTickRate { window: b.window, mode: b.mode, fps: b.fps }).await
}

// ── Blackbox endpoints ──────────────────────────────────────────────

async fn blackbox_slots(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.control.blackbox_slots())
}

async fn blackbox_widgets(
    State(s): State<AppState>,
    Path(slot): Path<String>,
) -> Response {
    match s.control.blackbox_widgets(&slot) {
        Some(w) => (StatusCode::OK, Json(w)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(CommandReply::err("unknown slot"))).into_response(),
    }
}

async fn blackbox_state(
    State(s): State<AppState>,
    Path(slot): Path<String>,
) -> Response {
    match s.control.blackbox_state(&slot) {
        Some(v) => (StatusCode::OK, Json(v)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(CommandReply::err("unknown slot"))).into_response(),
    }
}

async fn blackbox_action(
    State(s): State<AppState>,
    Path(slot): Path<String>,
    Json(action): Json<AgentAction>,
) -> Response {
    let s_clone = s.clone();
    let slot_clone = slot.clone();
    let action_name = action.name.clone();
    let result = tokio::task::spawn_blocking(move || {
        s_clone.control.blackbox_action(&slot_clone, action)
    }).await;
    let reply = match result {
        Ok(Some(r)) => r,
        Ok(None) => return (
            StatusCode::NOT_FOUND,
            Json(CommandReply::err("unknown slot")),
        ).into_response(),
        Err(_) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandReply::err("blackbox action panicked")),
        ).into_response(),
    };
    // If the action returned a `log_payload`, push it into the
    // merged agent log under category `<slot>.<action_name>`.
    if reply.ok {
        if let Some(payload) = reply.log_payload.clone() {
            let category = format!("{}.{}", slot, action_name);
            let window = s.control.snapshot().root.current_window;
            let _ = s.control.dispatch(Command::LogPush { category, payload, window });
        }
    }
    (StatusCode::OK, Json(reply)).into_response()
}

#[derive(Deserialize)]
struct ClickBlackboxWidgetBody {
    /// Optional override; defaults to whatever LM marks as
    /// `current_window`.
    window: Option<String>,
    sub_id: String,
}

async fn blackbox_click_widget(
    State(s): State<AppState>,
    Path(slot): Path<String>,
    Json(b): Json<ClickBlackboxWidgetBody>,
) -> Response {
    let window = match b.window {
        Some(w) => w,
        None => match s.control.snapshot().root.current_window {
            Some(w) => w,
            None => return (
                StatusCode::BAD_REQUEST,
                Json(CommandReply::err("no current window — pass `window` in body")),
            ).into_response(),
        },
    };
    forward(s, Command::BlackboxClickWidget {
        window,
        slot_id: slot,
        sub_id: b.sub_id,
    }).await
}

// ── shared forwarder ────────────────────────────────────────────────

async fn forward(s: AppState, cmd: Command) -> Response {
    // Dispatch is sync — push it onto a blocking pool so we don't
    // block a tokio worker thread if the WM takes time to apply.
    let control = Arc::clone(&s.control);
    let reply = tokio::task::spawn_blocking(move || control.dispatch(cmd))
        .await
        .unwrap_or_else(|e| CommandReply::err(format!("dispatch panicked: {e}")));
    (StatusCode::OK, Json(reply)).into_response()
}

#[allow(dead_code)]
fn _force_arc_use(_: &Arc<()>) {}
