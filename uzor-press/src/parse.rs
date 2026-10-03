//! Owned markdown-subset AST — front matter + block parser.
//!
//! Parses exactly the small markdown subset `SPEC.md` describes; anything
//! this parser cannot recognize is a hard parse error carrying the source
//! file's own 1-based line number, per `SPEC.md`'s "error on anything
//! else."

use std::fmt;
use std::path::PathBuf;

/// A parse error tied to a 1-based source line number.
#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

type ParseResult<T> = Result<T, ParseError>;

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError { line, message: message.into() }
}

/// Front matter fields (`key: value` between `---` lines).
#[derive(Debug, Clone)]
pub struct FrontMatter {
    pub title: String,
    /// Footer line text; empty when the key is absent.
    pub footer: String,
    pub lang: String,
    pub author: Option<String>,
    pub subject: Option<String>,
    /// Theme-independent brand mark: drawn on the cover (top-left/left
    /// band) and as a small mark at the left of the footer line on every
    /// page. Optional — every pre-existing content file (no `logo:` key)
    /// renders byte-identically to before.
    pub logo: Option<PathBuf>,
    /// A short case/product tag rendered as a rounded pill (accent fill,
    /// `bg`-colored uppercase text) at the top-right of the cover.
    /// Optional, same byte-identical-when-absent guarantee as `logo`.
    pub badge: Option<String>,
    /// Second cover line under `title` (the band cover). Optional.
    pub subtitle: Option<String>,
    /// Cover author lines, from `authors: A / B / C` (split on ` / `);
    /// empty when the key is absent. Read by the band cover only.
    pub authors: Vec<String>,
    /// Cover date line (band cover). Optional.
    pub date: Option<String>,
}

/// One inline text run: plain text or `**bold**`.
#[derive(Debug, Clone)]
pub enum InlineRun {
    Plain(String),
    Bold(String),
}

/// Which side an anchored island's image sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandSide {
    Left,
    Right,
}

/// `:::cover` / `:::closing` fenced block fields (same key set, per
/// `SPEC.md`: "`:::closing` ... same keys as cover").
#[derive(Debug, Clone, Default)]
pub struct CoverLike {
    pub title: String,
    pub subtitle: String,
    pub meta: String,
    pub image: Option<PathBuf>,
    pub site: String,
}

/// A `|---|---:|:---:|`-style table column's own alignment, from its
/// separator cell's leading/trailing `:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
}

/// One box inside a `:::diagram` row.
#[derive(Debug, Clone)]
pub struct DiagramBox {
    pub title: String,
    pub subtitle: Option<String>,
}

/// One parsed image directive (`![alt](path){...attrs...}`). `alt` text is
/// accepted (required for correct `![...]` syntax recognition) but not
/// carried forward — `uzor_typeset::ImageBlock` has no alt-text field, and
/// PDF `/Alt` tagging is resolved by the engine itself from a following
/// caption paragraph (see `press.rs`'s own `push_caption`), never from a
/// separate alt string.
#[derive(Debug, Clone)]
pub struct ImageSpec {
    pub path: PathBuf,
    pub height: Option<f64>,
    pub island: Option<IslandSide>,
    pub width: Option<f64>,
    pub caption: Option<String>,
}

/// Which side block a `>>` / `>>>` chunk is. Both read like a diff against
/// the text around them; they differ in colour and in the size of their text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsetKind {
    /// `>> `: the rewritten wording of a passage.
    Revised,
    /// `>>> `: a technical addition to the document.
    Technical,
}

impl InsetKind {
    /// The chevrons that open every line of the block.
    pub fn marker(self) -> &'static str {
        match self {
            InsetKind::Revised => ">>",
            InsetKind::Technical => ">>>",
        }
    }
}

/// One part of a `>>` / `>>>` block: the block kinds that make sense inside a
/// rewritten passage. Anything else is a parse error, so such a block can
/// never hold a heading, an image or another side block.
#[derive(Debug, Clone)]
pub enum InsetPart {
    Paragraph(Vec<InlineRun>),
    List { ordered: bool, items: Vec<Vec<InlineRun>> },
    Table { header: Vec<Vec<InlineRun>>, rows: Vec<Vec<Vec<InlineRun>>>, fractions: Vec<f64>, aligns: Vec<ColumnAlign> },
}

