//! URX profiling rig — backend A/B/C/D harness with agent control.
//!
//! Boots a window with a pinned reference scene (rects, lines, text,
//! N-instance batch) and 4 backend buttons on top. Click a backend, the
//! runtime hot-switches via `RenderControl::set_backend`. FPS + frame-time
//! readout live in the corner.
//!
//! Mini agent HTTP server on 127.0.0.1:17490 mirrors the buttons:
//!
//!   GET  /urx/state          → { backend, available[], fps, frame_ms, frames }
//!   POST /urx/backend/<name> → switch backend (name = "vello_gpu" / "vello_hybrid"
//!                              / "instanced_wgpu" / "tiny_skia" / "vello_cpu")
//!
//! Purpose: drive backend A/B over HTTP, scrape numbers, no manual click.
//! Forms the baseline for all URX-family perf comparisons (Phase 0 →
//! Phase 1 (urx-core) → Phase 2 (urx-wgpu) → … vs current vello stack).
//!
//! Run:  cargo run -p uzor-examples --bin urx-smoke-instanced --release

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use uzor::core::types::Rect;
use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::framework::widgets::lm;
use uzor::platform::types::{CornerStyle, RenderBackend};
use uzor::types::unsafe_widget_id;
use uzor_desktop::AppRun as _;

/// Shared HTTP-driven backend switch + readout. Main thread reads/writes
/// these every `ui()`; HTTP thread mutates `requested_backend` on POST,
/// reads the rest for GET.
#[derive(Default)]
struct Shared {
    /// Set by HTTP POST, consumed by next `ui()` call.
    requested_backend: Option<RenderBackend>,
    /// Last-known backend + fps + frame_ms (mirrors WindowCtx state).
    active:            Option<RenderBackend>,
    fps:               f32,
    frame_ms:          f32,
    frames:            u64,
}

struct SmokeApp {
    shared: Arc<Mutex<Shared>>,
}

const BTN_IDS: &[(&str, RenderBackend, &str)] = &[
    ("urx:btn:vello_gpu",      RenderBackend::VelloGpu,      "VelloGpu"),
    ("urx:btn:instanced_wgpu", RenderBackend::InstancedWgpu, "InstancedWgpu"),
    ("urx:btn:vello_hybrid",   RenderBackend::VelloHybrid,   "VelloHybrid"),
    ("urx:btn:vello_cpu",      RenderBackend::VelloCpu,      "VelloCpu"),
    ("urx:btn:tiny_skia",      RenderBackend::TinySkia,      "TinySkia"),
];

