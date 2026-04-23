//! SVG icon paths for macOS UI elements
//!
//! All paths are designed to be rendered at their specified viewBox dimensions
//! and can be scaled as needed. Use with `crate::render::draw_svg_icon`.

/// Checkmark icon for checkboxes and menu items
/// viewBox: 0 0 100 100
/// Recommended size: 18x18 for menu items, 19x19 for checkboxes
pub const CHECKMARK: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 38 74 L 22 55 C 20 52 20 48 22 46 C 25 44 28 44 31 47 L 44 63 L 68 26 C 71 23 75 21 78 23 C 80 25 81 29 78 33 L 51 74 C 50 76 46 78 45 78 C 44 78 42 78 41 77 C 40 76 39 75 38 74 Z" fill="currentColor"/></svg>"#;

/// Mixed/indeterminate state icon for checkboxes (horizontal line)
/// viewBox: 0 0 100 100
/// Recommended size: 18x18 for menu items, 19x19 for checkboxes
pub const CHECKMARK_MIXED: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 87 42.6 L 13 42.6 L 13 57.4 L 87 57.4 L 87 42.6 Z" fill="currentColor"/></svg>"#;

/// Chevron right icon for submenu indicators
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const CHEVRON_RIGHT: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 34 29 C 31 26 30 21 32 19 C 35 17 39 17 42 20 L 66 44 C 66 44 67 45 67 45 C 67 45 68 46 68 46 C 69 47 69 48 69 50 C 69 52 69 53 68 54 C 68 54 67 55 67 55 C 67 55 66 56 66 56 L 42 80 C 39 83 35 83 32 81 C 30 79 31 74 34 71 L 55 50 Z" fill="currentColor"/></svg>"#;

/// Chevron left icon
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const CHEVRON_LEFT: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 65.96 29 C 68.96 26 69.96 21 67.96 19 C 64.96 17 60.96 17 57.96 20 L 33.96 44 C 33.96 44 32.96 45 32.96 45 C 32.96 45 31.96 46 31.96 46 C 30.96 47 30.96 48 30.96 50 C 30.96 52 30.96 53 31.96 54 C 31.96 54 32.96 55 32.96 55 C 32.96 55 33.96 56 33.96 56 L 57.96 80 C 60.96 83 64.96 83 67.96 81 C 69.96 79 68.96 74 65.96 71 L 44.96 50 L 65.96 29 Z" fill="currentColor"/></svg>"#;

/// Chevron down icon for dropdowns
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const CHEVRON_DOWN: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 13 38.99 L 46 71.99 C 46 71.99 48 73.99 50 73.99 C 52 73.99 54 71.99 54 71.99 L 87 38.99 C 87 38.99 92 32.99 87 27.99 C 82 23.99 77 28.99 77 28.99 L 50 53.99 L 23 28.99 C 23 28.99 18 22.99 13 27.99 C 8 32.99 13 38.99 13 38.99 Z" fill="currentColor"/></svg>"#;

/// Chevron up icon
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const CHEVRON_UP: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 13 61 L 46 28 C 46 28 48 26 50 26 C 52 26 54 28 54 28 L 87 61 C 87 61 92 67 87 72 C 82 76 77 71 77 71 L 50 46 L 23 71 C 23 71 18 77 13 72 C 8 67 13 61 13 61 Z" fill="currentColor"/></svg>"#;

/// Arrow up icon for sort indicators
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const ARROW_UP: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 13 61 L 46 28 C 46 28 48 26 50 26 C 52 26 54 28 54 28 L 87 61 C 87 61 92 67 87 72 C 82 76 77 71 77 71 L 50 46 L 23 71 C 23 71 18 77 13 72 C 8 67 13 61 13 61 Z" fill="currentColor"/></svg>"#;

/// Arrow down icon for sort indicators
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const ARROW_DOWN: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 13 38.99 L 46 71.99 C 46 71.99 48 73.99 50 73.99 C 52 73.99 54 71.99 54 71.99 L 87 38.99 C 87 38.99 92 32.99 87 27.99 C 82 23.99 77 28.99 77 28.99 L 50 53.99 L 23 28.99 C 23 28.99 18 22.99 13 27.99 C 8 32.99 13 38.99 13 38.99 Z" fill="currentColor"/></svg>"#;