/// One content block.
#[derive(Debug, Clone)]
pub enum Block {
    Heading { level: u8, runs: Vec<InlineRun> },
    Paragraph(Vec<InlineRun>),
    List { ordered: bool, items: Vec<Vec<InlineRun>> },
    Callout(Vec<InlineRun>),
    /// `>> ` / `>>> ` lines: text shown next to the original, read like a
    /// diff. One or more parts, in source order.
    Inset { kind: InsetKind, parts: Vec<InsetPart> },
    Table { header: Vec<Vec<InlineRun>>, rows: Vec<Vec<Vec<InlineRun>>>, fractions: Vec<f64>, aligns: Vec<ColumnAlign> },
    Image(ImageSpec),
    Kpi(Vec<(String, String)>),
    Diagram(Vec<Vec<DiagramBox>>),
    Closing(CoverLike),
    Spacer(f64),
    /// `:::clear` — a no-op flow marker (island-strip authoring hint);
    /// dropped when the AST is turned into a scene.
    Clear,
}

/// One `---`-separated body section: a doc page-break hint, or one deck
/// slide — the press layer decides which, per `--format`.
#[derive(Debug, Clone, Default)]
pub struct Section {
    pub blocks: Vec<Block>,
}

/// The whole parsed document: front matter, an OPTIONAL leading
/// `:::cover` block, and the `---`-separated body sections that follow it.
/// `cover` is `None` when the file opens directly with an ordinary block
/// (e.g. a `# heading`) instead of a `:::cover` fence — the press layer
/// then skips the cover slice entirely and composes page 1 with the
/// ordinary footer master.
#[derive(Debug, Clone)]
pub struct Document {
    pub front_matter: FrontMatter,
    pub cover: Option<CoverLike>,
    pub sections: Vec<Section>,
}

/// Parse the whole content file. Every [`ParseError::line`] is 1-based,
/// matching the source file's own line numbers.
pub fn parse(source: &str) -> ParseResult<Document> {
    let lines: Vec<&str> = source.lines().collect();
    let (front_matter, rest_start) = parse_front_matter(&lines)?;

    // Split the remaining lines into raw chunks: maximal runs of
    // non-blank lines. A chunk that is exactly one line reading `---` is
    // a section separator, never content.
    let mut chunks: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut i = rest_start;
    while i < lines.len() {
        if lines[i].trim().is_empty() {
            i += 1;
            continue;
        }
        let start = i;
        let mut chunk = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            chunk.push(lines[i]);
            i += 1;
        }
        chunks.push((start + 1, chunk));
    }

    if chunks.is_empty() {
        return Err(err(rest_start + 1, "empty document body: expected at least one block"));
    }

    // The very first chunk MAY be the cover fence — optional: a file may
    // instead open directly with an ordinary block (e.g. a `# heading`).
    let (cover_line, cover_chunk) = &chunks[0];
    let opens_with_cover = matches!(cover_chunk.first(), Some(first) if first.trim() == ":::cover");
    let (cover, body_chunks) = if opens_with_cover {
        let cover_inner = fence_inner(*cover_line, "cover", cover_chunk)?;
        (Some(parse_cover_like(*cover_line, cover_inner)?), &chunks[1..])
    } else {
        (None, &chunks[..])
    };

    // Remaining chunks: split into sections at `---` separators.
    let mut sections: Vec<Section> = vec![Section::default()];
    for (line_no, chunk) in body_chunks {
        if chunk.len() == 1 && chunk[0].trim() == "---" {
            sections.push(Section::default());
            continue;
        }
        let block = parse_block(*line_no, chunk)?;
        sections.last_mut().expect("always at least one section").blocks.push(block);
    }
    // A `---` immediately after the cover (deck.md's own shape) leaves a
    // pre-seeded empty leading section — drop it rather than emitting an
    // empty slide/page-break-only section.
    sections.retain(|s| !s.blocks.is_empty());

    Ok(Document { front_matter, cover, sections })
}

