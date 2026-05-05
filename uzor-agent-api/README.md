# uzor-agent-api

Local HTTP control plane for any uzor app. Sits next to the
window manager (`uzor-desktop`, future `uzor-web`,
`uzor-window-mobile`) as a **peer L4 helper**, not part of it.

External agents (LLMs, QA tooling, scripts, IDE plugins) read
live LM state and drive every UI operation LM understands —
**without screenshots when possible**, with screenshots as the
escape hatch when not.

The trait + types live in **`uzor::layout::agent::*`** (core).
This crate is the HTTP transport. Mobile / web window managers
implement the same `AgentControl` trait and reuse the routes.

---

## Enable

```rust
AppBuilder::new(MyApp::new())
    .agent_api(17480)              // local HTTP server on 127.0.0.1:17480
    .window(...)
    .run()?;
```

The server runs on its own tokio runtime in a dedicated thread.
Bind failure (port taken) is non-fatal — logged to stderr, app
keeps running.

---

## Routes

| Method | Path                         | Body / query                              | Returns                  |
|--------|------------------------------|-------------------------------------------|--------------------------|
| GET    | `/health`                    | —                                         | `{"ok":true}`            |
| GET    | `/state/tree`                | —                                         | `AgentSnapshot`          |
| GET    | `/state/widgets`             | —                                         | `Vec<WidgetSnapshot>`    |
| GET    | `/log?since=N&limit=M&prefix=X` | —                                      | `Vec<AgentLogEntry>`     |
| GET    | `/log/tail?n=N`              | —                                         | `Vec<AgentLogEntry>`     |
| GET    | `/screenshot/:window`        | —                                         | `image/png`              |
| POST   | `/cmd`                       | `Command` (tagged JSON)                   | `CommandReply`           |
| POST   | `/input/click`               | `{window, x, y, button?}`                 | `CommandReply`           |
| POST   | `/input/hover`               | `{window, x, y}`                          | `CommandReply`           |
| POST   | `/input/scroll`              | `{window, dx, dy}`                        | `CommandReply`           |
| POST   | `/lm/click_widget`           | `{window, widget_id}`                     | `CommandReply`           |
| POST   | `/lm/hover_widget`           | `{window, widget_id}`                     | `CommandReply`           |
| POST   | `/lm/modal/open`             | `{window, modal_id}`                      | `CommandReply`           |
| POST   | `/lm/modal/close`            | `{window, modal_id}`                      | `CommandReply`           |
| POST   | `/lm/popup/open`             | `{window, popup_id}`                      | `CommandReply`           |
| POST   | `/lm/popup/close`            | `{window, popup_id}`                      | `CommandReply`           |
| POST   | `/lm/dropdown/open`          | `{window, dropdown_id}`                   | `CommandReply`           |
| POST   | `/lm/dropdown/close`         | `{window, dropdown_id}`                   | `CommandReply`           |
| POST   | `/lm/sidebar/toggle`         | `{window, sidebar_id}`                    | `CommandReply`           |
| POST   | `/window/spawn`              | `{key, title, width, height, ...}`        | `CommandReply`           |
| POST   | `/window/close`              | `{key}`                                   | `CommandReply`           |
| POST   | `/lm/sync_mode`              | `{node_id, mode, group_id?}`              | `CommandReply`           |
| POST   | `/lm/style_preset`           | `{name}`                                  | `CommandReply`           |
| POST   | `/lm/panel/resize_edge`      | `{window, panel_id, edge, delta_px}`      | `CommandReply`           |
| POST   | `/lm/panel/drag_separator`   | `{window, sep_idx, delta_px}`             | `CommandReply`           |
| POST   | `/lm/panel/set_rect`         | `{window, panel_id, x, y, width, height}` | `CommandReply`           |
| GET    | `/blackboxes`                | —                                         | `Vec<String>`            |
| GET    | `/blackbox/:slot/widgets`    | —                                         | `Vec<AgentWidget>`       |
| GET    | `/blackbox/:slot/state`      | —                                         | free-form JSON           |
| POST   | `/blackbox/:slot/action`     | `{name, args?}` (`AgentAction`)           | `AgentActionReply`       |
| POST   | `/blackbox/:slot/click_widget` | `{sub_id, window?}`                     | `CommandReply`           |

All POSTs apply on the WM event-loop thread (winit) within one
tick of the request — replies are blocking.

---

## Two operating modes

### A. Semantic / direct LM ops *(default)*

Address things by stable id. No coordinates, robust to layout
changes.

```
POST /lm/click_widget       {window:"main", widget_id:"settings:theme_dark"}
POST /lm/modal/open         {window:"main", modal_id:"settings"}
POST /lm/sidebar/toggle     {window:"main", sidebar_id:"left"}
POST /lm/style_preset       {name:"mirage_dark"}
POST /window/spawn          {key:"side", title:"...", width:800, height:600}
POST /blackbox/chart-btc/action {name:"set_symbol", args:{symbol:"ETH"}}
```