/// Close (X) icon for traffic light close button
/// viewBox: 0 0 16 18
/// Recommended size: 6x6
pub const TRAFFIC_LIGHT_CLOSE: &str = r#"<svg viewBox="0 0 16 18" fill="none"><path d="M15.7522 4.44381L11.1543 9.04165L15.7494 13.6368C16.0898 13.9771 16.078 14.5407 15.724 14.8947L13.8907 16.728C13.5358 17.0829 12.9731 17.0938 12.6328 16.7534L8.03766 12.1583L3.44437 16.7507C3.10402 17.091 2.54132 17.0801 2.18645 16.7253L0.273257 14.8121C-0.0807018 14.4572 -0.0925004 13.8945 0.247845 13.5542L4.84024 8.96087L0.32499 4.44653C-0.0153555 4.10619 -0.00355681 3.54258 0.350402 3.18862L2.18373 1.35529C2.53859 1.00042 3.1013 0.989533 3.44164 1.32988L7.95689 5.84422L12.5556 1.24638C12.8951 0.906035 13.4587 0.917833 13.8126 1.27179L15.7267 3.18589C16.0807 3.53985 16.0925 4.10346 15.7522 4.44381Z" fill="currentColor"/></svg>"#;

/// Minimize (–) icon for traffic light minimize button
/// viewBox: 0 0 17 6
/// Recommended size: 8x8
pub const TRAFFIC_LIGHT_MINIMIZE: &str = r#"<svg viewBox="0 0 17 6" fill="none"><path d="M1.47211 1.18042H15.4197C15.8052 1.18042 16.1179 1.50551 16.1179 1.90769V3.73242C16.1179 4.13387 15.8052 4.80006 15.4197 4.80006H1.47211C1.08665 4.80006 0.773926 4.47497 0.773926 4.07278V1.90769C0.773926 1.50551 1.08665 1.18042 1.47211 1.18042Z" fill="currentColor"/></svg>"#;

/// Maximize/Zoom (+) icon for traffic light maximize button
/// viewBox: 0 0 17 16
/// Recommended size: 8x8
pub const TRAFFIC_LIGHT_MAXIMIZE: &str = r#"<svg viewBox="0 0 17 16" fill="none"><path d="M15.5308 9.80147H10.3199V15.0095C10.3199 15.3949 9.9941 15.7076 9.59265 15.7076H7.51555C7.11337 15.7076 6.78828 15.3949 6.78828 15.0095V9.80147H1.58319C1.19774 9.80147 0.88501 9.47638 0.88501 9.07419V6.90619C0.88501 6.50401 1.19774 6.17892 1.58319 6.17892H6.78828V1.06183C6.78828 0.676375 7.11337 0.363647 7.51555 0.363647H9.59265C9.9941 0.363647 10.3199 0.676375 10.3199 1.06183V6.17892H15.5308C15.9163 6.17892 16.229 6.50401 16.229 6.90619V9.07419C16.229 9.47638 15.9163 9.80147 15.5308 9.80147Z" fill="currentColor"/></svg>"#;

/// Fullscreen (⤢) icon for traffic light fullscreen mode
/// viewBox: 0 0 15 15
/// Recommended size: 6x6
pub const TRAFFIC_LIGHT_FULLSCREEN: &str = r#"<svg viewBox="0 0 15 15" fill="none"><path d="M3.53068 0.433838L15.0933 12.0409C15.0933 12.0409 15.0658 5.35028 15.0658 4.01784C15.0658 1.32095 14.1813 0.433838 11.5378 0.433838C10.6462 0.433838 3.53068 0.433838 3.53068 0.433838ZM12.4409 15.5378L0.87735 3.93073C0.87735 3.93073 0.905794 10.6214 0.905794 11.9538C0.905794 14.6507 1.79024 15.5378 4.43291 15.5378C5.32535 15.5378 12.4409 15.5378 12.4409 15.5378Z" fill="currentColor"/></svg>"#;

