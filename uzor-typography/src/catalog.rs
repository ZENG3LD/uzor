//! Theme and preset files. Built-in names resolve in code; every other name
//! is `{home}/themes/{name}.toml` or `{home}/presets/{name}.toml`.

use std::path::Path;

use uzor::fonts::FontFamily;
use uzor_typeset::{CellPadding, Margins, PageNumberFormat, PageNumberStyle, TableRules};

use crate::preset::{HeadingSpace, Preset, Sheet, SheetExtras};
use crate::press::{
    CalloutStyle, CoverStyle, DocStyle, InsetLabel, InsetStyle, Palette, RuleStyle, Sizes, StrokeMark,
};

const MM: f64 = 72.0 / 25.4;

/// Built-in themes win. Any other name is a file under `home`.
pub fn load_theme(name: &str, home: Option<&Path>) -> Result<Palette, String> {
    match name {
        "light" => return Ok(Palette::light()),
        "dark" => return Ok(Palette::dark()),
        _ => {}
    }
    let path = file_path(name, home, "themes", "theme")?;
    let source = std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    parse_theme(&source, &path.display().to_string())
}

/// Built-in presets win. Any other name is a file under `home`.
pub fn load_preset(name: &str, home: Option<&Path>) -> Result<Preset, String> {
    match name {
        "column" => return Ok(Preset::column()),
        "notice" => return Ok(Preset::notice()),
        "filing" => return Ok(Preset::filing()),
        "long" => return Ok(Preset::long()),
        "dense" => return Ok(Preset::dense()),
        _ => {}
    }
    let path = file_path(name, home, "presets", "preset")?;
    let source = std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    parse_preset(&source, &path.display().to_string())
}

fn file_path(name: &str, home: Option<&Path>, dir: &str, kind: &str) -> Result<std::path::PathBuf, String> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(format!("invalid {kind} name {name:?}"));
    }
    let home = home.ok_or_else(|| format!("{kind} {name:?} is not built in and PRESS_HOME is not set"))?;
    Ok(home.join(dir).join(format!("{name}.toml")))
}

fn parse_theme(source_text: &str, source: &str) -> Result<Palette, String> {
    let table: toml::Table = toml::from_str(source_text).map_err(|e| format!("parsing {source}: {e}"))?;
    let mut bg = None;
    let mut panel = None;
    let mut ink = None;
    let mut muted = None;
    let mut accent = None;
    let mut second = None;
    let mut line = None;
    let mut up = None;
    let mut down = None;
    let mut categorical_extra = Vec::new();
    let mut font_display = FontFamily::Roboto;
    let mut font_mono = FontFamily::Roboto;
    let mut logo_sentinel = None;
    let mut wordmark = None;
    let mut mark = None;
    let mut doc = DocStyle::default();
    let mut saw_doc = false;

    for (key, value) in &table {
        match key.as_str() {
            "bg" => bg = Some(hex_value(value, "bg", source)?),
            "panel" => panel = Some(hex_value(value, "panel", source)?),
            "ink" => ink = Some(hex_value(value, "ink", source)?),
            "muted" => muted = Some(hex_value(value, "muted", source)?),
            "accent" => accent = Some(hex_value(value, "accent", source)?),
            "second" => second = Some(hex_value(value, "second", source)?),
            "line" => line = Some(hex_value(value, "line", source)?),
            "up" => up = Some(hex_value(value, "up", source)?),
            "down" => down = Some(hex_value(value, "down", source)?),
            "categorical_extra" => categorical_extra = hex_list(value, "categorical_extra", source)?,
            "font_display" => font_display = font_value(value, "font_display", source)?,
            "font_mono" => font_mono = font_value(value, "font_mono", source)?,
            "logo_sentinel" => logo_sentinel = Some(string_value(value, "logo_sentinel", source)?),
            "wordmark" => wordmark = Some(string_value(value, "wordmark", source)?),
            "mark" => mark = Some(parse_mark(value, source)?),
            "doc" => {
                saw_doc = true;
                doc = parse_doc(value, source)?;
            }
            other => return Err(format!("unknown key {other:?} in {source}")),
        }
    }

    let require = |slot: Option<u32>, key: &str| slot.ok_or_else(|| format!("missing key {key:?} in {source}"));
    let mut logo_mark = mark;
    if let Some(text) = wordmark {
        let mark = logo_mark.as_mut().ok_or_else(|| format!("key \"wordmark\" in {source} requires \"mark\""))?;
        mark.wordmark = Some(text);
    }
    if !saw_doc {
        doc = DocStyle::default();
    }
    Ok(Palette {
        bg: require(bg, "bg")?,
        panel: require(panel, "panel")?,
        ink: require(ink, "ink")?,
        muted: require(muted, "muted")?,
        accent: require(accent, "accent")?,
        second: require(second, "second")?,
        line: require(line, "line")?,
        up: require(up, "up")?,
        down: require(down, "down")?,
        categorical_extra,
        font_display,
        font_mono,
        doc,
        logo_sentinel,
        logo_mark,
    })
}