### B. Pixel + screenshot *(escape hatch)*

For blackbox bodies (charts, drawing) where the widget tree
doesn't describe clickable regions.

```
POST /input/click       {window:"main", x:420, y:110}
GET  /screenshot/main   → image/png
```

Screenshot first call patches the vello surface with `COPY_SRC`
+ requests redraw and returns `503`; the next call returns PNG.

---

## Snapshot shape (`GET /state/tree`)

```json
{
  "root": {
    "current_window": "main",
    "window_count": 1,
    "style_preset": "mirage_dark"
  },
  "windows": [
    {
      "key": "main",
      "rect": {"x":0,"y":0,"w":1400,"h":900},
      "initialised": true,
      "chrome_visible": true,
      "edge_count": 1,
      "dock_leaves": 4,
      "overlay_count": 0,
      "modal_count": 0,
      "popup_count": 0,
      "dropdown_count": 0,
      "toolbar_count": 0,
      "sidebar_count": 0,
      "context_menu_count": 0,
      "hovered_widget": "settings:theme_dark",
      "pressed_widget": null,
      "last_click": null,
      "pointer_pos": [73.5, 127.0]
    }
  ],
  "sync_nodes": [
    {"node_id":"styles", "mode":"synced", "group_id":null},
    {"node_id":"branch.dock_tree", "mode":"sometimes_alone", "group_id":null},
    ...
  ],
  "frame_time_ms": 28589.5,
  "frame_count": 941,
  "fps_ema": 26.2
}
```

---

## Merged event log (`GET /log`)

Anyone holding `&mut LayoutManager` writes into one feed —
LM internals (`lm.*`), the app (`app.*`), and blackbox handlers
(`<slot_id>.*`).

```
seq 2  lm.click            settings:theme_light @ (206.5, 127)
seq 3  lm.dispatch         Unhandled(theme_light)
seq 4  lm.agent_command    ClickWidget ok=true
seq 5  lm.style.preset     mirage_light
seq 6  app.theme.changed   theme=light, preset=mirage_light
seq 7  tree-debug.toggle_synced_root  {show_synced_root:false}
```

Filter by category prefix:

```
GET /log?prefix=lm.style.
GET /log?prefix=app.
GET /log?prefix=tree-debug.
GET /log?since=42&limit=200
GET /log/tail?n=50
```

Categories agreed by convention (no enforcement):

- `lm.*` — `lm.click`, `lm.dispatch`, `lm.style.preset`,
  `lm.sync_mode`, `lm.window.attach`, `lm.window.detach`,
  `lm.overlay.modal`, `lm.overlay.popup`, `lm.overlay.dropdown`,
  `lm.overlay.sidebar`, `lm.agent_command`, `lm.blackbox.register`,
  `lm.blackbox.unregister`
- `app.*` — pushed by app code via `lm.agent_log_push(category,
  payload)` or `lm.agent_log_note(message)`
- `<blackbox-slot-id>.*` — pushed by the framework after a
  successful `/blackbox/<slot>/action` returns a `log_payload`

---

## Blackboxes

A blackbox is a panel whose body uses custom rendering /
coordinate-clicks (chart canvas, drawing tool, DOM ladder). LM
sees only its outer rect; everything inside is opaque.

To make it agent-controllable, the blackbox state struct
implements `BlackboxAgentSurface`:

```rust
use uzor::layout::agent::{
    AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface,
};

impl BlackboxAgentSurface for ChartHandler {
    fn agent_slot_id(&self) -> &str { &self.id }
    fn agent_kind(&self) -> &str { "chart" }

    fn list_agent_widgets(&self) -> Vec<AgentWidget> {
        vec![
            AgentWidget {
                sub_id: "btn:zoom_in".into(),
                kind: "button".into(),
                rect: self.zoom_in_rect,
                label: Some("Zoom In".into()),
                meta: serde_json::json!({}),
            },
            // …
        ]
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "symbol":    self.symbol,
            "crosshair": self.crosshair,
            "bars":      self.bar_count,
        })
    }

    fn apply_agent_action(&mut self, a: AgentAction) -> AgentActionReply {
        match a.name.as_str() {
            "set_symbol" => {
                self.symbol = a.args["symbol"].as_str().unwrap_or("").into();
                AgentActionReply::ok_with_log(serde_json::json!({
                    "symbol": self.symbol
                }))
            }
            _ => AgentActionReply::err("unknown action"),
        }
    }
}
```

Then register it from `App::init`:

```rust
let chart_state = Arc::new(Mutex::new(ChartHandler::new(...)));
layout.register_blackbox_agent("chart-btc", chart_state.clone());
```

