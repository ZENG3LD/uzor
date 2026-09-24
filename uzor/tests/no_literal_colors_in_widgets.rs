//! H1 design doc §5's literal-enforcement test: no colour literal — a hex
//! string, an `rgba(...)` call written literally inside a string, or a
//! `[u8; 4]` array of literal VALUES (never the type annotation `[u8; 4]`
//! itself) — may be reachable from widget render code outside a comment.
//!
//! `tokens.rs` files are the ONE blanket, structural exemption: they ARE the
//! token tables the rest of each widget's code reads through
//! (`component_tokens!`'s default `ColorSpec` table, H1 §3). There is no
//! other allow-list — every other literal this scan finds must be rewritten,
//! per Brief 9 item c's "zero exceptions" mandate. Two call sites needed
//! exactly that rewrite to land clean here: `toast::theme::rgba` and
//! `color_swatch::render::rgba_css` both used to build an `"rgba(...)"`
//! string via `format!`, which put the literal text `rgba(` inside a string
//! literal; both now render through `crate::tokens::Rgba::to_css_hex`
//! instead (same pixel, canonical hex output, no matching shape). Likewise
//! `slider::render::hex_to_rgba`'s malformed-hex fallback used to be the bare
//! array literal `[255, 255, 255, 255]`; it now reads `Rgba::WHITE`'s own
//! channel fields instead.
//!
//! Pure text scan over source lines, not an AST parse — mirrors the H1
//! inventory's own method (three regex-shaped passes) but hand-written (no
//! `regex` dependency: `uzor`'s own D3 "no new dependency" rule applies to
//! its test suite too) since every shape here is simple and well-known.

use std::path::{Path, PathBuf};

/// Recursively collects every `.rs` file under `root`.
fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Drops everything from the first `//` that is NOT inside a `"..."` string
/// literal — covers `///`/`//!` doc comments too (both start with `//`).
fn strip_comment(line: &str) -> &str {
    let chars: Vec<char> = line.chars().collect();
    let mut in_string = false;
    let mut escaped = false;
    let mut byte_pos = 0;
    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            return &line[..byte_pos];
        }
        byte_pos += c.len_utf8();
    }
    line
}

/// Extracts the contents of every `"..."` string literal on a
/// (comment-stripped) line — scopes the hex/`rgba(` checks to text an
/// end-user actually sees rendered, never to an identifier or attribute that
/// merely contains the same characters outside any string.
fn string_literals(code: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            let mut escaped = false;
            while j < bytes.len() {
                if escaped {
                    escaped = false;
                } else if bytes[j] == b'\\' {
                    escaped = true;
                } else if bytes[j] == b'"' {
                    break;
                }
                j += 1;
            }
            out.push(&code[start..j.min(bytes.len())]);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// True if `s` contains `#` followed by a run of 3 to 8 hex digits — the
/// shape of every colour-literal hex string in this codebase (`#fff`,
/// `#d1d4dc`, `#161b22ff`, ...).
fn contains_hex_color_literal(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c != '#' {
            continue;
        }
        let run = chars[i + 1..].iter().take_while(|c| c.is_ascii_hexdigit()).count();
        if (3..=8).contains(&run) {
            return true;
        }
    }
    false
}

/// True if `code` contains a `[u8; 4]`-shaped literal VALUE array — four
/// comma-separated decimal integers (each `0..=255`) inside brackets, e.g.
/// `[255, 255, 255, 255]`. Deliberately distinct from the TYPE annotation
/// `[u8; 4]` (one `u8` identifier and a `;`, no commas at all) — only the
/// four-value shape is a colour literal.
fn contains_u8x4_array_literal(code: &str) -> bool {
    let mut search_from = 0;
    while let Some(rel_open) = code[search_from..].find('[') {
        let open = search_from + rel_open;
        let Some(rel_close) = code[open..].find(']') else {
            break;
        };
        let close = open + rel_close;
        let inner = &code[open + 1..close];
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        let is_u8x4_values = parts.len() == 4
            && parts.iter().all(|p| {
                !p.is_empty()
                    && p.chars().all(|c| c.is_ascii_digit())
                    && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
            });
        if is_u8x4_values {
            return true;
        }
        search_from = close + 1;
    }
    false
}