impl App for SmokeApp {
    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        let viewport = win.layout.last_window()
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 880.0, height: 720.0 });

        // ── HTTP-requested backend switch ────────────────────────────────
        {
            let mut s = self.shared.lock().unwrap();
            if let Some(b) = s.requested_backend.take() {
                win.render_control.set_backend(b);
                eprintln!("[urx-smoke] HTTP requested backend → {}", b.label());
            }
        }

        let active = win.render_control.active_backend();
        let available = win.render_control.available_backends();
        let fps      = win.render_control.measured_fps();
        let frame_ms = win.render_control.last_frame_time_ms();
        let frames   = win.render_control.frame_count();

        // ── Mirror to shared state for the HTTP /urx/state endpoint ─────
        {
            let mut s = self.shared.lock().unwrap();
            s.active   = Some(active);
            s.fps      = fps;
            s.frame_ms = frame_ms;
            s.frames   = frames;
        }

        // ── Top strip: backend buttons ──────────────────────────────────
        let strip_h    = 36.0;
        let btn_pad    = 6.0;
        let btn_w      = 110.0;
        for (i, &(id_str, backend, label)) in BTN_IDS.iter().enumerate() {
            let x = btn_pad + (i as f64) * (btn_w + btn_pad);
            let y = btn_pad;
            let r = Rect { x, y, width: btn_w, height: strip_h - 2.0 * btn_pad };

            let in_pool = available.contains(&backend);
            let id = unsafe_widget_id(id_str);
            let was_clicked = win.layout.was_clicked(&id);
            if was_clicked && in_pool {
                win.render_control.set_backend(backend);
                eprintln!("[urx-smoke] click → {}", backend.label());
            }

            // Color: active = green, available = grey, missing-from-pool = dim red.
            let bg = if backend == active {
                "#3fb950"
            } else if in_pool {
                "#30363d"
            } else {
                "#5a1f1f"
            };
            win.render.set_fill_color(bg);
            win.render.fill_rect(r.x, r.y, r.width, r.height);
            win.render.set_stroke_color("#8b949e");
            win.render.set_stroke_width(1.0);
            win.render.stroke_rect(r.x, r.y, r.width, r.height);

            // Register the click region with LM so was_clicked works.
            lm::text(
                unsafe_widget_id(format!("{}:lbl", id_str).as_str()),
                Rect { x: r.x + 6.0, y: r.y + 4.0, width: r.width - 12.0, height: r.height - 8.0 },
                label,
            )
            .build(win.layout, win.render);
        }

        // ── HUD: backend + FPS + frame time + frame counter ─────────────
        let hud_y = strip_h + 4.0;
        let hud_text = format!(
            "active: {}   fps: {:>5.1}   frame: {:>5.2} ms   frames: {}",
            active.label(), fps, frame_ms, frames,
        );
        lm::text(
            unsafe_widget_id("urx:hud"),
            Rect { x: 12.0, y: hud_y, width: viewport.width as f64 - 24.0, height: 20.0 },
            hud_text.as_str(),
        )
        .build(win.layout, win.render);

        // ── Pinned reference scene (everything below the HUD) ──────────
        // Same scene under every backend → apples-to-apples comparison.

        let scene_top = hud_y + 28.0;

        // Group 1: solid filled rects (3) — QuadInstance batch
        for (i, color) in ["#1f6feb", "#a371f7", "#fb6c2c"].iter().enumerate() {
            let x = 20.0 + (i as f64) * 220.0;
            win.render.set_fill_color(color);
            win.render.fill_rect(x, scene_top, 200.0, 80.0);
        }

        // Group 2: stroke rects (3) — LineInstance via border path
        for (i, color) in ["#3fb950", "#58a6ff", "#f0883e"].iter().enumerate() {
            let x = 20.0 + (i as f64) * 220.0;
            let y = scene_top + 100.0;
            win.render.set_stroke_color(color);
            win.render.set_stroke_width(3.0);
            win.render.stroke_rect(x, y, 200.0, 80.0);
        }

        // Group 3: diagonal lines (10) — LineInstance capsule SDF
        let line_y = scene_top + 220.0;
        for i in 0..10 {
            let x0 = 20.0 + (i as f64) * 70.0;
            let x1 = x0 + 60.0;
            win.render.set_stroke_color(if i & 1 == 0 { "#d29922" } else { "#bc8cff" });
            win.render.set_stroke_width(2.0);
            win.render.begin_path();
            win.render.move_to(x0, line_y);
            win.render.line_to(x1, line_y + 50.0);
            win.render.stroke();
        }

        // Group 4: N-instance batch (small squares) — instance-buffer stress
        let batch_y = scene_top + 300.0;
        let cols = 40;
        let rows = 6;
        for ry in 0..rows {
            for cx in 0..cols {
                let x = 20.0 + (cx as f64) * 16.0;
                let y = batch_y + (ry as f64) * 16.0;
                let col = if (cx + ry) & 1 == 0 { "#161b22" } else { "#21262d" };
                win.render.set_fill_color(col);
                win.render.fill_rect(x, y, 14.0, 14.0);
            }
        }

        // Group 5: a longer text run — glyph atlas exercise
        lm::text(
            unsafe_widget_id("urx:label_long"),
            Rect { x: 20.0, y: batch_y + (rows as f64) * 16.0 + 16.0,
                   width: viewport.width as f64 - 40.0, height: 22.0 },
            "URX Phase 0 — InstancedWgpu wiring smoke. Identical scene across all 5 backends.",
        )
        .build(win.layout, win.render);
    }
}