From now on:

- `GET  /blackboxes`                          → `["chart-btc", ...]`
- `GET  /blackbox/chart-btc/widgets`          → published hot-zones
- `GET  /blackbox/chart-btc/state`            → `{symbol, crosshair, bars}`
- `POST /blackbox/chart-btc/action {name:"set_symbol", args:{symbol:"ETH"}}`
- `POST /blackbox/chart-btc/click_widget {sub_id:"btn:zoom_in"}`
- `GET  /log?prefix=chart-btc.`               → every action's `log_payload`

LM sees nothing chart-specific. Each blackbox is a
self-contained sandbox; an agent can drive several
simultaneously without them stepping on each other.

`AgentActionReply::ok_with_log(payload)` makes the framework
push a `<slot>.<action_name>` entry into `/log`. Use
`AgentActionReply::ok()` if you don't want a log entry.

---

## Layering

```
uzor (core)
└── layout::agent
    ├── AgentControl trait   ← what every WM implements
    ├── Command / Reply      ← write vocabulary
    ├── Snapshot / Widget    ← read shape
    ├── AgentLog (ring buf)  ← merged feed
    ├── BlackboxAgentSurface ← per-blackbox sandbox contract
    └── LmAgent<P>           ← default implementation of every op
                                routable through `LayoutManager`

uzor-agent-api (this crate)
└── axum + tokio HTTP shim over `Arc<dyn AgentControl>`

uzor-desktop::Manager
└── implements AgentControl via:
    ├── Arc<RwLock<AgentSnapshot>>     rebuilt every tick
    ├── Arc<RwLock<Vec<WidgetSnapshot>>>
    ├── Arc<RwLock<Vec<AgentLogEntry>>>
    ├── Arc<RwLock<HashMap<slot, Arc<Mutex<dyn ...>>>>>
    ├── std::sync::mpsc<(Command, Sender<Reply>)>   drained in about_to_wait
    └── std::sync::mpsc<ScreenshotRequest>          drained in about_to_wait
```

---

## Threading

- HTTP server lives on its own tokio multi-thread runtime.
- `dispatch(Command)` is sync — server handlers run it on
  `spawn_blocking` so a blocked WM tick doesn't pin a tokio
  worker.
- Commands apply on the **winit** thread inside `about_to_wait`
  before the next solve, so the next `GET /state/tree` already
  reflects the change.
- Blackbox locks are short — a single `Mutex` per blackbox, taken
  by HTTP handlers for the duration of `widgets()` /
  `state()` / `apply_agent_action()`.

---

## Recipe: agent-driven smoke test

```sh
# 1. Discover what's on screen
curl -s :17480/state/tree     | jq .root.current_window
curl -s :17480/state/widgets  | jq '.[] | {id, kind, rect}'

# 2. Find the theme buttons
curl -s :17480/state/widgets  | jq '.[] | select(.id | startswith("settings:theme"))'

# 3. Switch theme by id (no pixel coords)
curl -s -X POST :17480/lm/click_widget \
  -H 'Content-Type: application/json' \
  -d '{"window":"main","widget_id":"settings:theme_light"}'

# 4. Replay the cause-and-effect
curl -s ':17480/log/tail?n=20' | jq '.[] | "\(.seq) \(.category) \(.payload)"'

# 5. Drive a blackbox without knowing its internals
curl -s :17480/blackboxes
curl -s :17480/blackbox/tree-debug/widgets
curl -s -X POST :17480/blackbox/tree-debug/action \
  -H 'Content-Type: application/json' \
  -d '{"name":"toggle_synced_root"}'
curl -s :17480/blackbox/tree-debug/state
```

---

## Status

**First-pass.** API is stable enough for internal agents but
will grow.  Planned:

- `POST /lm/dock/split`, `POST /lm/edge/add` — direct dock/edge
  manipulation (currently only via app layer).
- `POST /input/key` / `/input/text` — keyboard injection.
- `POST /lm/sync_mode` write semantics — currently flips the
  classification tag; actual storage relocation between
  shared/per-window is a future pass.
- WebSocket subscription on `/log` for push-mode instead of
  polling.
- Authentication (bearer token) before binding to non-loopback.

---

## Why a peer L4 helper, not a window-manager feature

WMs (`uzor-desktop`, `uzor-window-web`, `uzor-window-mobile`)
host the LM and own the event loop. The agent API is
strictly *one more consumer* of LM — it implements
`AgentControl` against the same LM the WM owns. The same
trait will work on iOS/Android (HTTP listening on
`127.0.0.1`) and in browsers (extension talking to
`AgentControl` exposed on the JS side).

LM is the boss. WMs and the agent API are tools that read /
write LM by contract.