fn parse_mark(value: &toml::Value, source: &str) -> Result<StrokeMark, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"mark\" in {source} must be a table"))?;
    let mut viewbox = None;
    let mut stroke_width = None;
    let mut polylines = None;
    for (key, value) in table {
        match key.as_str() {
            "viewbox" => viewbox = Some(number(value, "mark.viewbox", source)?),
            "stroke_width" => stroke_width = Some(number(value, "mark.stroke_width", source)?),
            "polylines" => polylines = Some(parse_polylines(value, source)?),
            other => return Err(format!("unknown key \"mark.{other}\" in {source}")),
        }
    }
    Ok(StrokeMark {
        viewbox: viewbox.ok_or_else(|| format!("missing key \"mark.viewbox\" in {source}"))?,
        stroke_width: stroke_width.ok_or_else(|| format!("missing key \"mark.stroke_width\" in {source}"))?,
        polylines: polylines.ok_or_else(|| format!("missing key \"mark.polylines\" in {source}"))?,
        wordmark: None,
    })
}

fn parse_polylines(value: &toml::Value, source: &str) -> Result<Vec<Vec<(f64, f64)>>, String> {
    let lines = value.as_array().ok_or_else(|| format!("key \"mark.polylines\" in {source} must be an array"))?;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let points = line.as_array().ok_or_else(|| format!("key \"mark.polylines\" in {source} must be an array of point pairs"))?;
        let mut poly = Vec::with_capacity(points.len());
        for point in points {
            let pair = point.as_array().ok_or_else(|| format!("key \"mark.polylines\" in {source} must be an array of point pairs"))?;
            if pair.len() != 2 {
                return Err(format!("key \"mark.polylines\" in {source} must be an array of point pairs"));
            }
            poly.push((number(&pair[0], "mark.polylines", source)?, number(&pair[1], "mark.polylines", source)?));
        }
        out.push(poly);
    }
    Ok(out)
}

fn parse_doc(value: &toml::Value, source: &str) -> Result<DocStyle, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"doc\" in {source} must be a table"))?;
    let mut doc = DocStyle::default();
    for (key, value) in table {
        match key.as_str() {
            "docx_font" => doc.docx_font = Some(string_value(value, "doc.docx_font", source)?),
            "table_rule" => doc.table_rule = Some(parse_rule(value, source)?),
            "table_head_fill" => doc.table_head_fill = Some(hex_value(value, "doc.table_head_fill", source)?),
            "table_total_fill" => doc.table_total_fill = Some(hex_value(value, "doc.table_total_fill", source)?),
            "callout" => doc.callout = parse_callout(value, source)?,
            "page_mark" => doc.page_mark = Some(hex_value(value, "doc.page_mark", source)?),
            "cover" => doc.cover = parse_cover(value, source)?,
            "footer_logo" => doc.footer_logo = bool_value(value, "doc.footer_logo", source)?,
            "symbol_face" => doc.symbol_face = Some(font_value(value, "doc.symbol_face", source)?),
            "total_prefixes" => doc.total_prefixes = string_list(value, "doc.total_prefixes", source)?,
            "revised" => doc.revised = parse_inset(value, "doc.revised", source)?,
            "technical" => doc.technical = parse_inset(value, "doc.technical", source)?,
            other => return Err(format!("unknown key \"doc.{other}\" in {source}")),
        }
    }
    Ok(doc)
}