/// Radio button dot (filled circle) for selected radio state
/// viewBox: 0 0 100 100
/// Recommended size: 7-8 px (inner dot, 40% of radio button)
pub const RADIO_DOT: &str = r#"<svg viewBox="0 0 100 100" fill="none"><circle cx="50" cy="50" r="40" fill="currentColor"/></svg>"#;

/// Clear/close (X) icon for general use
/// viewBox: 0 0 100 100
/// Recommended size: 16x16 for buttons, 12x12 for inputs
pub const CLEAR: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 25 16 L 50 41 L 75 16 L 84 25 L 59 50 L 84 75 L 75 84 L 50 59 L 25 84 L 16 75 L 41 50 L 16 25 Z" fill="currentColor"/></svg>"#;

/// Minimize icon (horizontal line) for general use
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const MINIMIZE: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 22 43 L 78 43 C 82 43 85 45 85 48 L 85 52 C 85 55 82 57 78 57 L 22 57 C 18 57 15 55 15 52 L 15 48 C 15 45 18 43 22 43 Z" fill="currentColor"/></svg>"#;

/// Maximize/expand icon for general use
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const MAXIMIZE: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 57 23 C 61 23 61 25 57 29 L 29 59 C 25 63 23 63 23 59 L 23 29 C 23 25 25 23 29 23 L 57 23 Z M 40 76 C 36 76 36 74 40 70 L 68 41 C 72 37 75 37 75 41 L 75 70 C 75 74 72 76 68 76 L 40 76 Z" fill="currentColor"/></svg>"#;

/// Restore/windowed mode icon
/// viewBox: 0 0 100 100
/// Recommended size: 16x16
pub const RESTORE: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M 14 50 C 10 50 10 48 14 44 L 42 14 C 46 10 48 10 48 14 L 48 44 C 48 48 46 50 42 50 L 14 50 Z M 84 50 C 88 50 88 52 84 56 L 56 85 C 52 89 49 89 49 85 L 49 56 C 49 52 52 50 56 50 L 84 50 Z" fill="currentColor"/></svg>"#;

/// Search/magnifying glass icon
/// viewBox: 0 0 100 100
/// Recommended size: 16x16 for inputs, 20x20 for buttons
pub const SEARCH: &str = r#"<svg viewBox="0 0 100 100" fill="none"><circle cx="38" cy="38" r="25" stroke="currentColor" stroke-width="6" fill="none"/><path d="M 57 57 L 82 82" stroke="currentColor" stroke-width="6" stroke-linecap="round"/></svg>"#;

/// Home / house icon
/// viewBox: 0 0 100 100
/// Recommended size: 22x22 for nav bars, 20x20 for toolbars
pub const HOME: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M15 50L50 15L85 50M25 45V80C25 82 27 84 29 84H42V62H58V84H71C73 84 75 82 75 80V45" stroke="currentColor" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// Gear / settings icon
/// viewBox: 0 0 100 100
/// Recommended size: 22x22 for nav bars, 20x20 for toolbars
pub const GEAR: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M50 35A15 15 0 1050 65A15 15 0 1050 35Z" stroke="currentColor" stroke-width="6"/><path d="M50 10V20M50 80V90M10 50H20M80 50H90M22 22L29 29M71 71L78 78M78 22L71 29M29 71L22 78" stroke="currentColor" stroke-width="6" stroke-linecap="round"/></svg>"#;

/// Plus / add icon
/// viewBox: 0 0 100 100
/// Recommended size: 16x16 for buttons, 20x20 for toolbars
pub const PLUS: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M50 20V80M20 50H80" stroke="currentColor" stroke-width="7" stroke-linecap="round"/></svg>"#;

/// Document / logs / list icon
/// viewBox: 0 0 100 100
/// Recommended size: 22x22 for nav bars, 20x20 for toolbars
pub const DOCUMENT: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M25 15H60L75 30V85H25V15Z" stroke="currentColor" stroke-width="6" stroke-linejoin="round"/><path d="M60 15V30H75" stroke="currentColor" stroke-width="6" stroke-linejoin="round"/><path d="M35 50H65M35 62H65M35 74H55" stroke="currentColor" stroke-width="5" stroke-linecap="round"/></svg>"#;