/// Scans one file's lines, appending a `path:line: description` entry to
/// `hits` for every literal found.
fn scan_file(path: &Path, hits: &mut Vec<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for (lineno, line) in text.lines().enumerate() {
        let code = strip_comment(line);
        if code.trim().is_empty() {
            continue;
        }
        for literal in string_literals(code) {
            if contains_hex_color_literal(literal) {
                hits.push(format!("{}:{}: hex colour literal {literal:?}", path.display(), lineno + 1));
            }
            if literal.contains("rgba(") {
                hits.push(format!("{}:{}: rgba( literal {literal:?}", path.display(), lineno + 1));
            }
        }
        if contains_u8x4_array_literal(code) {
            hits.push(format!(
                "{}:{}: [u8; 4] literal array value: {}",
                path.display(),
                lineno + 1,
                code.trim()
            ));
        }
    }
}

/// Fails if any `.rs` file under `src/ui/widgets/{atomic,composite}/**`,
/// `src/framework/widgets/lm/**`, or `src/layout/**` contains a colour
/// literal outside a comment line and outside a `tokens.rs` file (module
/// doc's blanket exemption). The latter two roots joined the scan in H1
/// Brief 9 item 2 once `StyleManager`'s removal took every remaining
/// hand-copied literal fallback out of the `lm::*` bridge files and
/// `layout::panel_api::types::PanelTheme`'s `Default` impl (now reads the
/// dark built-in token set instead).
#[test]
fn no_literal_colors_reachable_from_widget_render_code() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roots = [
        manifest_dir.join("src/ui/widgets"),
        manifest_dir.join("src/framework/widgets/lm"),
        manifest_dir.join("src/layout"),
    ];

    let mut files = Vec::new();
    for root in &roots {
        collect_rs_files(root, &mut files);
    }
    assert!(!files.is_empty(), "expected to find source files under {roots:?}");

    let mut hits = Vec::new();
    for path in files {
        if path.file_name().and_then(|f| f.to_str()) == Some("tokens.rs") {
            continue;
        }
        scan_file(&path, &mut hits);
    }
    assert!(
        hits.is_empty(),
        "colour literals reachable from widget/layout render code:\n{}",
        hits.join("\n")
    );
}

/// Guards against `button/defaults.rs` (a 237-hex `*PrototypeColors` catalog,
/// confirmed zero call sites, H1 §3) resurfacing after deletion.
#[test]
fn button_prototype_catalog_stays_deleted() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/widgets/atomic/button/defaults.rs");
    assert!(!p.exists(), "button/defaults.rs must stay deleted (H1 §3)");
}

#[cfg(test)]
mod self_tests {
    use super::*;

    #[test]
    fn strip_comment_drops_from_first_unquoted_slash_slash() {
        assert_eq!(strip_comment(r#"    let x = 1; // a comment"#), "    let x = 1; ");
        assert_eq!(strip_comment("/// doc comment with \"#ffffff\""), "");
        assert_eq!(strip_comment("//! module doc with rgba("), "");
        assert_eq!(strip_comment(r#"    let s = "not // a comment";"#), r#"    let s = "not // a comment";"#);
    }

    #[test]
    fn string_literals_extracts_quoted_content_only() {
        assert_eq!(string_literals("fn f(&self) -> &str { \"#ffffff\" }"), vec!["#ffffff"]);
        assert_eq!(string_literals("let a = 1; let b = 2;"), Vec::<&str>::new());
    }

    #[test]
    fn hex_color_literal_detection() {
        assert!(contains_hex_color_literal("#fff"));
        assert!(contains_hex_color_literal("#d1d4dc"));
        assert!(contains_hex_color_literal("#161b22ff"));
        assert!(!contains_hex_color_literal("transparent"));
        assert!(!contains_hex_color_literal("rainbow"));
    }

    #[test]
    fn u8x4_array_literal_detection_excludes_the_type_annotation() {
        assert!(contains_u8x4_array_literal("[255, 255, 255, 255]"));
        assert!(contains_u8x4_array_literal("[0,0,0,0]"));
        assert!(!contains_u8x4_array_literal("fn f(hex: &str) -> [u8; 4] {"));
        assert!(!contains_u8x4_array_literal("let v: [u8; 4] = w.to_rgba8();"));
    }

    #[test]
    fn rgba_call_syntax_is_not_flagged_outside_a_string() {
        // `rgba(hex_to_rgb(theme.shadow()), alpha)` is a plain function call,
        // not a colour literal — it must never appear inside `string_literals`
        // output for a line shaped like this.
        let code = "ctx.set_fill_color(&rgba(hex_to_rgb(theme.shadow()), alpha));";
        assert!(string_literals(code).is_empty());
    }
}