fn parse_rule(value: &toml::Value, source: &str) -> Result<RuleStyle, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"doc.table_rule\" in {source} must be a table"))?;
    let mut color = None;
    let mut width = None;
    for (key, value) in table {
        match key.as_str() {
            "color" => color = Some(hex_value(value, "doc.table_rule.color", source)?),
            "width" => width = Some(number(value, "doc.table_rule.width", source)?),
            other => return Err(format!("unknown key \"doc.table_rule.{other}\" in {source}")),
        }
    }
    Ok(RuleStyle {
        color: color.ok_or_else(|| format!("missing key \"doc.table_rule.color\" in {source}"))?,
        width: width.ok_or_else(|| format!("missing key \"doc.table_rule.width\" in {source}"))?,
    })
}

fn parse_callout(value: &toml::Value, source: &str) -> Result<CalloutStyle, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"doc.callout\" in {source} must be a table"))?;
    let mut kind = None;
    let mut fill = None;
    let mut bar = None;
    let mut bar_width = None;
    for (key, value) in table {
        match key.as_str() {
            "kind" => kind = Some(string_value(value, "doc.callout.kind", source)?),
            "fill" => fill = Some(hex_value(value, "doc.callout.fill", source)?),
            "bar" => bar = Some(hex_value(value, "doc.callout.bar", source)?),
            "bar_width" => bar_width = Some(number(value, "doc.callout.bar_width", source)?),
            other => return Err(format!("unknown key \"doc.callout.{other}\" in {source}")),
        }
    }
    match kind.as_deref() {
        Some("boxed") => Ok(CalloutStyle::Boxed),
        Some("tinted") => Ok(CalloutStyle::Tinted {
            fill: fill.ok_or_else(|| format!("missing key \"doc.callout.fill\" in {source}"))?,
            bar: bar.ok_or_else(|| format!("missing key \"doc.callout.bar\" in {source}"))?,
            bar_width: bar_width.ok_or_else(|| format!("missing key \"doc.callout.bar_width\" in {source}"))?,
        }),
        Some(other) => Err(format!("unknown value {other:?} for key \"doc.callout.kind\" in {source}")),
        None => Err(format!("missing key \"doc.callout.kind\" in {source}")),
    }
}

fn parse_cover(value: &toml::Value, source: &str) -> Result<CoverStyle, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"doc.cover\" in {source} must be a table"))?;
    let mut kind = None;
    let mut shape = None;
    let mut slant = None;
    for (key, value) in table {
        match key.as_str() {
            "kind" => kind = Some(string_value(value, "doc.cover.kind", source)?),
            "shape" => shape = Some(hex_value(value, "doc.cover.shape", source)?),
            "slant" => slant = Some(number(value, "doc.cover.slant", source)?),
            other => return Err(format!("unknown key \"doc.cover.{other}\" in {source}")),
        }
    }
    match kind.as_deref() {
        Some("standard") => Ok(CoverStyle::Standard),
        Some("band") => Ok(CoverStyle::Band {
            shape: shape.ok_or_else(|| format!("missing key \"doc.cover.shape\" in {source}"))?,
            slant: slant.ok_or_else(|| format!("missing key \"doc.cover.slant\" in {source}"))?,
        }),
        Some(other) => Err(format!("unknown value {other:?} for key \"doc.cover.kind\" in {source}")),
        None => Err(format!("missing key \"doc.cover.kind\" in {source}")),
    }
}

fn parse_inset(value: &toml::Value, key: &str, source: &str) -> Result<InsetStyle, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"{key}\" in {source} must be a table"))?;
    let mut style = InsetStyle::default();
    for (name, value) in table {
        match name.as_str() {
            "fill" => style.fill = Some(hex_value(value, &format!("{key}.fill"), source)?),
            "bar" => style.bar = Some(hex_value(value, &format!("{key}.bar"), source)?),
            "bar_width" => style.bar_width = number(value, &format!("{key}.bar_width"), source)?,
            "label" => style.label = Some(parse_label(value, key, source)?),
            other => return Err(format!("unknown key \"{key}.{other}\" in {source}")),
        }
    }
    Ok(style)
}

fn parse_label(value: &toml::Value, key: &str, source: &str) -> Result<InsetLabel, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"{key}.label\" in {source} must be a table"))?;
    let mut text = None;
    let mut size = None;
    for (name, value) in table {
        match name.as_str() {
            "text" => text = Some(string_value(value, &format!("{key}.label.text"), source)?),
            "size" => size = Some(number(value, &format!("{key}.label.size"), source)?),
            other => return Err(format!("unknown key \"{key}.label.{other}\" in {source}")),
        }
    }
    Ok(InsetLabel {
        text: text.ok_or_else(|| format!("missing key \"{key}.label.text\" in {source}"))?,
        size: size.ok_or_else(|| format!("missing key \"{key}.label.size\" in {source}"))?,
    })
}