/// Server / database icon
/// viewBox: 0 0 100 100
/// Recommended size: 22x22 for nav bars, 20x20 for toolbars
pub const SERVER: &str = r#"<svg viewBox="0 0 100 100" fill="none"><ellipse cx="50" cy="25" rx="30" ry="10" stroke="currentColor" stroke-width="6"/><path d="M20 25V50C20 55.5 36 60 50 60C64 60 80 55.5 80 50V25" stroke="currentColor" stroke-width="6"/><path d="M20 50V75C20 80.5 36 85 50 85C64 85 80 80.5 80 75V50" stroke="currentColor" stroke-width="6"/></svg>"#;

/// Globe / world icon
/// viewBox: 0 0 100 100
/// Recommended size: 20x20 for toolbars, 16x16 for inline
pub const GLOBE: &str = r#"<svg viewBox="0 0 100 100" fill="none"><circle cx="50" cy="50" r="35" stroke="currentColor" stroke-width="6"/><ellipse cx="50" cy="50" rx="18" ry="35" stroke="currentColor" stroke-width="5"/><path d="M15 50H85M18 30H82M18 70H82" stroke="currentColor" stroke-width="4"/></svg>"#;

/// Shield / security icon
/// viewBox: 0 0 100 100
/// Recommended size: 20x20 for toolbars, 22x22 for nav bars
pub const SHIELD: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M50 10L15 28V50C15 72 30 88 50 95C70 88 85 72 85 50V28L50 10Z" stroke="currentColor" stroke-width="6" stroke-linejoin="round"/><path d="M35 52L46 63L65 40" stroke="currentColor" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// Menu / hamburger icon
/// viewBox: 0 0 100 100
/// Recommended size: 20x20 for toolbars, 22x22 for nav bars
pub const MENU: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M20 30H80M20 50H80M20 70H80" stroke="currentColor" stroke-width="7" stroke-linecap="round"/></svg>"#;

/// Info / about icon (circle with i)
/// viewBox: 0 0 100 100
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const INFO: &str = r#"<svg viewBox="0 0 100 100" fill="none"><circle cx="50" cy="50" r="35" stroke="currentColor" stroke-width="6"/><path d="M50 45V70" stroke="currentColor" stroke-width="7" stroke-linecap="round"/><circle cx="50" cy="32" r="4" fill="currentColor"/></svg>"#;

/// Bell / notification icon
/// viewBox: 0 0 100 100
/// Recommended size: 20x20 for toolbars, 22x22 for nav bars
pub const BELL: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M40 80C40 86 44 90 50 90C56 90 60 86 60 80" stroke="currentColor" stroke-width="5"/><path d="M25 68H75L70 55C68 42 62 30 50 25C38 30 32 42 30 55L25 68Z" stroke="currentColor" stroke-width="6" stroke-linejoin="round"/></svg>"#;

/// Link / chain icon
/// viewBox: 0 0 100 100
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const LINK: &str = r#"<svg viewBox="0 0 100 100" fill="none"><path d="M42 58L58 42M35 65C25 55 25 39 35 29L40 24C50 14 66 14 76 24C86 34 86 50 76 60L71 65" stroke="currentColor" stroke-width="6" stroke-linecap="round"/><path d="M65 35C75 45 75 61 65 71L60 76C50 86 34 86 24 76C14 66 14 50 24 40L29 35" stroke="currentColor" stroke-width="6" stroke-linecap="round"/></svg>"#;

/// Trash / delete icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for buttons, 20x20 for toolbars
pub const DELETE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 6h18"/>
  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/>
  <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
</svg>"##;

/// Lock (closed) icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const LOCK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <rect x="5" y="11" width="14" height="10" rx="2"/>
  <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
</svg>"##;

/// Lock open (unlocked) icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const UNLOCK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="5" y="11" width="14" height="10" rx="2"/>
  <path d="M7 11V7a5 5 0 0 1 9.9-1"/>
</svg>"##;