fn parse_front_matter(lines: &[&str]) -> ParseResult<(FrontMatter, usize)> {
    if lines.is_empty() || lines[0].trim() != "---" {
        return Err(err(1, "file must start with a `---` front-matter block"));
    }
    let mut title = None;
    let mut footer = None;
    let mut lang = "ru".to_owned();
    let mut author = None;
    let mut subject = None;
    let mut logo = None;
    let mut badge = None;
    let mut subtitle = None;
    let mut authors = Vec::new();
    let mut date = None;
    let mut i = 1;
    loop {
        if i >= lines.len() {
            return Err(err(i + 1, "unterminated front matter: expected a closing `---`"));
        }
        if lines[i].trim() == "---" {
            i += 1;
            break;
        }
        let line_no = i + 1;
        let (key, value) = split_key_value(line_no, lines[i])?;
        match key {
            "title" => title = Some(value.to_owned()),
            "footer" => footer = Some(value.to_owned()),
            "lang" => lang = value.to_owned(),
            "author" => author = Some(value.to_owned()),
            "subject" => subject = Some(value.to_owned()),
            "logo" => logo = Some(PathBuf::from(value)),
            "badge" => badge = Some(value.to_owned()),
            "subtitle" => subtitle = Some(value.to_owned()),
            "authors" => authors = value.split(" / ").map(|line| line.trim().to_owned()).filter(|line| !line.is_empty()).collect(),
            "date" => date = Some(value.to_owned()),
            other => return Err(err(line_no, format!("unknown front matter key `{other}`"))),
        }
        i += 1;
    }
    let title = title.ok_or_else(|| err(1, "front matter missing required key `title`"))?;
    let footer = footer.unwrap_or_default();
    Ok((FrontMatter { title, footer, lang, author, subject, logo, badge, subtitle, authors, date }, i))
}

fn split_key_value(line_no: usize, raw: &str) -> ParseResult<(&str, &str)> {
    let (key, value) = raw.split_once(':').ok_or_else(|| err(line_no, format!("malformed `key: value` line: {raw:?}")))?;
    Ok((key.trim(), value.trim()))
}