// ── Mini agent HTTP server (zero deps — raw TCP) ────────────────────────────

fn parse_backend(name: &str) -> Option<RenderBackend> {
    match name {
        "vello_gpu"      => Some(RenderBackend::VelloGpu),
        "instanced_wgpu" => Some(RenderBackend::InstancedWgpu),
        "vello_hybrid"   => Some(RenderBackend::VelloHybrid),
        "vello_cpu"      => Some(RenderBackend::VelloCpu),
        "tiny_skia"      => Some(RenderBackend::TinySkia),
        _ => None,
    }
}

fn http_response(status: u16, body: &str) -> String {
    let reason = match status { 200 => "OK", 400 => "Bad Request", 404 => "Not Found", _ => "" };
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, reason, body.len(), body,
    )
}

fn handle_request(req: &str, shared: &Arc<Mutex<Shared>>) -> String {
    // Parse first line: e.g. "GET /urx/state HTTP/1.1"
    let first = req.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    match (method, path) {
        ("GET", "/health") => http_response(200, r#"{"ok":true,"service":"urx-smoke"}"#),
        ("GET", "/urx/state") => {
            let s = shared.lock().unwrap();
            let active = s.active.map(|b| b.label()).unwrap_or("<unknown>");
            let body = format!(
                r#"{{"backend":"{}","fps":{:.2},"frame_ms":{:.3},"frames":{},"available":["vello_gpu","instanced_wgpu","vello_hybrid","vello_cpu","tiny_skia"]}}"#,
                active, s.fps, s.frame_ms, s.frames,
            );
            http_response(200, &body)
        }
        ("POST", p) if p.starts_with("/urx/backend/") => {
            let name = &p["/urx/backend/".len()..];
            match parse_backend(name) {
                Some(b) => {
                    shared.lock().unwrap().requested_backend = Some(b);
                    http_response(200, &format!(r#"{{"queued":"{}"}}"#, b.label()))
                }
                None => http_response(400, &format!(r#"{{"error":"unknown backend: {}"}}"#, name)),
            }
        }
        _ => http_response(404, r#"{"error":"not found"}"#),
    }
}

fn start_http_server(port: u16, shared: Arc<Mutex<Shared>>) {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[urx-smoke] HTTP bind failed: {e}");
            return;
        }
    };
    eprintln!("[urx-smoke] HTTP listening on http://127.0.0.1:{port}/urx/state");
    thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { continue };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(1))).ok();
            let mut buf = [0u8; 2048];
            let n = match stream.read(&mut buf) { Ok(n) => n, Err(_) => continue };
            let req = std::str::from_utf8(&buf[..n]).unwrap_or("");
            let resp = handle_request(req, &shared);
            let _ = stream.write_all(resp.as_bytes());
        }
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shared: Arc<Mutex<Shared>> = Arc::new(Mutex::new(Shared::default()));
    start_http_server(17490, shared.clone());

    AppBuilder::new(SmokeApp { shared })
        // We DON'T set .backend(...) here — start with autodetect (VelloGpu
        // on a machine with wgpu). Users switch via the toolbar / HTTP POST.
        //
        // MSAA off — vello Renderer is built with area-only AA on this
        // host. Setting msaa(0) keeps every backend on the analytic-AA
        // path (vello area, instanced SDF, tiny_skia coverage) which is
        // also what we want for apples-to-apples comparison.
        .msaa(0)
        .window(
            WindowSpec::new(WindowKey::new("main"), "URX smoke — backend A/B rig")
                .size(880, 720)
                .decorations(false)
                .background(0xFF_0d_11_17)
                .corner_style(CornerStyle::Rounded)
                .border_color(0x00_FB_B2_6A),
        )
        .run()?;
    Ok(())
}