/// Eye / visible icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const EYE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z"/>
  <circle cx="12" cy="12" r="3"/>
  <path d="M7 7L5.5 4.5"/>
  <path d="M12 5V2"/>
  <path d="M17 7l1.5-2.5"/>
</svg>"##;

/// Eye off / hidden icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const EYE_OFF: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z"/>
  <path d="M7 17l-1.5 2.5"/>
  <path d="M12 19v3"/>
  <path d="M17 17l1.5 2.5"/>
</svg>"##;

/// Close (X) icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for buttons, 20x20 for toolbars
pub const CLOSE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <line x1="18" y1="6" x2="6" y2="18"/>
  <line x1="6" y1="6" x2="18" y2="18"/>
</svg>"##;

/// Shield with checkmark icon (security verified)
/// viewBox: 0 0 24 24
/// Recommended size: 20x20 for toolbars, 22x22 for nav bars
pub const SHIELD_CHECK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M9 12l2 2 4-4"/></svg>"##;

/// Key icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const KEY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>"##;

/// Copy / clipboard icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const COPY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <rect x="9" y="9" width="13" height="13" rx="2"/>
  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
</svg>"##;

/// Star (empty) icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const STAR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>"##;

/// Star filled icon
/// viewBox: 0 0 24 24
/// Recommended size: 16x16 for inline, 20x20 for buttons
pub const STAR_FILLED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_paths_are_valid_svg() {
        let paths = [
            CHECKMARK,
            CHECKMARK_MIXED,
            CHEVRON_RIGHT,
            CHEVRON_LEFT,
            CHEVRON_DOWN,
            CHEVRON_UP,
            ARROW_UP,
            ARROW_DOWN,
            TRAFFIC_LIGHT_CLOSE,
            TRAFFIC_LIGHT_MINIMIZE,
            TRAFFIC_LIGHT_MAXIMIZE,
            TRAFFIC_LIGHT_FULLSCREEN,
            RADIO_DOT,
            CLEAR,
            MINIMIZE,
            MAXIMIZE,
            RESTORE,
            SEARCH,
            HOME,
            GEAR,
            PLUS,
            DOCUMENT,
            SERVER,
            GLOBE,
            SHIELD,
            MENU,
            INFO,
            BELL,
            LINK,
            DELETE,
            LOCK,
            UNLOCK,
            EYE,
            EYE_OFF,
            CLOSE,
            SHIELD_CHECK,
            KEY,
            COPY,
            STAR,
            STAR_FILLED,
        ];

        for path in &paths {
            assert!(path.starts_with("<svg"));
            assert!(path.ends_with("</svg>"));
            assert!(path.contains("viewBox"));
        }
    }

    #[test]
    fn test_traffic_light_icons_have_correct_viewbox() {
        assert!(TRAFFIC_LIGHT_CLOSE.contains(r#"viewBox="0 0 16 18"#));
        assert!(TRAFFIC_LIGHT_MINIMIZE.contains(r#"viewBox="0 0 17 6"#));
        assert!(TRAFFIC_LIGHT_MAXIMIZE.contains(r#"viewBox="0 0 17 16"#));
        assert!(TRAFFIC_LIGHT_FULLSCREEN.contains(r#"viewBox="0 0 15 15"#));
    }

    #[test]
    fn test_standard_icons_have_100x100_viewbox() {
        let standard_icons = [
            CHECKMARK,
            CHECKMARK_MIXED,
            CHEVRON_RIGHT,
            CHEVRON_LEFT,
            CHEVRON_DOWN,
            CHEVRON_UP,
            ARROW_UP,
            ARROW_DOWN,
            RADIO_DOT,
            CLEAR,
            MINIMIZE,
            MAXIMIZE,
            RESTORE,
            SEARCH,
            HOME,
            GEAR,
            PLUS,
            DOCUMENT,
            SERVER,
            GLOBE,
            SHIELD,
            MENU,
            INFO,
            BELL,
            LINK,
        ];

        for icon in &standard_icons {
            assert!(icon.contains(r#"viewBox="0 0 100 100"#));
        }
    }
}