fn parse_preset(source_text: &str, source: &str) -> Result<Preset, String> {
    let table: toml::Table = toml::from_str(source_text).map_err(|e| format!("parsing {source}: {e}"))?;
    let mut page = None;
    let mut slides = None;
    for (key, value) in &table {
        match key.as_str() {
            "page" => page = Some(parse_sheet(value, "page", source)?),
            "slides" => slides = Some(parse_sheet(value, "slides", source)?),
            other => return Err(format!("unknown key {other:?} in {source}")),
        }
    }
    Ok(Preset {
        page: page.ok_or_else(|| format!("missing key \"page\" in {source}"))?,
        slides,
    })
}

fn parse_sheet(value: &toml::Value, prefix: &str, source: &str) -> Result<Sheet, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"{prefix}\" in {source} must be a table"))?;
    let mut margins = Margins::new(0.0, 0.0, 0.0, 0.0);
    let mut footer_from_edge = 0.0;
    let mut sizes = zero_sizes();
    let mut body_leading_value = None;
    let mut paragraph_spacing = 0.0;
    let mut justify = false;
    let mut hyphenate = false;
    let mut h1_rule = false;
    let mut table_rules = TableRules::Box;
    let mut cell_padding = CellPadding::new(0.0, 0.0);
    let mut page_number = PageNumberStyle::new(PageNumberFormat::Bare, 1);
    let mut baseline = None;
    let mut measure = None;
    let mut columns = 1usize;
    let mut column_gap = 0.0;
    let mut first_indent = 0.0;
    let mut text_family = None;
    let mut extras_value = None;

    for (key, value) in table {
        let path = format!("{prefix}.{key}");
        match key.as_str() {
            "margins_mm" => {
                let mm = number_array(value, &path, source, 4)?;
                margins = Margins::new(mm[0] * MM, mm[1] * MM, mm[2] * MM, mm[3] * MM);
            }
            "margins_pt" => {
                let pt = number_array(value, &path, source, 4)?;
                margins = Margins::new(pt[0], pt[1], pt[2], pt[3]);
            }
            "footer_from_edge_mm" => footer_from_edge = number(value, &path, source)? * MM,
            "footer_from_edge" => footer_from_edge = number(value, &path, source)?,
            "sizes" => sizes = parse_sizes(value, &path, source)?,
            "body_leading" => body_leading_value = Some(value),
            "paragraph_spacing" => paragraph_spacing = number(value, &path, source)?,
            "justify" => justify = bool_value(value, &path, source)?,
            "hyphenate" => hyphenate = bool_value(value, &path, source)?,
            "h1_rule" => h1_rule = bool_value(value, &path, source)?,
            "table_rules" => table_rules = parse_table_rules(value, &path, source)?,
            "cell_padding" => {
                let pair = number_array(value, &path, source, 2)?;
                cell_padding = CellPadding::new(pair[0], pair[1]);
            }
            "page_number" => page_number = parse_page_number(value, &path, source)?,
            "baseline" => baseline = Some(number(value, &path, source)?),
            "measure" => measure = Some(number(value, &path, source)?),
            "columns" => columns = usize_value(value, &path, source)?,
            "column_gap" => column_gap = number(value, &path, source)?,
            "first_indent" => first_indent = number(value, &path, source)?,
            "text_family" => text_family = Some(font_value(value, &path, source)?),
            "extras" => extras_value = Some(value),
            other => return Err(format!("unknown key \"{prefix}.{other}\" in {source}")),
        }
    }

    let body_leading = match body_leading_value {
        Some(value) => scaled_number(sizes.body, value, &format!("{prefix}.body_leading"), source)?,
        None => 0.0,
    };
    let extras = match extras_value {
        Some(value) => parse_extras(value, &format!("{prefix}.extras"), source, body_leading, sizes.list_item)?,
        None => SheetExtras::default(),
    };
    Ok(Sheet {
        margins,
        footer_from_edge,
        sizes,
        body_leading,
        paragraph_spacing,
        justify,
        hyphenate,
        h1_rule,
        table_rules,
        cell_padding,
        page_number,
        baseline,
        measure,
        columns,
        column_gap,
        first_indent,
        text_family,
        extras,
    })
}