/// Strip a `:::keyword` opening line and its closing `:::` line off
/// `chunk`, returning the inner lines. `chunk[0]` must already be known to
/// open the fence named `keyword`.
fn fence_inner<'a>(line_no: usize, keyword: &str, chunk: &'a [&'a str]) -> ParseResult<&'a [&'a str]> {
    if chunk.len() < 2 || chunk.last().copied().unwrap_or_default().trim() != ":::" {
        return Err(err(line_no, format!("unterminated `:::{keyword}` block (missing closing `:::`)")));
    }
    Ok(&chunk[1..chunk.len() - 1])
}

fn parse_block(line_no: usize, chunk: &[&str]) -> ParseResult<Block> {
    let first = chunk[0].trim();

    if let Some(keyword) = first.strip_prefix(":::") {
        let keyword = keyword.trim();
        if let Some(rest) = keyword.strip_prefix("spacer") {
            if chunk.len() != 1 {
                return Err(err(line_no, "`:::spacer N` must be a single line"));
            }
            let n: f64 = rest
                .trim()
                .parse()
                .map_err(|_| err(line_no, format!("`:::spacer` needs a numeric height, got {first:?}")))?;
            return Ok(Block::Spacer(n));
        }
        if keyword == "clear" {
            if chunk.len() != 1 {
                return Err(err(line_no, "`:::clear` must be a single line"));
            }
            return Ok(Block::Clear);
        }
        let fence_name = keyword.split_whitespace().next().unwrap_or(keyword);
        let inner = fence_inner(line_no, fence_name, chunk)?;
        return match fence_name {
            "cover" => Err(err(line_no, "`:::cover` may only be the document's first block")),
            "closing" => Ok(Block::Closing(parse_cover_like(line_no, inner)?)),
            "kpi" => Ok(Block::Kpi(parse_kpi(line_no, inner)?)),
            "diagram" => Ok(Block::Diagram(parse_diagram(line_no, inner)?)),
            other => Err(err(line_no, format!("unknown fenced block `:::{other}`"))),
        };
    }

    if first.starts_with('#') {
        let level = heading_level(first)
            .ok_or_else(|| err(line_no, format!("malformed heading (expected 1-3 `#` followed by a space): {first:?}")))?;
        if chunk.len() != 1 {
            return Err(err(line_no, "a heading must be a single line"));
        }
        let text = first[level as usize..].trim();
        return Ok(Block::Heading { level, runs: parse_inline(text) });
    }

    if first.starts_with(">>>") {
        return parse_inset(line_no, chunk, InsetKind::Technical);
    }
    if first.starts_with(">>") {
        return parse_inset(line_no, chunk, InsetKind::Revised);
    }

    if first.starts_with('>') {
        let text = chunk.iter().map(|l| l.trim().trim_start_matches('>').trim()).collect::<Vec<_>>().join(" ");
        return Ok(Block::Callout(parse_inline(&text)));
    }

    if is_list_line(first) {
        return parse_list(line_no, chunk);
    }

    if first.starts_with('|') {
        return parse_table(line_no, chunk);
    }

    if first.starts_with("![") {
        return parse_image(line_no, chunk);
    }

    // Plain paragraph: consecutive non-blank lines join with a space.
    let text = chunk.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
    Ok(Block::Paragraph(parse_inline(&text)))
}

/// A `>>` / `>>>` chunk: every line is `>> text` (marker, one space, text) or
/// a bare marker that acts as the blank line between the block's own parts.
/// The text between separators is parsed exactly like an ordinary chunk
/// (paragraph, list or table); other block kinds are refused.
fn parse_inset(line_no: usize, chunk: &[&str], kind: InsetKind) -> ParseResult<Block> {
    let marker = kind.marker();
    let opener = format!("{marker} ");
    let mut stripped: Vec<&str> = Vec::with_capacity(chunk.len());
    for (offset, raw) in chunk.iter().enumerate() {
        let trimmed = raw.trim();
        let text = if trimmed == marker {
            ""
        } else {
            trimmed
                .strip_prefix(opener.as_str())
                .ok_or_else(|| err(line_no + offset, format!("a `{marker}` block line must start with `{opener}` (or be a bare `{marker}`), got {trimmed:?}")))?
                .trim()
        };
        stripped.push(text);
    }

    let mut parts = Vec::new();
    let mut i = 0;
    while i < stripped.len() {
        if stripped[i].is_empty() {
            i += 1;
            continue;
        }
        let start = i;
        while i < stripped.len() && !stripped[i].is_empty() {
            i += 1;
        }
        let part_line = line_no + start;
        let part = match parse_block(part_line, &stripped[start..i])? {
            Block::Paragraph(runs) => InsetPart::Paragraph(runs),
            Block::List { ordered, items } => InsetPart::List { ordered, items },
            Block::Table { header, rows, fractions, aligns } => InsetPart::Table { header, rows, fractions, aligns },
            _ => return Err(err(part_line, format!("a `{marker}` block may hold paragraphs, lists and tables only"))),
        };
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(err(line_no, format!("a `{marker}` block needs at least one line of text")));
    }
    Ok(Block::Inset { kind, parts })
}

fn heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 3 {
        return None;
    }
    if line.as_bytes().get(hashes) != Some(&b' ') {
        return None;
    }
    Some(hashes as u8)
}

fn numbered_prefix(line: &str) -> Option<usize> {
    let digits_end = line.find(|c: char| !c.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    if line[digits_end..].starts_with(". ") {
        Some(digits_end + 2)
    } else {
        None
    }
}

fn is_list_line(line: &str) -> bool {
    line.starts_with("- ") || numbered_prefix(line).is_some()
}

fn parse_list(line_no: usize, chunk: &[&str]) -> ParseResult<Block> {
    let ordered = numbered_prefix(chunk[0].trim()).is_some();
    let mut items = Vec::with_capacity(chunk.len());
    for (offset, raw) in chunk.iter().enumerate() {
        let trimmed = raw.trim();
        let text = if ordered {
            let cut = numbered_prefix(trimmed)
                .ok_or_else(|| err(line_no + offset, format!("expected a numbered list item, got {trimmed:?}")))?;
            &trimmed[cut..]
        } else {
            trimmed
                .strip_prefix("- ")
                .ok_or_else(|| err(line_no + offset, format!("expected a bulleted list item, got {trimmed:?}")))?
        };
        items.push(parse_inline(text.trim()));
    }
    Ok(Block::List { ordered, items })
}

fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_start_matches('|').trim_end_matches('|');
    trimmed.split('|').map(|c| c.trim().to_owned()).collect()
}

