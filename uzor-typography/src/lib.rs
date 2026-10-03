//! Press CLI: parse a content file, press it (doc or deck), write the PDF,
//! PNG proofs, DOCX, or PPTX.

mod catalog;
mod docx;
mod figures;
mod parse;
mod pptx;
mod preset;
mod press;
mod tokens;

use std::path::PathBuf;
use std::process::ExitCode;

use press::{Format, Press};

/// `--emit pdf|docx|both`, default `pdf` — reproduces every existing
/// caller's exact current behavior (zero new files written unless
/// explicitly asked, same "byte-identical when absent" discipline this
/// crate already follows for `logo`/`badge`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Emit {
    Pdf,
    Docx,
    Both,
    Pptx,
}

impl Emit {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "pdf" => Some(Emit::Pdf),
            "docx" => Some(Emit::Docx),
            "both" => Some(Emit::Both),
            "pptx" => Some(Emit::Pptx),
            _ => None,
        }
    }

    fn wants_pdf(self) -> bool {
        matches!(self, Emit::Pdf | Emit::Both)
    }

    fn wants_docx(self) -> bool {
        matches!(self, Emit::Docx | Emit::Both)
    }

    fn wants_pptx(self) -> bool {
        matches!(self, Emit::Pptx)
    }
}

struct Args {
    input: PathBuf,
    format: Format,
    theme: press::Palette,
    preset: preset::Preset,
    tokens: Option<PathBuf>,
    out: PathBuf,
    name: String,
    emit: Emit,
}

fn press_home() -> Option<PathBuf> {
    std::env::var_os("PRESS_HOME").map(PathBuf::from)
}

fn parse_args() -> Result<Args, String> {
    let mut argv = std::env::args().skip(1);
    let mut input = None;
    let mut format = None;
    let mut theme = None;
    let mut out = None;
    let mut name = None;
    let mut emit = None;
    let mut preset = None;
    let mut tokens = None;
    let home = press_home();

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--format" => {
                let v = argv.next().ok_or("--format needs a value")?;
                format = Some(Format::parse(&v).ok_or_else(|| format!("bad --format value {v:?} (expected doc|deck|report)"))?);
            }
            "--theme" => {
                let v = argv.next().ok_or("--theme needs a value")?;
                theme = Some(catalog::load_theme(&v, home.as_deref()).map_err(|e| {
                    format!("{e} (expected light|dark or a file under PRESS_HOME)")
                })?);
            }
            "--out" => out = Some(PathBuf::from(argv.next().ok_or("--out needs a value")?)),
            "--name" => name = Some(argv.next().ok_or("--name needs a value")?),
            "--preset" => {
                let v = argv.next().ok_or("--preset needs a value")?;
                preset = Some(catalog::load_preset(&v, home.as_deref()).map_err(|e| {
                    format!("{e} (expected column|notice|filing|long|dense or a file under PRESS_HOME)")
                })?);
            }
            "--tokens" => tokens = Some(PathBuf::from(argv.next().ok_or("--tokens needs a value")?)),
            "--emit" => {
                let v = argv.next().ok_or("--emit needs a value")?;
                emit = Some(Emit::parse(&v).ok_or_else(|| format!("bad --emit value {v:?} (expected pdf|docx|both|pptx)"))?);
            }
            other if !other.starts_with("--") && input.is_none() => input = Some(PathBuf::from(other)),
            other => return Err(format!("unrecognized argument {other:?}")),
        }
    }

    Ok(Args {
        input: input.ok_or("missing <input.md> argument")?,
        format: format.ok_or("missing --format doc|deck|report")?,
        theme: theme.ok_or("missing --theme light|dark or a file under PRESS_HOME")?,
        preset: preset.unwrap_or_else(preset::Preset::column),
        tokens,
        out: out.ok_or("missing --out <dir>")?,
        name: name.ok_or("missing --name <base>")?,
        emit: emit.unwrap_or(Emit::Pdf),
    })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    // `--tokens` tunes the sheet AFTER the preset (or its default) resolves.
    let mut preset = args.preset;
    if let Some(path) = &args.tokens {
        tokens::Tokens::load(path)?.apply(&mut preset);
    }

    if args.format == Format::Deck && args.emit.wants_docx() {
        return Err("--format deck does not support --emit docx (deck stays PDF-only)".to_owned());
    }
    if args.emit.wants_pptx() && args.format != Format::Deck {
        return Err("--emit pptx requires --format deck".to_owned());
    }

    let source = std::fs::read_to_string(&args.input).map_err(|e| format!("reading {}: {e}", args.input.display()))?;
    let document = parse::parse(&source).map_err(|e| format!("{}: {e}", args.input.display()))?;

    // Relative paths inside the content file resolve against the content
    // file's own directory.
    let base_dir = args.input.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    if args.format == Format::Deck && preset.slides.is_none() {
        return Err("--format deck is not defined for this preset".to_owned());
    }
    let press = Press::new(args.format, args.theme, preset, base_dir);

    std::fs::create_dir_all(&args.out).map_err(|e| format!("creating {}: {e}", args.out.display()))?;

    let mut written = Vec::new();

    if args.emit.wants_pdf() {
        let output = press.build(&document).map_err(|e| e.to_string())?;

        let pdf_bytes = press.to_pdf(&output, &document.front_matter);
        let pdf_path = args.out.join(format!("{}.pdf", args.name));
        std::fs::write(&pdf_path, &pdf_bytes).map_err(|e| format!("writing {}: {e}", pdf_path.display()))?;

        println!("pages: {}", output.pages.len());
        for warning in &output.warnings {
            println!("{warning}");
        }
        written.push(pdf_path);

        for (i, page) in output.pages.iter().enumerate() {
            let png_bytes = press.render_page_png(page).map_err(|e| format!("rendering page {} PNG: {e}", i + 1))?;
            let png_path = args.out.join(format!("{}-p{:02}.png", args.name, i + 1));
            std::fs::write(&png_path, &png_bytes).map_err(|e| format!("writing {}: {e}", png_path.display()))?;
            written.push(png_path);
        }
    }

    if args.emit.wants_pptx() {
        let output = press.build(&document).map_err(|e| e.to_string())?;
        let mut pngs = Vec::with_capacity(output.pages.len());
        for (i, page) in output.pages.iter().enumerate() {
            pngs.push(press.render_page_png(page).map_err(|e| format!("rendering slide {} PNG: {e}", i + 1))?);
        }
        let (w, h) = args.format.page_size();
        let pptx_bytes = pptx::build_pptx(&pngs, w, h, &document.front_matter.title);
        let pptx_path = args.out.join(format!("{}.pptx", args.name));
        std::fs::write(&pptx_path, &pptx_bytes).map_err(|e| format!("writing {}: {e}", pptx_path.display()))?;
        written.push(pptx_path);
    }

    if args.emit.wants_docx() {
        let docx_bytes = docx::build_docx(&document, &press).map_err(|e| e.to_string())?;
        let docx_path = args.out.join(format!("{}.docx", args.name));
        std::fs::write(&docx_path, &docx_bytes).map_err(|e| format!("writing {}: {e}", docx_path.display()))?;
        written.push(docx_path);
    }

    for path in &written {
        println!("wrote {}", path.display());
    }

    Ok(())
}

/// Parse argv and press the named file. `PRESS_HOME`, when set, is the
/// directory of theme and preset files.
pub fn cli_main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