fn zero_sizes() -> Sizes {
    Sizes {
        h1: 0.0,
        h2: 0.0,
        h3: 0.0,
        body: 0.0,
        list_item: 0.0,
        table_header: 0.0,
        table_cell: 0.0,
        caption: 0.0,
        footer: 0.0,
        callout: 0.0,
        kpi_value: 0.0,
        kpi_label: 0.0,
        cover_title: 0.0,
        cover_sub: 0.0,
        cover_meta: 0.0,
        diagram_title: 0.0,
        diagram_subtitle: 0.0,
    }
}

fn parse_sizes(value: &toml::Value, prefix: &str, source: &str) -> Result<Sizes, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"{prefix}\" in {source} must be a table"))?;
    let mut sizes = zero_sizes();
    for (key, value) in table {
        let path = format!("{prefix}.{key}");
        let v = number(value, &path, source)?;
        match key.as_str() {
            "h1" => sizes.h1 = v,
            "h2" => sizes.h2 = v,
            "h3" => sizes.h3 = v,
            "body" => sizes.body = v,
            "list_item" => sizes.list_item = v,
            "table_header" => sizes.table_header = v,
            "table_cell" => sizes.table_cell = v,
            "caption" => sizes.caption = v,
            "footer" => sizes.footer = v,
            "callout" => sizes.callout = v,
            "kpi_value" => sizes.kpi_value = v,
            "kpi_label" => sizes.kpi_label = v,
            "cover_title" => sizes.cover_title = v,
            "cover_sub" => sizes.cover_sub = v,
            "cover_meta" => sizes.cover_meta = v,
            "diagram_title" => sizes.diagram_title = v,
            "diagram_subtitle" => sizes.diagram_subtitle = v,
            other => return Err(format!("unknown key \"{prefix}.{other}\" in {source}")),
        }
    }
    Ok(sizes)
}

fn parse_page_number(value: &toml::Value, prefix: &str, source: &str) -> Result<PageNumberStyle, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"{prefix}\" in {source} must be a table"))?;
    let mut format = PageNumberFormat::Bare;
    let mut start = 1u32;
    let mut suppress_if_single = false;
    let mut suppress_title = false;
    for (key, value) in table {
        let path = format!("{prefix}.{key}");
        match key.as_str() {
            "format" => {
                format = match string_value(value, &path, source)?.as_str() {
                    "bare" => PageNumberFormat::Bare,
                    "of-total" => PageNumberFormat::OfTotal,
                    other => return Err(format!("unknown value {other:?} for key \"{path}\" in {source}")),
                };
            }
            "start" => {
                let n = value.as_integer().ok_or_else(|| format!("key \"{path}\" in {source} must be an integer"))?;
                if n < 0 {
                    return Err(format!("key \"{path}\" in {source} must be an integer"));
                }
                start = n as u32;
            }
            "suppress_if_single" => suppress_if_single = bool_value(value, &path, source)?,
            "suppress_title" => suppress_title = bool_value(value, &path, source)?,
            other => return Err(format!("unknown key \"{prefix}.{other}\" in {source}")),
        }
    }
    Ok(PageNumberStyle::new(format, start).with_suppress_if_single(suppress_if_single).with_suppress_title(suppress_title))
}

fn parse_extras(value: &toml::Value, prefix: &str, source: &str, body_leading: f64, list_item: f64) -> Result<SheetExtras, String> {
    let table = value.as_table().ok_or_else(|| format!("key \"{prefix}\" in {source} must be a table"))?;
    let mut extras = SheetExtras::default();
    for (key, value) in table {
        let path = format!("{prefix}.{key}");
        match key.as_str() {
            "heading_space" => extras.heading_space = parse_heading_space(value, &path, source)?,
            "heading_leading" => extras.heading_leading = Some(heading_leading(value, &path, source, body_leading)?),
            "h1_page_break" => extras.h1_page_break = bool_value(value, &path, source)?,
            "list_leading" => extras.list_leading = Some(scaled_number(list_item, value, &path, source)?),
            "justify_lists" => extras.justify_lists = bool_value(value, &path, source)?,
            "docx_cell_margins" => extras.docx_cell_margins = bool_value(value, &path, source)?,
            other => return Err(format!("unknown key \"{prefix}.{other}\" in {source}")),
        }
    }
    Ok(extras)
}