fn parse_table(line_no: usize, chunk: &[&str]) -> ParseResult<Block> {
    if chunk.len() < 2 {
        return Err(err(line_no, "a table needs a header row and a `|---|---|` separator row"));
    }
    let header_cells = split_table_row(chunk[0]);
    let sep_cells = split_table_row(chunk[1]);
    if sep_cells.len() != header_cells.len() || !sep_cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')) {
        return Err(err(line_no + 1, "expected a `|---|---|`-style separator row after the table header"));
    }
    let fractions: Vec<f64> = sep_cells.iter().map(|c| c.chars().filter(|&ch| ch == '-').count().max(1) as f64).collect();
    let aligns: Vec<ColumnAlign> = sep_cells
        .iter()
        .map(|c| match (c.starts_with(':'), c.ends_with(':')) {
            (true, true) => ColumnAlign::Center,
            (false, true) => ColumnAlign::Right,
            _ => ColumnAlign::Left,
        })
        .collect();
    let header: Vec<Vec<InlineRun>> = header_cells.iter().map(|c| parse_inline(c)).collect();
    let mut rows = Vec::new();
    for (offset, raw) in chunk[2..].iter().enumerate() {
        let cells = split_table_row(raw);
        if cells.len() != header.len() {
            return Err(err(line_no + 2 + offset, format!("table row has {} cells, expected {}", cells.len(), header.len())));
        }
        rows.push(cells.iter().map(|c| parse_inline(c)).collect());
    }
    Ok(Block::Table { header, rows, fractions, aligns })
}

fn parse_image(line_no: usize, chunk: &[&str]) -> ParseResult<Block> {
    let first = chunk[0].trim();
    let rest = first.strip_prefix("![").ok_or_else(|| err(line_no, format!("malformed image directive: {first:?}")))?;
    let (_alt, rest) = rest
        .split_once("](")
        .ok_or_else(|| err(line_no, format!("malformed image directive (missing `](`): {first:?}")))?;
    let (path_str, rest) = rest
        .split_once(')')
        .ok_or_else(|| err(line_no, format!("malformed image directive (missing closing `)`): {first:?}")))?;
    let attrs_str = rest.trim();

    let mut height = None;
    let mut island = None;
    let mut width = None;
    if !attrs_str.is_empty() {
        let attrs_str = attrs_str
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .ok_or_else(|| err(line_no, format!("malformed image attributes (expected `{{...}}`): {attrs_str:?}")))?;
        for kv in attrs_str.split_whitespace() {
            let (key, value) = kv.split_once('=').ok_or_else(|| err(line_no, format!("malformed image attribute `{kv}` (expected key=value)")))?;
            match key {
                "height" => height = Some(value.parse::<f64>().map_err(|_| err(line_no, format!("bad `height` value {value:?}")))?),
                "width" => width = Some(value.parse::<f64>().map_err(|_| err(line_no, format!("bad `width` value {value:?}")))?),
                "island" => {
                    island = Some(match value {
                        "left" => IslandSide::Left,
                        "right" => IslandSide::Right,
                        other => return Err(err(line_no, format!("bad `island` value {other:?} (expected left|right)"))),
                    })
                }
                other => return Err(err(line_no, format!("unknown image attribute `{other}`"))),
            }
        }
    }

    let caption = if chunk.len() > 1 {
        let cap_line = chunk[1].trim();
        let text = cap_line
            .strip_prefix("Caption: ")
            .ok_or_else(|| err(line_no + 1, format!("expected `Caption: ...` after the image, got {cap_line:?}")))?;
        if chunk.len() > 2 {
            return Err(err(line_no + 2, "an image block may hold at most one caption line"));
        }
        Some(text.trim().to_owned())
    } else {
        None
    };

    Ok(Block::Image(ImageSpec { path: PathBuf::from(path_str), height, island, width, caption }))
}

fn parse_kpi(line_no: usize, inner: &[&str]) -> ParseResult<Vec<(String, String)>> {
    inner
        .iter()
        .enumerate()
        .map(|(offset, raw)| {
            let (value, label) = raw.split_once('|').ok_or_else(|| err(line_no + 1 + offset, format!("expected `value | label`, got {raw:?}")))?;
            Ok((value.trim().to_owned(), label.trim().to_owned()))
        })
        .collect()
}

