# uzor-layout

Layout calculation helpers for UZOR (Level 2).

## Purpose

Provides helper functions for common layout patterns without enforcing visual style.
You calculate layout using these helpers, then render however you want.

## Usage

```rust
use uzor_layout::helpers;

// Center a button
let button_rect = helpers::center_rect(screen, 200.0, 50.0);

// Stack items vertically
let items = helpers::stack_vertical(sidebar, 40.0, 8.0, 10);

// Create grid
let cells = helpers::grid_layout(container, 3, 3, 10.0);
```

## When to Use

- You want quick layout calculation
- You want custom rendering
- You don't need ready-made widgets

For ready-made widgets with rendering, see `uzor-widgets`.
For maximum control, use `uzor-core` directly.