fn parse_heading_space(value: &toml::Value, key: &str, source: &str) -> Result<[HeadingSpace; 3], String> {
    let rows = value.as_array().ok_or_else(|| format!("key \"{key}\" in {source} must be an array of 3 pairs"))?;
    if rows.len() != 3 {
        return Err(format!("key \"{key}\" in {source} must be an array of 3 pairs"));
    }
    let mut out = [HeadingSpace::default(); 3];
    for (i, row) in rows.iter().enumerate() {
        let pair = number_array(row, key, source, 2)?;
        out[i] = HeadingSpace { before: pair[0], after: pair[1] };
    }
    Ok(out)
}

fn heading_leading(value: &toml::Value, key: &str, source: &str, body_leading: f64) -> Result<f64, String> {
    if let Some(text) = value.as_str() {
        return if text == "body" {
            Ok(body_leading)
        } else {
            Err(format!("unknown value {text:?} for key \"{key}\" in {source}"))
        };
    }
    scaled_number(body_leading, value, key, source)
}

/// A bare number, or `{ ratio = [a, b, ...] }` meaning `base * a * b * ...`.
fn scaled_number(base: f64, value: &toml::Value, key: &str, source: &str) -> Result<f64, String> {
    if value.as_float().is_some() || value.as_integer().is_some() {
        return number(value, key, source);
    }
    let table = value.as_table().ok_or_else(|| format!("key \"{key}\" in {source} must be a number or a table"))?;
    let mut ratio = None;
    for (name, value) in table {
        match name.as_str() {
            "ratio" => ratio = Some(value),
            other => return Err(format!("unknown key \"{key}.{other}\" in {source}")),
        }
    }
    let ratio = ratio.ok_or_else(|| format!("missing key \"{key}.ratio\" in {source}"))?;
    let factors = ratio.as_array().ok_or_else(|| format!("key \"{key}.ratio\" in {source} must be an array of numbers"))?;
    let mut out = base;
    for factor in factors {
        out *= number(factor, &format!("{key}.ratio"), source)?;
    }
    Ok(out)
}

fn parse_table_rules(value: &toml::Value, key: &str, source: &str) -> Result<TableRules, String> {
    match string_value(value, key, source)?.as_str() {
        "box" => Ok(TableRules::Box),
        "horizontal" => Ok(TableRules::Horizontal),
        other => Err(format!("unknown value {other:?} for key \"{key}\" in {source}")),
    }
}

fn number(value: &toml::Value, key: &str, source: &str) -> Result<f64, String> {
    value.as_float().or_else(|| value.as_integer().map(|i| i as f64)).ok_or_else(|| format!("key \"{key}\" in {source} must be a number"))
}

fn number_array(value: &toml::Value, key: &str, source: &str, len: usize) -> Result<Vec<f64>, String> {
    let items = value.as_array().ok_or_else(|| format!("key \"{key}\" in {source} must be an array of {len} numbers"))?;
    if items.len() != len {
        return Err(format!("key \"{key}\" in {source} must be an array of {len} numbers"));
    }
    items.iter().map(|item| number(item, key, source)).collect()
}

fn bool_value(value: &toml::Value, key: &str, source: &str) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| format!("key \"{key}\" in {source} must be a boolean"))
}

fn string_value(value: &toml::Value, key: &str, source: &str) -> Result<String, String> {
    value.as_str().map(str::to_owned).ok_or_else(|| format!("key \"{key}\" in {source} must be a string"))
}

fn string_list(value: &toml::Value, key: &str, source: &str) -> Result<Vec<String>, String> {
    let items = value.as_array().ok_or_else(|| format!("key \"{key}\" in {source} must be an array of strings"))?;
    items.iter().map(|item| string_value(item, key, source)).collect()
}

fn usize_value(value: &toml::Value, key: &str, source: &str) -> Result<usize, String> {
    let n = value.as_integer().ok_or_else(|| format!("key \"{key}\" in {source} must be an integer"))?;
    if n < 0 {
        return Err(format!("key \"{key}\" in {source} must be an integer"));
    }
    Ok(n as usize)
}

