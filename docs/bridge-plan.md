# Bridge: from current uzor to Mirage-style `view!` DSL

Pair this with:
- `docs/mirage-target.md` — what we copy (JSX style, token discipline, flat trees)
- `docs/uzor-current.md` — what we have (L3 LM API, builders, flex engine)

Goal: a proc-macro `view!` in a new crate `uzor-framework-macros`, re-exported by `uzor-framework`, that lets the agent/user write JSX-mimicking trees instead of imperative `lm::*` chains. Native L3 stays untouched.

---

## Mirage → uzor mapping

| Mirage (JSX) | uzor expansion |
|---|---|
| `<KPI label="rps" value=12 unit="k"/>` | call user-defined fn `KPI(label, value, unit)` returning a `view!` block |
| `<div style={{display:'flex', gap:8}}>` | `<row gap=8>` (or `<col>`) — built-in primitives that emit `LayoutNode` with `Display::Flex` |
| `className="btn btn--primary"` | `class={[Btn, BtnPrimary]}` — Rust enum constants resolved to `LayoutStyle` |
| `var(--accent)` | `tokens::accent` — typed Rust constant in a generated module from a `tokens.toml` |
| `{items.map((x,i) => <Row .../>)}` | `for x in items { Row(x) }` inside `view!` |
| `{cond && <X/>}` | `if cond { X {} }` |
| `<button onClick={() => save()}>Save</button>` | `<button on_click={\|\| self.save()}>Save</button>` — closure forwarded to `lm::button(...).on_click(...)` |
| `<modal title="Settings" open>...</modal>` | `<modal title="Settings" open={true}>...</modal>` — children become body block, parent auto-wired |
| `useState(0)` | `state!(counter: i32 = 0)` — generates a field on the App struct |

---

## What the macro generates (lowering)

`view! { <modal handle=settings title="Settings"> <button id=save text="Save" on_click={||self.save()}/> </modal> }`

→

```rust
{
    lm::modal(&self.settings).title("Settings").build(&mut layout, &mut render);
    let __body = layout.modal_body_rect(&self.settings).unwrap_or_default();
    // children laid out inside __body via flex compute
    let __ids = ["save"];
    let __rects = uzor_framework::layout::row(__body, &__ids, /*gap*/ 0.0);
    lm::button("save", __rects[0]).text("Save").on_click(|| self.save()).build(&mut layout, &mut render);
}
```

Auto-IDs: macro derives `path::file::line:col` then hashes to short stable `String`. User-supplied `id=` overrides.

Auto-handles: composite tags (`<modal>`, `<popup>`, `<dropdown>`, `<toolbar>`, `<sidebar>`) with no `handle=` prop generate a hidden field on the App struct via a paired `#[uzor::app]` attr macro that walks the `view!` once at compile time and emits `add_modal/add_popup/...` calls into a generated `init()` impl.

---

## Crate layout (additions)

```
uzor-framework-macros/         # new proc-macro crate
  src/
    lib.rs                    # entry: #[proc_macro] view!, #[proc_macro_attribute] uzor_app
    parse.rs                  # syn-based JSX-ish parser
    lower.rs                  # AST → TokenStream emitting lm::* calls
    ids.rs                    # stable ID derivation
    tags.rs                   # registry: <modal> / <popup> / <button> / <row> / <col> / <text> / <checkbox> / <toggle> / <kpi> / <chrome> ...

uzor-framework/
  src/
    layout/
      mod.rs                  # row()/col()/grid() helpers — solve flex for [child_id]→[Rect]
      tokens.rs               # palette + spacing + radii constants (mirrors Mirage colors_and_type.css)
    macros.rs                 # re-export view!, uzor_app
```

Tokens crate is generated from `uzor-framework/tokens.toml` at build time (build.rs) so designers can edit one file.

---

## Tag set (v1)

Built-in:
- Layout: `<row>` `<col>` `<grid>` `<stack>` `<spacer>`
- Composite: `<modal>` `<popup>` `<dropdown>` `<context_menu>` `<toolbar>` `<sidebar>` `<chrome>` `<panel>` `<blackbox>`
- Atomic: `<button>` `<text>` `<checkbox>` `<toggle>` `<separator>` `<icon>`
- Control flow: `if cond { ... }`, `for x in xs { ... }` — Rust syntax, not JSX-y

User-defined: any `fn Foo(props) -> View { view!{...} }` — call as `<Foo prop=val/>`.

---

## Migration path (no breakage)

1. **Phase B1** — add `uzor-framework-macros` crate, expose `view!` covering only `<row>` `<col>` `<button>` `<text>` `<checkbox>`. Ship one example: `level4_dashboard.rs` rewritten as a single `view!` tree using the existing `App::ui` slot.
2. **Phase B2** — add composite tags (`<modal>` `<popup>` `<dropdown>` `<toolbar>` `<sidebar>` `<chrome>`). Add `#[uzor_app]` attr macro that scans `view!` calls and synthesises handle fields + `init()`.
3. **Phase B3** — add `tokens.toml` + `build.rs` codegen. Replace ad-hoc colors in atomic builders with token references.
4. **Phase B4** — add user-defined component support (`<Foo/>` calling `fn Foo(...) -> View`). Until then, components must be inlined.
5. **Phase B5** — replace `level3_dashboard.rs` content equivalent inside `level4_dashboard.rs` so we have a 1:1 polygon comparison (raw L3 vs `view!`-driven L4) for sanity-checking parity.

L3 stays untouched throughout — `view!` only emits calls to existing `lm::*` builders.

---

## Open questions to settle before B1

1. **Children flex defaults.** `<row>` defaults to `Display::Flex` + `FlexDirection::Row` + `gap=0`. `<col>` flips direction. Padding default = 0. Confirmed?
2. **Auto-ID strategy.** `file_line_col` hashed to 8 chars vs `parent_path::tag[index]`. Latter is debuggable but verbose. Pick: `parent_path::tag[index]` for now.
3. **Closures and `self`.** `on_click={|| self.save()}` requires the closure to outlive the frame. Today `lm::button(...).on_click(...)` takes `FnMut() + 'a`. Confirm `view!` expansion borrows `&mut self` for the whole macro block — fine because the macro emits one statement-block tied to `App::ui(&mut self, ...)`.
4. **Overlap with existing chainable builders.** `view!` should NOT replace `lm::*`. It generates calls into them. Apps that need escape-hatch behaviour mix raw `lm::*` calls and `view!` blocks freely.
5. **Compile-time vs runtime token checking.** Tokens-as-Rust-consts means typo = compile error. Good. But CSS `var(--accent)` lookups in inline objects don't typo-check — we get a strict-mode improvement here for free.

---

## Success criteria

The L4 dashboard polygon, written in `view!`, must:
- Be ≤ 60% of the line count of the equivalent imperative L3 polygon.
- Contain zero `WidgetId::new` / `unsafe_widget_id` calls.
- Contain zero literal `Rect::new(...)` calls outside top-level chrome configuration.
- Compile-error on mistyped tokens, mistyped tags, and missing required props.
- Have a structure that an LLM trained on JSX can extend without reading the macro source.
