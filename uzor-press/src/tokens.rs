//! Typography overrides from `--tokens <file.toml>`: tune the resolved
//! preset's page sheet without a rebuild. Every key is optional; an absent
//! key keeps the preset's own value and an unknown key is a hard error
//! naming the key. When `sizes.body` changes and `page.body_leading` is
//! absent, the body line is recomputed as body x 1.2 x 1.15 — the preset's
//! own ratio rule — and the leading values derived from it
//! (`heading_leading` when it tracked the body line, `list_leading` when
//! `list_item` changes) follow.

use std::path::Path;

use uzor_typeset::Margins;

use crate::preset::Preset;

/// Point sizes named on the sheet's type scale (`Sizes`). Integers in the
/// TOML (`body = 11`) are accepted as floats.
#[derive(Debug, Default, Clone, Copy)]
struct SizeTokens {
    h1: Option<f64>,
    h2: Option<f64>,
    h3: Option<f64>,
    body: Option<f64>,
    list_item: Option<f64>,
    table_header: Option<f64>,
    table_cell: Option<f64>,
    caption: Option<f64>,
    footer: Option<f64>,
    callout: Option<f64>,
}

/// Page-sheet values. `margins_mm` is millimetres in `Margins::new` order:
/// top, right, bottom, left.
#[derive(Debug, Default, Clone, Copy)]
struct PageTokens {
    margins_mm: Option<[f64; 4]>,
    body_leading: Option<f64>,
    paragraph_spacing: Option<f64>,
}

/// A parsed `--tokens` file, ready to apply to any preset.
#[derive(Debug, Default, Clone, Copy)]
pub struct Tokens {
    sizes: SizeTokens,
    page: PageTokens,
}

impl Tokens {
    /// Read and parse the file. A missing or unreadable file and malformed
    /// TOML are ordinary errors, never a panic.
    pub fn load(path: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        Self::parse(&source, &path.display().to_string())
    }

    /// `source` names the file in error messages.
    fn parse(source_text: &str, source: &str) -> Result<Self, String> {
        let table: toml::Table = toml::from_str(source_text).map_err(|e| format!("parsing {source}: {e}"))?;
        let mut tokens = Tokens::default();
        for (key, value) in &table {
            match key.as_str() {
                "sizes" => tokens.sizes = parse_sizes(value, source)?,
                "page" => tokens.page = parse_page(value, source)?,
                other => return Err(format!("unknown key {other:?} in {source}")),
            }
        }
        Ok(tokens)
    }

    /// Override the preset's page sheet. Values not named in the file keep
    /// the preset's own; the body/list lines follow the ratio rule above.
    pub fn apply(&self, preset: &mut Preset) {
        const MM: f64 = 72.0 / 25.4;
        let sheet = &mut preset.page;
        let old_body_line = sheet.body_leading;
        let old_list_item = sheet.sizes.list_item;

        let sizes = &self.sizes;
        if let Some(v) = sizes.h1 {
            sheet.sizes.h1 = v;
        }
        if let Some(v) = sizes.h2 {
            sheet.sizes.h2 = v;
        }
        if let Some(v) = sizes.h3 {
            sheet.sizes.h3 = v;
        }
        if let Some(v) = sizes.body {
            sheet.sizes.body = v;
        }
        if let Some(v) = sizes.list_item {
            sheet.sizes.list_item = v;
        }
        if let Some(v) = sizes.table_header {
            sheet.sizes.table_header = v;
        }
        if let Some(v) = sizes.table_cell {
            sheet.sizes.table_cell = v;
        }
        if let Some(v) = sizes.caption {
            sheet.sizes.caption = v;
        }
        if let Some(v) = sizes.footer {
            sheet.sizes.footer = v;
        }
        if let Some(v) = sizes.callout {
            sheet.sizes.callout = v;
        }

        if let Some(mm) = self.page.margins_mm {
            sheet.margins = Margins::new(mm[0] * MM, mm[1] * MM, mm[2] * MM, mm[3] * MM);
        }
        if let Some(v) = self.page.body_leading {
            sheet.body_leading = v;
        }
        if let Some(v) = self.page.paragraph_spacing {
            sheet.paragraph_spacing = v;
        }

        if sizes.body.is_some() && self.page.body_leading.is_none() {
            let line = sheet.sizes.body * 1.2 * 1.15;
            sheet.body_leading = line;
            if sheet.extras.heading_leading == Some(old_body_line) {
                sheet.extras.heading_leading = Some(line);
            }
        }
        if sheet.sizes.list_item != old_list_item && sheet.extras.list_leading == Some(old_list_item * 1.2 * 1.15) {
            sheet.extras.list_leading = Some(sheet.sizes.list_item * 1.2 * 1.15);
        }
    }
}

/// One TOML number: a float, or an integer widened to a float.
fn number(value: &toml::Value, key: &str, source: &str) -> Result<f64, String> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|i| i as f64))
        .ok_or_else(|| format!("key {key:?} in {source} must be a number"))
}