fn hex_value(value: &toml::Value, key: &str, source: &str) -> Result<u32, String> {
    let text = string_value(value, key, source)?;
    parse_hex(&text, key, source)
}

fn hex_list(value: &toml::Value, key: &str, source: &str) -> Result<Vec<u32>, String> {
    let items = value.as_array().ok_or_else(|| format!("key \"{key}\" in {source} must be an array of #RRGGBB colors"))?;
    items.iter().map(|item| hex_value(item, key, source)).collect()
}

fn parse_hex(text: &str, key: &str, source: &str) -> Result<u32, String> {
    let hex = text.strip_prefix('#').ok_or_else(|| format!("key \"{key}\" in {source} must be a #RRGGBB color"))?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("key \"{key}\" in {source} must be a #RRGGBB color"));
    }
    u32::from_str_radix(hex, 16).map_err(|_| format!("key \"{key}\" in {source} must be a #RRGGBB color"))
}

fn font_value(value: &toml::Value, key: &str, source: &str) -> Result<FontFamily, String> {
    match string_value(value, key, source)?.as_str() {
        "roboto" => Ok(FontFamily::Roboto),
        "pt-root-ui" => Ok(FontFamily::PtRootUi),
        "jetbrains-mono" => Ok(FontFamily::JetBrainsMono),
        "source-serif-4" => Ok(FontFamily::SourceSerif4),
        other => Err(format!("unknown font {other:?} for key \"{key}\" in {source}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_preset, parse_theme};

    #[test]
    fn a_theme_file_parses_anonymous_colors_a_mark_and_the_sentinel() {
        let palette = parse_theme(
            r##"
                bg = "#112233"
                panel = "#000000"
                ink = "#ffffff"
                muted = "#888888"
                accent = "#ff0000"
                second = "#00ff00"
                line = "#cccccc"
                up = "#00aa00"
                down = "#aa0000"
                font_display = "roboto"
                font_mono = "jetbrains-mono"
                logo_sentinel = "mark"
                wordmark = "MARK"
                [mark]
                viewbox = 10
                stroke_width = 1.5
                polylines = [[[0, 0], [10, 10]]]
            "##,
            "mem",
        )
        .expect("theme parses");
        assert_eq!(palette.bg, 0x112233);
        assert_eq!(palette.accent, 0xff0000);
        assert_eq!(palette.font_mono, uzor::fonts::FontFamily::JetBrainsMono);
        assert_eq!(palette.logo_sentinel.as_deref(), Some("mark"));
        let mark = palette.logo_mark.expect("mark");
        assert_eq!(mark.viewbox, 10.0);
        assert_eq!(mark.stroke_width, 1.5);
        assert_eq!(mark.polylines, vec![vec![(0.0, 0.0), (10.0, 10.0)]]);
        assert_eq!(mark.wordmark.as_deref(), Some("MARK"));
        assert!(palette.doc.total_prefixes.is_empty());

        let err = parse_theme("bg = \"#112233\"\nbogus = 1\n", "mem").expect_err("unknown key");
        assert!(err.contains("bogus"), "{err}");
    }

    #[test]
    fn load_preset_long_is_built_in() {
        let preset = load_preset("long", None).expect("long is built in");
        assert!(preset.slides.is_none());
        assert!(preset.page.justify && preset.page.hyphenate);
        assert_eq!(preset.page.columns, 1);
        assert!(!preset.page.extras.h1_page_break);
        assert_eq!(preset.page.sizes.h1, 16.0);
        assert_eq!(preset.page.sizes.h2, 13.0);
        assert_eq!(preset.page.sizes.h3, 12.0);
        assert_eq!(preset.page.sizes.body, 11.0);
        assert_eq!(preset.page.paragraph_spacing, 8.0);
        let line = 11.0 * 1.2 * 1.15;
        assert!((preset.page.body_leading - line).abs() < 1e-9);
        assert_eq!(preset.page.extras.heading_leading, Some(line));
        assert_eq!(preset.page.table_rules, uzor_typeset::TableRules::Horizontal);
        const MM: f64 = 72.0 / 25.4;
        assert_eq!(preset.page.margins.top, 20.0 * MM);
        assert_eq!(preset.page.margins.right, 18.0 * MM);
        assert_eq!(preset.page.margins.bottom, 20.0 * MM);
        assert_eq!(preset.page.margins.left, 22.0 * MM);
    }
}
