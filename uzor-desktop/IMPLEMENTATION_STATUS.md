# uzor-desktop Factory Implementation Status

## Overview

This document tracks the implementation of factory rendering functions for uzor-desktop.

## Completed

1. **Factory Module Structure** (`src/factory/mod.rs`)
   - Defined `DesktopRenderContext` trait with minimal rendering API
   - Created module scaffolding for all 9 widget types
   - Re-exports for convenient factory function access

2. **Widget Factory Files Created**
   - `button.rs` - Button rendering (adapted from terminal reference)
   - `container.rs` - Container with scrollbar rendering
   - `popup.rs` - Context menus and color pickers
   - `panel.rs` - Toolbars, sidebars, modals
   - `overlay.rs` - Tooltips and info overlays
   - `text_input.rs` - Text/number/search/password inputs
   - `dropdown.rs` - Standard/grid/layout dropdowns
   - `slider.rs` - Single and dual sliders
   - `toast.rs` - Info/success/warning/error toasts

## Architecture Mismatch Discovered

During implementation, a significant architectural difference was discovered between the spec (FACTORY_RPD.md) and the actual uzor-core migration:

### Expected (from spec)
```rust
// Widget types without geometry
enum ButtonType {
    Action { variant, disabled },
    // ...
}

// Theme returns hex strings
trait ButtonTheme {
    fn button_bg_normal() -> &str; // Returns "#1E1E1E"
}
```

### Actual (uzor-core reality)
```rust
// Widget types INCLUDE geometry (L1 standard signature)
enum ButtonType {
    Action {
        variant,
        disabled,
        position: (f64, f64),  // NEW
        width: f64,             // NEW
        height: f64,            // NEW
    },
}

// Theme returns RGBA byte arrays
trait ButtonTheme {
    fn button_bg_normal() -> [u8; 4]; // Returns [30, 30, 30, 255]
}
```

### Impact

**156 compilation errors** due to:
1. Pattern matching doesn't account for position/width/height fields in every variant
2. Theme method signatures expect RGBA arrays, not hex strings
3. Additional widget states (Active, Toggled) not in original spec
4. Theme method names differ from spec (e.g., `scrollbar_track_color()` vs `container_scrollbar_track()`)

## Next Steps

### Option 1: Complete Adaptation (Recommended)
Fully adapt all 9 widget factories to match uzor-core's actual architecture:

1. **Update pattern matching** in all factories to handle position/width/height
2. **Convert theme usage** from hex strings to RGBA byte arrays
3. **Add missing state handling** for Active and Toggled states
4. **Fix theme method names** to match actual traits
5. **Update DesktopRenderContext** to accept RGBA instead of hex strings

Estimated effort: 4-6 hours

### Option 2: Define Bridge Adapter
Create an adapter layer that converts between architectures:

```rust
pub struct ThemeAdapter<'a> {
    theme: &'a dyn ButtonTheme,
}

impl ThemeAdapter<'_> {
    fn button_bg_normal(&self) -> String {
        let rgba = self.theme.button_bg_normal();
        format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2])
    }
}
```

Estimated effort: 2-3 hours

### Option 3: Update RPD Spec
Update FACTORY_RPD.md to reflect actual uzor-core architecture, then implement:

1. Document L1 standard signature (geometry in variants)
2. Document RGBA theme contract
3. Rewrite factory examples with correct patterns
4. Implement factories following updated spec

Estimated effort: 6-8 hours

## Recommendation

**Option 1 is recommended** because:
- uzor-core architecture is already established and production-tested
- Other parts of the codebase already use this pattern successfully
- Direct implementation is cleaner than maintaining adapter layers
- Avoids confusion from outdated specs

## Files Requiring Updates

All factory files need updates:
- [ ] `src/factory/button.rs` (62 errors)
- [ ] `src/factory/container.rs` (12 errors)
- [ ] `src/factory/popup.rs` (18 errors)
- [ ] `src/factory/panel.rs` (14 errors)
- [ ] `src/factory/overlay.rs` (8 errors)
- [ ] `src/factory/text_input.rs` (10 errors)
- [ ] `src/factory/dropdown.rs` (16 errors)
- [ ] `src/factory/slider.rs` (8 errors)
- [ ] `src/factory/toast.rs` (8 errors)

Plus:
- [ ] `src/factory/mod.rs` - Update DesktopRenderContext to use RGBA

## Current Compilation Status

```bash
cd zengeld-terminal/crates/uzor/uzor-desktop
cargo check
```

Result: **156 errors** due to architecture mismatch

## Example Fix Pattern

### Before (current implementation)
```rust
ContainerType::Plain { border } => {
    ctx.set_fill_color(theme.container_bg());  // Expects hex string
    // ...
}
```

### After (correct implementation)
```rust
ContainerType::Plain { position, width, height } => {
    let rgba = theme.container_bg_color();
    let hex = format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2]);
    ctx.set_fill_color(&hex);
    // OR update DesktopRenderContext to accept RGBA directly
    // ...
}
```

## Lessons Learned

1. **Always read actual type definitions** before implementing against specs
2. **Run early compilation checks** to catch architecture mismatches sooner
3. **Specs can become outdated** - verify against current codebase
4. **Migration is complex** - the uzor-core migration changed more than expected

## Contact

For questions about continuing this implementation, consult:
- `zengeld-terminal/crates/uzor/FACTORY_RPD.md` - Original spec (needs update)
- `zengeld-terminal/crates/uzor/uzor-core/src/widgets/*/types.rs` - Actual types
- `zengeld-terminal/crates/uzor/uzor-core/src/widgets/*/theme.rs` - Actual themes
- `zengeld-terminal/crates/core/src/ui/widgets/button/factory/render.rs` - Reference impl (may be outdated)