fn parse_diagram(line_no: usize, inner: &[&str]) -> ParseResult<Vec<Vec<DiagramBox>>> {
    inner
        .iter()
        .enumerate()
        .map(|(offset, raw)| {
            let row_line = line_no + 1 + offset;
            let rest = raw.trim().strip_prefix("row:").ok_or_else(|| err(row_line, format!("expected a `row: ...` line, got {raw:?}")))?;
            rest.split(';')
                .map(|box_str| {
                    let box_str = box_str.trim();
                    if box_str.is_empty() {
                        return Err(err(row_line, "a diagram row has an empty box (check for a stray `;`)"));
                    }
                    if let Some((title, subtitle)) = box_str.split_once('|') {
                        Ok(DiagramBox { title: title.trim().to_owned(), subtitle: Some(subtitle.trim().to_owned()) })
                    } else {
                        Ok(DiagramBox { title: box_str.to_owned(), subtitle: None })
                    }
                })
                .collect()
        })
        .collect()
}

fn parse_cover_like(line_no: usize, inner: &[&str]) -> ParseResult<CoverLike> {
    let mut cover = CoverLike::default();
    for (offset, raw) in inner.iter().enumerate() {
        let field_line = line_no + 1 + offset;
        let (key, value) = split_key_value(field_line, raw)?;
        match key {
            "title" => cover.title = value.to_owned(),
            "subtitle" => cover.subtitle = value.to_owned(),
            "meta" => cover.meta = value.to_owned(),
            "image" => cover.image = Some(PathBuf::from(value)),
            "site" => cover.site = value.to_owned(),
            other => return Err(err(field_line, format!("unknown cover/closing key `{other}`"))),
        }
    }
    Ok(cover)
}