fn parse_sizes(value: &toml::Value, source: &str) -> Result<SizeTokens, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"sizes\" in {source} must be a table"))?;
    let mut sizes = SizeTokens::default();
    for (key, value) in table {
        let v = number(value, &format!("sizes.{key}"), source)?;
        match key.as_str() {
            "h1" => sizes.h1 = Some(v),
            "h2" => sizes.h2 = Some(v),
            "h3" => sizes.h3 = Some(v),
            "body" => sizes.body = Some(v),
            "list_item" => sizes.list_item = Some(v),
            "table_header" => sizes.table_header = Some(v),
            "table_cell" => sizes.table_cell = Some(v),
            "caption" => sizes.caption = Some(v),
            "footer" => sizes.footer = Some(v),
            "callout" => sizes.callout = Some(v),
            other => return Err(format!("unknown key \"sizes.{other}\" in {source}")),
        }
    }
    Ok(sizes)
}

fn parse_page(value: &toml::Value, source: &str) -> Result<PageTokens, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"page\" in {source} must be a table"))?;
    let mut page = PageTokens::default();
    for (key, value) in table {
        match key.as_str() {
            "margins_mm" => {
                let items = value.as_array().ok_or_else(|| format!("key \"page.margins_mm\" in {source} must be an array of 4 numbers"))?;
                if items.len() != 4 {
                    return Err(format!("key \"page.margins_mm\" in {source} must be an array of 4 numbers"));
                }
                let mut mm = [0.0; 4];
                for (i, item) in items.iter().enumerate() {
                    mm[i] = number(item, "page.margins_mm", source)?;
                }
                page.margins_mm = Some(mm);
            }
            "body_leading" => page.body_leading = Some(number(value, "page.body_leading", source)?),
            "paragraph_spacing" => page.paragraph_spacing = Some(number(value, "page.paragraph_spacing", source)?),
            other => return Err(format!("unknown key \"page.{other}\" in {source}")),
        }
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::Tokens;
    use crate::preset::Preset;

    const MM: f64 = 72.0 / 25.4;

    /// Overrides land on top of the preset, absent keys keep its values, and
    /// a body change without an explicit line recomputes the body line and
    /// the heading line that tracked it (the preset's own ratio rule).
    #[test]
    fn tokens_override_the_preset_and_recompute_the_lines() {
        let mut preset = Preset::long();
        let tokens = Tokens::parse(
            r#"
                [sizes]
                body = 10.5
                callout = 9
                [page]
                margins_mm = [20.0, 15.0, 20.0, 30.0]
                paragraph_spacing = 4.0
            "#,
            "test",
        )
        .unwrap();
        tokens.apply(&mut preset);

        let sheet = &preset.page;
        assert_eq!(sheet.sizes.body, 10.5);
        assert_eq!(sheet.sizes.callout, 9.0);
        assert_eq!(sheet.sizes.h1, 16.0, "absent keys keep the preset's values");
        assert!((sheet.body_leading - 10.5 * 1.2 * 1.15).abs() < 1e-9);
        assert_eq!(sheet.extras.heading_leading, Some(10.5 * 1.2 * 1.15), "headings share the body line on this preset");
        assert_eq!(sheet.paragraph_spacing, 4.0);
        assert_eq!(sheet.margins.top, 20.0 * MM);
        assert_eq!(sheet.margins.right, 15.0 * MM);
        assert_eq!(sheet.margins.bottom, 20.0 * MM);
        assert_eq!(sheet.margins.left, 30.0 * MM);
    }

    /// An explicit `body_leading` wins over the ratio rule; a `list_item`
    /// change rescales a list line the preset derived from the old size.
    #[test]
    fn an_explicit_body_line_wins_and_the_list_line_follows_its_size() {
        let mut preset = Preset::long();
        let tokens = Tokens::parse(
            r#"
                [sizes]
                body = 10.5
                list_item = 9.5
                [page]
                body_leading = 14.0
            "#,
            "test",
        )
        .unwrap();
        tokens.apply(&mut preset);

        let sheet = &preset.page;
        assert_eq!(sheet.body_leading, 14.0);
        assert_eq!(sheet.extras.heading_leading, Some(11.0 * 1.2 * 1.15), "untouched: the ratio rule never ran");
        assert_eq!(sheet.extras.list_leading, Some(9.5 * 1.2 * 1.15));
    }

    /// A key the surface does not name is a hard error carrying the key.
    #[test]
    fn unknown_keys_are_a_hard_error_naming_the_key() {
        let err = Tokens::parse("[sizes]\nbogus = 1.0", "test").unwrap_err();
        assert!(err.contains("sizes.bogus"), "{err}");
        let err = Tokens::parse("[page]\ncolumns = 2", "test").unwrap_err();
        assert!(err.contains("page.columns"), "{err}");
        let err = Tokens::parse("[bogus]\nbody = 11.0", "test").unwrap_err();
        assert!(err.contains("\"bogus\""), "{err}");
    }

    /// A missing file is a clean error, not a panic.
    #[test]
    fn a_missing_tokens_file_errors_cleanly() {
        let path = std::path::Path::new("no-such-tokens-file-here.toml");
        let err = Tokens::load(path).unwrap_err();
        assert!(err.contains("no-such-tokens-file-here.toml"), "{err}");
    }
}
