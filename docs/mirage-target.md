# Mirage Design System — Target Spec for `view!` macro

Source: `C:\Users\VA PC\AppData\Local\Temp\mirage-ds\`

---

## Stack

| Layer | Choice |
|-------|--------|
| Language | JSX (React 18) |
| Runtime | Browser — no build step |
| Transpiler | **Babel Standalone** (`@babel/standalone@7.29.0` via unpkg CDN) |
| Framework | **React 18** via unpkg UMD (no npm, no bundler) |
| CSS | **Vanilla CSS** — one global token file + one per-kit CSS file |
| Fonts | Self-hosted `.ttf` via `@font-face` |
| Icons | Inline SVG — `currentColor`, `stroke-width: 1.8`, no icon lib |
| Build tooling | **None.** `<script type="text/babel" src="Foo.jsx">` — browser compiles on load |

Bootstrapping pattern (from `ui_kits/landing/index.html`):
```html
<script src="react.development.js"></script>
<script src="react-dom.development.js"></script>
<script src="babel.min.js"></script>
<script type="text/babel" src="Atoms.jsx"></script>   <!-- defines window.Logo etc -->
<script type="text/babel" src="Hero.jsx"></script>
<script type="text/babel">
  ReactDOM.createRoot(document.getElementById('root')).render(<App />);
</script>
```

Cross-file sharing: each JSX file ends with `Object.assign(window, { Foo, Bar })`.
No module system. No imports. Global namespace only.

---

## Component Patterns

- **Function components** exclusively. No class components, no HOCs.
- Props destructured inline: `function KPI({ label, value, unit }) { ... }`
- Default prop values in destructuring: `const Logo = ({ size = 20, color = 'var(--accent)' }) => ...`
- Hooks used: `React.useState`, `React.useEffect`, `React.useMemo` — all via `React.` prefix (no destructure imports, because UMD globals).
- Custom hooks: simple, named with `use` prefix, return primitive values (`function useTickingLog() { return i; }`).
- No context, no reducers, no external state libs. State lives in the top-level screen component (`AdminApp`, `ClientApp`) and flows down as props.

---

## Composition Style

- Children passed as JSX children: `<Button variant="outline" sm>Войти</Button>`
- Conditional render via short-circuit and ternary: `{isOff && <BigButton/>}`, `{!isOff && <MeshWheel/>}`
- Fragments via `<>...</>` for conditional multi-element blocks.
- Lists via `.map((item, i) => <div key={i}>...</div>)` — key is always the index (no stable IDs).
- Local helper components defined in the same file as their consumer (e.g. `Terminal` inside `Hero.jsx`, `KPIStrip` inside `AdminApp.jsx`).
- No slot/render-prop pattern. Composition is purely positional children or explicit data props.

---

## Styling

**Two-tier system:**

1. **CSS variables** (`colors_and_type.css`) — all tokens: palette, type scale, spacing, radii, shadows. Applied as `var(--accent)`, `var(--font-mono)`, etc.
2. **CSS classes** (`kit.css`) — structural components: `.btn`, `.btn--primary`, `.hdr`, `.container`, `.eyebrow`, `.kpi`, `.layer-chip`, `.status-pill`.

**Inline styles** are used heavily in `app/` components (AdminApp, ClientApp) where the entire component tree is styled inline — no kit.css dependency. Tokens are copied into a local `const S = { bg: '#08090B', ... }` object at the top of the file.

Pattern:
```jsx
// Atom components: CSS class
<button className={`btn btn--${variant}${sm ? ' btn--sm' : ''}`}>

// App components: inline style object
<div style={{ display:'flex', gap:8, fontFamily:A.mono, fontSize:11, color:A.fg2 }}>
```

No Tailwind. No CSS Modules. No styled-components. No CSS-in-JS lib.

---

## Layout Primitives

Everything is `display: flex` or `display: grid`. No layout abstraction beyond that.

```jsx
// Column stack
<div style={{ display:'flex', flexDirection:'column', gap:9 }}>

// Row with space-between
<div style={{ display:'flex', justifyContent:'space-between', alignItems:'baseline' }}>

// Fixed grid columns (node list row)
<div style={{ display:'grid', gridTemplateColumns:'6px auto 1fr auto auto', gap:10, alignItems:'center' }}>