/// Split `text` into plain/bold runs at `**...**` markers.
fn parse_inline(text: &str) -> Vec<InlineRun> {
    let mut runs = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("**") {
        if start > 0 {
            runs.push(InlineRun::Plain(rest[..start].to_owned()));
        }
        let after = &rest[start + 2..];
        if let Some(end) = after.find("**") {
            runs.push(InlineRun::Bold(after[..end].to_owned()));
            rest = &after[end + 2..];
        } else {
            // Unterminated `**` — tolerated literally rather than a hard
            // parse error (a rare dangling marker is not worth failing
            // the whole document over).
            runs.push(InlineRun::Plain(rest[start..].to_owned()));
            rest = "";
            break;
        }
    }
    if !rest.is_empty() {
        runs.push(InlineRun::Plain(rest.to_owned()));
    }
    if runs.is_empty() {
        runs.push(InlineRun::Plain(String::new()));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_is_optional_and_defaults_to_empty() {
        let doc = parse("---\ntitle: T\n---\n\nBody.\n").expect("front matter without footer parses");
        assert_eq!(doc.front_matter.footer, "");
    }

    #[test]
    fn cover_keys_subtitle_authors_and_date_are_read() {
        let source = "---\ntitle: T\nsubtitle: S: with colon\nauthors: A / B, руководитель / C\ndate: Дата: 18.09.2026\n---\n\nBody.\n";
        let doc = parse(source).expect("cover keys parse");
        assert_eq!(doc.front_matter.subtitle.as_deref(), Some("S: with colon"));
        assert_eq!(doc.front_matter.authors, vec!["A", "B, руководитель", "C"]);
        assert_eq!(doc.front_matter.date.as_deref(), Some("Дата: 18.09.2026"));
    }

    fn inset_parts(body: &str) -> (InsetKind, Vec<InsetPart>) {
        let doc = parse(&format!("---\ntitle: T\n---\n\n{body}\n")).expect("inset fixture parses");
        match doc.sections.into_iter().next().and_then(|s| s.blocks.into_iter().next()) {
            Some(Block::Inset { kind, parts }) => (kind, parts),
            other => panic!("expected a `>>` block, got {other:?}"),
        }
    }

    fn plain(runs: &[InlineRun]) -> String {
        runs.iter().map(|r| match r { InlineRun::Plain(s) | InlineRun::Bold(s) => s.as_str() }).collect()
    }

    #[test]
    fn consecutive_double_chevron_lines_are_one_paragraph_and_a_bare_marker_splits_it() {
        let (kind, parts) = inset_parts(">> первая **строка**\n>> вторая строка\n>>\n>> другой абзац");
        assert_eq!(kind, InsetKind::Revised);
        assert_eq!(parts.len(), 2);
        let InsetPart::Paragraph(first) = &parts[0] else { panic!("first part is a paragraph") };
        assert_eq!(plain(first), "первая строка вторая строка");
        assert!(first.iter().any(|r| matches!(r, InlineRun::Bold(b) if b == "строка")));
        let InsetPart::Paragraph(second) = &parts[1] else { panic!("second part is a paragraph") };
        assert_eq!(plain(second), "другой абзац");
    }

    #[test]
    fn a_triple_chevron_block_is_the_technical_kind_with_the_same_rules() {
        let (kind, parts) = inset_parts(">>> Техника **важна**\n>>> вторая строка\n>>>\n>>> - пункт\n>>> - ещё пункт\n>>>\n>>> | a | b |\n>>> |---|---|\n>>> | 1 | 2 |");
        assert_eq!(kind, InsetKind::Technical);
        assert_eq!(parts.len(), 3);
        assert!(matches!(&parts[0], InsetPart::Paragraph(r) if plain(r) == "Техника важна вторая строка"));
        assert!(matches!(&parts[1], InsetPart::List { ordered: false, items } if items.len() == 2));
        assert!(matches!(&parts[2], InsetPart::Table { rows, .. } if rows.len() == 1));
    }

    #[test]
    fn a_revised_block_may_hold_bullets_numbered_items_and_a_table() {
        let (_, parts) = inset_parts(">> Вступление\n>>\n>> - один\n>> - два\n>>\n>> 1. первый\n>> 2. второй\n>>\n>> | a | b |\n>> |---|--:|\n>> | 1 | 2 |");
        assert_eq!(parts.len(), 4);
        assert!(matches!(&parts[1], InsetPart::List { ordered: false, items } if items.len() == 2));
        assert!(matches!(&parts[2], InsetPart::List { ordered: true, items } if items.len() == 2));
        match &parts[3] {
            InsetPart::Table { header, rows, aligns, .. } => {
                assert_eq!((header.len(), rows.len()), (2, 1));
                assert_eq!(aligns[1], ColumnAlign::Right);
            }
            other => panic!("expected a table, got {other:?}"),
        }
    }

    #[test]
    fn a_blank_line_ends_a_block_and_a_single_chevron_stays_a_callout() {
        let doc = parse("---\ntitle: T\n---\n\n>> раз\n\n>> два\n\n>>> три\n\n> заметка\n").expect("parses");
        let blocks = &doc.sections[0].blocks;
        assert!(matches!(&blocks[0], Block::Inset { kind: InsetKind::Revised, parts } if parts.len() == 1));
        assert!(matches!(&blocks[1], Block::Inset { kind: InsetKind::Revised, parts } if parts.len() == 1));
        assert!(matches!(&blocks[2], Block::Inset { kind: InsetKind::Technical, parts } if parts.len() == 1));
        assert!(matches!(&blocks[3], Block::Callout(_)));
    }

    #[test]
    fn a_malformed_block_is_an_error_with_its_line_number() {
        let no_space = parse("---\ntitle: T\n---\n\n>> ok\n>>bad\n").expect_err("`>>bad` has no space after the marker");
        assert_eq!(no_space.line, 6);
        let heading = parse("---\ntitle: T\n---\n\n>> # заголовок\n").expect_err("a heading is not allowed inside");
        assert_eq!(heading.line, 5);
        let empty = parse("---\ntitle: T\n---\n\n>>\n>>\n").expect_err("a block of bare markers has no text");
        assert_eq!(empty.line, 5);
        let nested = parse("---\ntitle: T\n---\n\n>> >> вложенный\n").expect_err("blocks do not nest");
        assert_eq!(nested.line, 5);
        let bad_list = parse("---\ntitle: T\n---\n\n>> - a\n>> не пункт\n").expect_err("a list part holds only items");
        assert_eq!(bad_list.line, 6);
        let mixed = parse("---\ntitle: T\n---\n\n>> раз\n>>> два\n").expect_err("the two kinds do not mix in one chunk");
        assert_eq!(mixed.line, 6);
        let mixed_up = parse("---\ntitle: T\n---\n\n>>> раз\n>> два\n").expect_err("nor the other way round");
        assert_eq!(mixed_up.line, 6);
    }

    #[test]
    fn a_document_without_cover_keys_has_none_of_them() {
        let doc = parse("---\ntitle: T\nfooter: F\n---\n\nBody.\n").expect("plain front matter parses");
        assert!(doc.front_matter.subtitle.is_none() && doc.front_matter.authors.is_empty() && doc.front_matter.date.is_none());
    }
}
