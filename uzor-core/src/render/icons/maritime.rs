//! Maritime icons — silhouettes for ship tracking
//!
//! Note: Most sources provide side-view ship icons. True top-down vessel icons
//! are rare in open-source libraries. These are from Lucide/Tabler (side-view).

/// Cargo ship (side-view)
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Side-view cargo/container ship
pub const CARGO: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 10.189V14" /><path d="M12 2v3" /><path d="M19 13V7a2 2 0 0 0-2-2H7a2 2 0 0 0-2 2v6" /><path d="M19.38 20A11.6 11.6 0 0 0 21 14l-8.188-3.639a2 2 0 0 0-1.624 0L3 14a11.6 11.6 0 0 0 2.81 7.76" /><path d="M2 21c.6.5 1.2 1 2.5 1 2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1s1.2 1 2.5 1c2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1" /></svg>"#;

/// Sailboat (side-view, use with CHEVRON for direction)
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Side-view sailboat with mast. Render smaller (~14px) with
/// a CHEVRON indicator nearby for heading direction.
pub const SAILBOAT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 2v15" /><path d="M7 22a4 4 0 0 1-4-4 1 1 0 0 1 1-1h16a1 1 0 0 1 1 1 4 4 0 0 1-4 4z" /><path d="M9.159 2.46a1 1 0 0 1 1.521-.193l9.977 8.98A1 1 0 0 1 20 13H4a1 1 0 0 1-.824-1.567z" /></svg>"#;

/// Generic boat (top-down view)
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Top-down wide boat — fully filled hull, pointed bow, wide stern
/// Pointing UP (north) — rotation via draw_svg_icon_rotated
pub const BOAT_GENERIC: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M12 1 L8 5 L7 8 L6.5 12 L6.5 18 L7 20 L8.5 22 L15.5 22 L17 20 L17.5 18 L17.5 12 L17 8 L16 5 Z"/></svg>"#;

/// Submarine (side-view)
/// Source: Tabler Icons
/// License: MIT
///
/// Design: Side-view submarine with periscope and conning tower
pub const SUBMARINE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 11v6h2l1 -1.5l3 1.5h10a3 3 0 0 0 0 -6h-10l-3 1.5l-1 -1.5h-2" /><path d="M17 11l-1 -3h-5l-1 3" /><path d="M13 8v-2a1 1 0 0 1 1 -1h1" /></svg>"#;

/// Bulk carrier / cargo ship (top-down view)
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Top-down view of a bulk carrier with tapered bow, wide hull, cargo holds
/// Pointing UP (north) — rotation via draw_svg_icon_rotated
pub const BULK_CARRIER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M12 1 L9 4 L8 4 L7.5 6 L7 10 L7 18 L7.5 20 L8.5 22 L10 23 L14 23 L15.5 22 L16.5 20 L17 18 L17 10 L16.5 6 L16 4 L15 4 Z M9.5 7 L14.5 7 L14.5 10 L9.5 10 Z M9.5 11.5 L14.5 11.5 L14.5 14.5 L9.5 14.5 Z M9.5 16 L14.5 16 L14.5 19 L9.5 19 Z"/></svg>"#;

/// Oil tanker (top-down view)
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Top-down tanker — wider hull, rounded bow, pipe runs along deck
/// Pointing UP (north) — rotation via draw_svg_icon_rotated
pub const TANKER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M12 1 L9.5 3 L8 5 L7 8 L6.5 12 L6.5 18 L7 20 L8.5 22 L10.5 23 L13.5 23 L15.5 22 L17 20 L17.5 18 L17.5 12 L17 8 L16 5 L14.5 3 Z M11.5 4 L12.5 4 L12.5 19 L11.5 19 Z M8.5 9 a3 3 0 0 1 3 -3 L12 6 L12 12 L8.5 12 Z M12 6 L15.5 9 a3 3 0 0 1 -3 3 L12 12 L12 6 Z M8.5 14 L11 14 L11 18 L8.5 18 Z M13 14 L15.5 14 L15.5 18 L13 18 Z"/></svg>"#;

/// Direction chevron (small arrow indicator)
/// Source: Custom
/// License: Public domain
///
/// Design: Simple filled chevron/arrow pointing UP, used as heading indicator
/// next to side-view vessel icons
pub const CHEVRON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M12 4 L18 16 L12 12 L6 16 Z"/></svg>"#;

/// Anchor (maritime symbol)
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Traditional ship anchor
pub const ANCHOR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 6v16" /><path d="m19 13 2-1a9 9 0 0 1-18 0l2 1" /><path d="M9 11h6" /><circle cx="12" cy="4" r="2" /></svg>"#;

/// Ferry / passenger liner (side-view, use with CHEVRON for direction)
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Side-view passenger ship — hull, deck superstructure, funnel
/// Stroke-based outline. Use with CHEVRON for heading direction.
pub const FERRY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 17 L4 14 L20 14 L22 17 L21 18 L3 18 Z"/><rect x="6" y="10" width="12" height="4" rx="0.5"/><rect x="10" y="6" width="3" height="4"/></svg>"#;