// Container with max-width
<div className="container">   // max-width: 1280px, margin: 0 auto, padding: 0 24px
```

No abstract `Stack`, `Row`, `Col`, `Grid` components. Raw HTML elements + className/style.

---

## Naming Conventions

- Component names: PascalCase, noun-first (`KPIStrip`, `NodeList`, `SectionHead`, `BigButton`, `MeshWheel`).
- Files: PascalCase matching primary export (`AdminApp.jsx`, `Atoms.jsx`, `Hero.jsx`).
- CSS classes: BEM-lite — block `hdr`, element `hdr__inner`, modifier `btn--primary`.
- Token vars: kebab with semantic prefix — `--fg-1`, `--surface-raised`, `--accent-dim`, `--shadow-glow`.
- Local style constants: single-letter alias `A = ADMIN_S`, `C = CLIENT_S` — used inline as `A.mono`, `C.accent`.
- Helper components: same file as consumer, no dedicated file unless shared across sections.

---

## What Makes It Claude-Friendly

1. **No magic.** Every component is a function that returns JSX. No decorators, no annotations, no codegen.
2. **Self-contained files.** Each `.jsx` is independently readable — imports nothing, exports via `window`.
3. **Inline data.** Sample data lives as `const` arrays inside the component file (`A_NODES`, terminal `rows`). No fetch, no store, no API contract needed to render.
4. **Predictable prop shapes.** Props are either primitives (`string | number | bool`) or simple objects. No polymorphic `as` prop, no `forwardRef`.
5. **Flat hierarchy.** Most components are 2–3 levels deep. No deeply nested provider trees.
6. **Token discipline.** All values come from a finite set of named tokens. An LLM can pick `var(--accent)` / `var(--fg-2)` / `var(--font-mono)` without guessing hex codes.
7. **Conditional via ternary/short-circuit only.** No complex render logic, no switch-case in JSX.
8. **Animations are rare and explicit.** `setInterval` for ticking, `requestAnimationFrame` for smooth animation — both in `useEffect`, both cancel-on-cleanup. No CSS animation lib.

---

## Canonical Example to Mimic

`KPI` from `Atoms.jsx` — simplest complete atom, shows token usage + mono stat pattern:

```jsx
const KPI = ({ label, value, unit }) => (
  <div className="kpi">
    <div className="kpi__label">{label}</div>
    <div className="kpi__value">
      {value}
      {unit && <span className="unit"> {unit}</span>}
    </div>
  </div>
);
```

With CSS:
```css
.kpi__label {
  font-family: var(--font-mono);
  font-size: 9px;
  color: var(--fg-3);
  letter-spacing: 0.12em;
  text-transform: uppercase;
  margin-bottom: 5px;
}
.kpi__value {
  font-family: var(--font-mono);
  font-weight: 700;
  font-size: 18px;
  color: var(--fg-0);
  font-variant-numeric: tabular-nums;
  line-height: 1;
}
```

`NodeList` from `AdminApp.jsx` — canonical data row pattern using grid layout + conditional color:

```jsx
function NodeList() {
  return (
    <div>
      {A_NODES.map((n, i) => {
        const dot = n.s==='err' ? A.err : n.s==='warn' ? A.warn : A.ok;
        const lc  = n.l==='L1' ? A.l1 : A.accent;
        return (
          <div key={i} style={{
            display: 'grid',
            gridTemplateColumns: '6px auto 1fr auto auto',
            gap: 10, alignItems: 'center',
            padding: '9px 22px',
            borderBottom: `1px solid ${A.border}`,
            fontFamily: A.mono, fontSize: 11,
          }}>
            <span style={{ width:6, height:6, borderRadius:99, background:dot,
              boxShadow: n.s==='ok' ? `0 0 6px ${dot}` : 'none' }}/>
            <span style={{ fontSize:9, padding:'1px 5px', color:lc,
              background: n.l==='L1' ? 'rgba(109,184,245,0.12)' : 'rgba(251,178,106,0.12)',
              border: `1px solid ${n.l==='L1' ? 'rgba(109,184,245,0.3)' : 'rgba(251,178,106,0.3)'}`,
              letterSpacing:'0.08em' }}>{n.l}</span>
            <span style={{ color:A.fg0, overflow:'hidden', textOverflow:'ellipsis',
              whiteSpace:'nowrap' }}>{n.n}</span>
            <span style={{ color:A.fg2, fontVariantNumeric:'tabular-nums' }}>{n.up}</span>
            <span style={{ color:A.fg1, fontVariantNumeric:'tabular-nums',
              minWidth:36, textAlign:'right' }}>
              {n.s==='err'
                ? <span style={{ color:A.err }}>—</span>
                : <>{n.rtt}<span style={{ color:A.fg3 }}>ms</span></>}
            </span>
          </div>
        );
      })}
    </div>
  );
}
```

---

## Implications for `view!` Macro DSL

| Mirage pattern | `view!` macro target |
|----------------|---------------------|
| `className="btn btn--primary"` | `class: "btn btn--primary"` or `cls: [Btn, BtnPrimary]` |
| `style={{ display:'flex', gap:8 }}` | `style: { display: flex, gap: 8 }` |
| `var(--accent)` | `color: accent` (resolve to token at macro-expand time) |
| Conditional `{isOff && <X/>}` | `if state == Off { X {} }` |
| `{items.map((x,i) => <Row key={i} .../>)}` | `for item in items { Row { ... } }` |
| Local const data arrays | static `const` blocks outside `view!`, referenced by name |
| `Object.assign(window, { Foo })` | `pub` visibility on the widget struct |
| `React.useState` | `state!` or owned field on widget struct |
| `React.useEffect` — interval | `on_mount` + `spawn` timer |
| `React.useMemo` — computed | `derived!` or plain `let` in render fn |

Token file `colors_and_type.css` maps 1:1 to what a Rust `theme.rs` or `tokens.rs` should expose as constants — same names, same semantic grouping (surface ladder, accent, l1-blue, status, borders, radii, type scale, spacing).
