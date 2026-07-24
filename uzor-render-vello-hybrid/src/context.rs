//! `VelloHybridRenderContext` — hybrid CPU/GPU `RenderContext` implementation.
//!
//! Uses `vello_hybrid::Scene` for geometry encoding (CPU side) and a wgpu
//! `Renderer` for the final rasterization (GPU side).
//!
//! ## Architecture
//!
//! The `vello_hybrid` backend is a two-phase pipeline:
//!
//! 1. **CPU phase**: Drawing calls (`fill_rect`, `stroke_path`, …) are encoded
//!    into a `vello_hybrid::Scene` using the sparse-strips algorithm.
//!
//! 2. **GPU phase**: The scene is submitted to `vello_hybrid::Renderer::render()`
//!    which uploads strip data to the GPU and runs a fragment shader to produce
//!    the final image.
//!
//! ## wgpu conflict note
//!
//! `vello_hybrid 0.0.6` depends on `wgpu 27.x`.  This crate MUST NOT be
//! combined in the same binary with `vello 0.6` (which uses `wgpu 0.20`).
//!
//! ## Frame lifecycle
//!
//! ```rust,ignore
//! // Initialise (once):
//! let renderer = vello_hybrid::Renderer::new(&device, &render_target_config);
//! let mut ctx = VelloHybridRenderContext::new(1.0);
//!
//! // Each frame:
//! ctx.begin_frame(width, height);
//!
//! ctx.set_fill_color("#202020");
//! ctx.fill_rect(0.0, 0.0, width as f64, height as f64);
//!
//! let mut encoder = device.create_command_encoder(&Default::default());
//! ctx.render(&mut renderer, &device, &queue, &mut encoder, &view);
//! queue.submit([encoder.finish()]);
//! ```

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------
// vello_common re-exports — kurbo geometry, peniko paints, glyph types
// ---------------------------------------------------------------------------

// vello_common 0.0.9 moved glyph types into the glifo crate.
use glifo::Glyph;
use vello_common::kurbo::{self, Affine, BezPath, Cap, Join, Rect, Shape, Stroke};
use vello_common::peniko::{Blob, Compose, Fill, FontData, Mix};

// ---------------------------------------------------------------------------
// skrifa — font metrics for text measurement
// ---------------------------------------------------------------------------

use skrifa::{
    MetadataProvider,
    raw::{FileRef, FontRef},
};

// ---------------------------------------------------------------------------
// vello_hybrid — Scene (geometry encoder) and GPU Renderer
// ---------------------------------------------------------------------------

use vello_hybrid::{
    RenderSize, RenderTargetConfig, Renderer, Scene, TextureBindings, TextureId,
};

// ---------------------------------------------------------------------------
// wgpu — GPU device/queue/encoder/view handles
// ---------------------------------------------------------------------------

use wgpu::{CommandEncoder, Device, Queue, TextureView};

// ---------------------------------------------------------------------------
// uzor-render trait
// ---------------------------------------------------------------------------

use uzor::fonts::{self, FontFamily};
use uzor::render::{
    BatchPainter, BlendMode as UzorBlendMode, CircleBatch,
    Effects, GradientPainter, LineSegment, Masking, Painter,
    OffscreenTarget, OffscreenTargetDesc, OffscreenTargetId,
    RenderContext as UzorRenderContext, RenderContextExt, ShapeHelpers,
    TextBounds, TextMetrics, TextRenderer, TextAlign, TextBaseline,
};
use uzor::core::types::Rect as UzorRect;

// ---------------------------------------------------------------------------
// Cached vello_hybrid FontData (one per process)
// ---------------------------------------------------------------------------

static FONT_REGULAR:     OnceLock<FontData> = OnceLock::new();
static FONT_BOLD:        OnceLock<FontData> = OnceLock::new();
static FONT_ITALIC:      OnceLock<FontData> = OnceLock::new();
static FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

static FONT_PT_ROOT_UI:      OnceLock<FontData> = OnceLock::new();
static FONT_JB_MONO_REGULAR: OnceLock<FontData> = OnceLock::new();
static FONT_JB_MONO_BOLD:    OnceLock<FontData> = OnceLock::new();

static FONT_FALLBACK_NERD_FONT:   OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_SYMBOLS2:    OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_COLOR_EMOJI: OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_EMOJI:       OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_CJK_SC:      OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_ARABIC:      OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_DEVANAGARI:  OnceLock<FontData> = OnceLock::new();

fn get_font(family: FontFamily, bold: bool, italic: bool) -> &'static FontData {
    match family {
        FontFamily::PtRootUi => FONT_PT_ROOT_UI
            .get_or_init(|| make_font(fonts::font_bytes(family, bold, italic))),
        FontFamily::JetBrainsMono => {
            let _ = italic;
            if bold {
                FONT_JB_MONO_BOLD
                    .get_or_init(|| make_font(fonts::font_bytes(family, true, false)))
            } else {
                FONT_JB_MONO_REGULAR
                    .get_or_init(|| make_font(fonts::font_bytes(family, false, false)))
            }
        }
        FontFamily::Roboto => match (bold, italic) {
            (true,  true)  => FONT_BOLD_ITALIC
                .get_or_init(|| make_font(fonts::font_bytes(family, true, true))),
            (true,  false) => FONT_BOLD
                .get_or_init(|| make_font(fonts::font_bytes(family, true, false))),
            (false, true)  => FONT_ITALIC
                .get_or_init(|| make_font(fonts::font_bytes(family, false, true))),
            (false, false) => FONT_REGULAR
                .get_or_init(|| make_font(fonts::font_bytes(family, false, false))),
        },
    }
}

fn get_fallback_fonts() -> &'static [FontData] {
    static FALLBACK_LIST: OnceLock<Vec<FontData>> = OnceLock::new();
    FALLBACK_LIST.get_or_init(|| {
        let nf   = FONT_FALLBACK_NERD_FONT.get_or_init(|| make_font(fonts::SYMBOLS_NERD_FONT_MONO));
        let s2   = FONT_FALLBACK_SYMBOLS2.get_or_init(|| make_font(fonts::NOTO_SANS_SYMBOLS2));
        let cjk  = FONT_FALLBACK_CJK_SC.get_or_init(|| make_font(fonts::NOTO_SANS_CJK_SC));
        let ar   = FONT_FALLBACK_ARABIC.get_or_init(|| make_font(fonts::NOTO_SANS_ARABIC));
        let deva = FONT_FALLBACK_DEVANAGARI.get_or_init(|| make_font(fonts::NOTO_SANS_DEVANAGARI));
        let cv   = FONT_FALLBACK_COLOR_EMOJI.get_or_init(|| make_font(fonts::NOTO_COLOR_EMOJI));
        let em   = FONT_FALLBACK_EMOJI.get_or_init(|| make_font(fonts::NOTO_EMOJI));
        // Order: [0]=NerdFont, [1]=Symbols2, [2]=CjkSc, [3]=Arabic, [4]=Devanagari,
        //        [5]=NotoColorEmoji, [6]=NotoEmoji
        // Text script fonts (CJK/Arabic/Devanagari) placed BEFORE emoji so ordinary
        // script codepoints resolve to text outlines rather than emoji glyphs.
        vec![
            nf.clone(), s2.clone(), cjk.clone(), ar.clone(), deva.clone(),
            cv.clone(), em.clone(),
        ]
    })
}

fn make_font(bytes: &'static [u8]) -> FontData {
    FontData::new(Blob::new(Arc::new(bytes) as Arc<dyn AsRef<[u8]> + Send + Sync>), 0)
}

fn to_font_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(f)         => Some(f),
        FileRef::Collection(col) => col.get(font.index).ok(),
    }
}

// ---------------------------------------------------------------------------
// Resolved glyph with fallback font tracking
// ---------------------------------------------------------------------------

struct ResolvedGlyph {
    /// None = primary font; Some(i) = fallback index i.
    font_index: Option<usize>,
    glyph_id: u32,
    x: f32,
    advance: f32,
}

fn resolve_glyphs_with_fallback(
    text: &str,
    primary_ref: &FontRef<'_>,
    font_size: f32,
) -> Vec<ResolvedGlyph> {
    let size = skrifa::instance::Size::new(font_size);
    let var_loc = skrifa::instance::LocationRef::default();
    let primary_charmap = primary_ref.charmap();
    let primary_metrics = primary_ref.glyph_metrics(size, var_loc);
    let fallbacks = get_fallback_fonts();

    let mut pen_x = 0.0f32;
    let mut result = Vec::with_capacity(text.len());

    for ch in text.chars() {
        let primary_gid = primary_charmap.map(ch).unwrap_or_default();
        if primary_gid != skrifa::GlyphId::new(0) {
            let adv = primary_metrics.advance_width(primary_gid).unwrap_or_default();
            result.push(ResolvedGlyph {
                font_index: None,
                glyph_id: primary_gid.to_u32(),
                x: pen_x,
                advance: adv,
            });
            pen_x += adv;
        } else {
            let mut found_index = None;
            let mut found_gid = primary_gid;
            let mut found_adv = primary_metrics.advance_width(primary_gid).unwrap_or_default();

            for (idx, fb_font) in fallbacks.iter().enumerate() {
                if let Some(fb_ref) = to_font_ref(fb_font) {
                    let fb_gid = fb_ref.charmap().map(ch).unwrap_or_default();
                    if fb_gid != skrifa::GlyphId::new(0) {
                        let fb_metrics = fb_ref.glyph_metrics(size, var_loc);
                        found_adv = fb_metrics.advance_width(fb_gid).unwrap_or_default();
                        found_gid = fb_gid;
                        found_index = Some(idx);
                        break;
                    }
                }
            }

            result.push(ResolvedGlyph {
                font_index: found_index,
                glyph_id: found_gid.to_u32(),
                x: pen_x,
                advance: found_adv,
            });
            pen_x += found_adv;
        }
    }

    result
}

fn resolved_total_width(glyphs: &[ResolvedGlyph]) -> f32 {
    glyphs.last().map_or(0.0, |g| g.x + g.advance)
}

// ---------------------------------------------------------------------------
// Color type and parsing (CSS → peniko / vello_common color)
// ---------------------------------------------------------------------------

/// sRGB color with alpha, backed by vello_common's color module.
type Color = vello_common::peniko::color::AlphaColor<vello_common::peniko::color::Srgb>;

fn parse_color(s: &str) -> Color {
    let (r, g, b, a) = uzor::render::parse_color(s);
    Color::from_rgba8(r, g, b, a)
}

// ---------------------------------------------------------------------------
// CSS font parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct FontInfo {
    size:   f64,
    bold:   bool,
    italic: bool,
    family: FontFamily,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self { size: 12.0, bold: false, italic: false, family: FontFamily::Roboto }
    }
}

fn parse_css_font(font_str: &str) -> FontInfo {
    let parsed = fonts::parse_css_font(font_str);
    FontInfo {
        size:   parsed.size as f64,
        bold:   parsed.bold,
        italic: parsed.italic,
        family: parsed.family,
    }
}

// ---------------------------------------------------------------------------
// Text metrics via skrifa
// ---------------------------------------------------------------------------

fn measure_text_width(text: &str, font_info: &FontInfo) -> f64 {
    let font = get_font(font_info.family, font_info.bold, font_info.italic);
    let Some(font_ref) = to_font_ref(font) else {
        return text.len() as f64 * font_info.size * 0.6;
    };
    let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_info.size as f32);
    resolved_total_width(&glyphs) as f64
}

// ---------------------------------------------------------------------------
// Save/restore state
// ---------------------------------------------------------------------------

/// Drop shadow state for the hybrid backend (offset-copy approximation).
#[derive(Clone)]
struct ShadowState {
    dx:    f64,
    dy:    f64,
    color: Color,
}

#[derive(Clone)]
struct SavedState {
    transform:     Affine,
    stroke_color:  Color,
    stroke_width:  f64,
    fill_color:    Color,
    line_cap:      Cap,
    line_join:     Join,
    global_alpha:  f64,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    /// Whether `clip()` was called at this save level (so we pop it on restore).
    has_clip:      bool,
    /// Blend mode at this save level.
    blend_mode:    UzorBlendMode,
}

// ---------------------------------------------------------------------------
// VelloHybridRenderContext
// ---------------------------------------------------------------------------

/// Hybrid CPU/GPU rendering context backed by `vello_hybrid`.
///
/// Geometry is encoded on the CPU using the sparse-strips algorithm; the GPU
/// then executes a lightweight fragment shader to produce the final pixels.
/// This gives better GPU compatibility than the full `vello` compute backend
/// while retaining most of its visual quality.
///
/// ## wgpu version
///
/// This crate requires `wgpu 27.x` (via `vello_hybrid 0.0.6`).  It CANNOT
/// coexist in the same binary with `uzor-render-vello-gpu` which pulls in
/// `wgpu 0.20` through `vello 0.6`.
///
/// ## Frame lifecycle
///
/// ```rust,ignore
/// let mut ctx = VelloHybridRenderContext::new(1.0);
/// ctx.begin_frame(width, height);
///
/// ctx.set_fill_color("#1e1e1e");
/// ctx.fill_rect(0.0, 0.0, width as f64, height as f64);
///
/// let mut encoder = device.create_command_encoder(&Default::default());
/// ctx.render(&mut renderer, &device, &queue, &mut encoder, &view)
///     .expect("hybrid render failed");
/// queue.submit([encoder.finish()]);
/// ```
pub struct VelloHybridRenderContext {
    /// The vello_hybrid scene — rebuilt each frame.
    scene:         Option<Scene>,
    /// vello_hybrid 0.0.9 split per-frame resources (atlas / glyph cache) out of
    /// the renderer; we own one set so glyph_run + render() can borrow it.
    resources:     vello_hybrid::Resources,
    width:         u32,
    height:        u32,
    dpr:           f64,

    // Drawing state mirroring Canvas2D semantics
    transform:     Affine,
    stroke_color:  Color,
    stroke_width:  f64,
    fill_color:    Color,
    line_cap:      Cap,
    line_join:     Join,
    global_alpha:  f64,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,

    /// Current Canvas2D-style path being built.
    path:          Option<BezPath>,

    /// Whether a clip path is active at the current innermost save level.
    clip_active:   bool,

    /// Save/restore stack.
    state_stack:   Vec<SavedState>,

    /// M6-P1: Drop shadow (approximated as offset copy).
    shadow:        Option<ShadowState>,
    /// M6-P3: Blend mode.
    blend_mode:    UzorBlendMode,

    // Offscreen-target support — capability-gated on `set_gpu_handles`
    // (see the `RenderContext` impl below for the full rationale). Both
    // handles are populated together by `set_gpu_handles`; `None` means
    // "backend not yet wired to a GPU device", the honest default.
    gpu_device: Option<Arc<Device>>,
    gpu_queue:  Option<Arc<Queue>>,

    // Rasterised offscreen-target cache — each entry owns the `wgpu::Texture`
    // it was rendered into plus the `TextureView` bound for replay via
    // `Scene::draw_texture_rects`.
    offscreen_targets: HashMap<OffscreenTargetId, CachedTarget>,
    // Monotonic counter for allocating new `OffscreenTargetId`s (and,
    // paired 1:1, the `vello_hybrid::TextureId` bound for that target).
    next_offscreen_id: u64,
    // Stack of saved recording state, swapped out by
    // `push_offscreen_target`; `pop_offscreen_target` restores the top
    // entry and rasterises the just-finished scene into the target's texture.
    offscreen_stack: Vec<SavedRecording>,
}

/// Recording state swapped out while painting into an offscreen target.
struct SavedRecording {
    id:     OffscreenTargetId,
    scene:  Option<Scene>,
    width:  u32,
    height: u32,
    // Outer per-frame drawing state — the recording paints at its own
    // local origin with fresh state; the outer walk continues afterwards
    // with ITS state intact (without the restore, one mid-walk recording
    // clobbers the caller's translate/clip/save stack for the rest of
    // the frame).
    transform:   Affine,
    clip_active: bool,
    state_stack: Vec<SavedState>,
    path:        Option<BezPath>,
}

/// Rasterised offscreen-target content — a GPU texture rendered once at
/// `pop_offscreen_target` time and replayed via `draw_texture_rects` on
/// every subsequent `draw_cached_target` call, no re-rasterisation needed.
///
/// `wgpu::TextureView` (wgpu 29) already holds a `Clone`-able internal
/// reference to the `Texture` it was created from (`TextureView::texture()`)
/// — a separate `texture` field here would be a redundant, dead-weight
/// keep-alive handle, not extra safety.
struct CachedTarget {
    view:       TextureView,
    texture_id: TextureId,
    width:      u32,
    height:     u32,
}

impl VelloHybridRenderContext {
    /// Create a new context.
    ///
    /// The underlying `vello_hybrid::Scene` is lazily created on the first
    /// call to [`begin_frame`](Self::begin_frame).
    ///
    /// `dpr` — device pixel ratio (used by the `RenderContext::dpr` method).
    pub fn new(dpr: f64) -> Self {
        Self {
            scene:         None,
            resources:     vello_hybrid::Resources::new(),
            width:         0,
            height:        0,
            dpr,
            transform:     Affine::IDENTITY,
            stroke_color:  Color::from_rgba8(255, 255, 255, 255),
            stroke_width:  1.0,
            fill_color:    Color::from_rgba8(0, 0, 0, 0),
            line_cap:      Cap::Butt,
            line_join:     Join::Miter,
            global_alpha:  1.0,
            font_info:     FontInfo::default(),
            text_align:    TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            path:          None,
            clip_active:   false,
            state_stack:   Vec::new(),
            shadow:        None,
            blend_mode:    UzorBlendMode::Normal,
            gpu_device:    None,
            gpu_queue:     None,
            offscreen_targets: HashMap::new(),
            next_offscreen_id: 0,
            offscreen_stack:   Vec::new(),
        }
    }

    /// Provide the GPU device/queue handles this backend needs to actually
    /// rasterise offscreen targets.
    ///
    /// `vello_hybrid::Scene` has no CPU-only raster path and no
    /// `vello::Scene::append`-equivalent for merging encodings (see the
    /// `RenderContext` impl doc comment below) — the only way to turn a
    /// recorded `Scene` into replayable content is `Renderer::render()`,
    /// which needs a `Device`/`Queue`. This context is constructed
    /// GPU-handle-free (`new(dpr: f64)`) on purpose so it stays usable
    /// before a device exists; call this setter once the caller has one
    /// (same "declare it via a setter, not the constructor" precedent as
    /// `set_blur_image` on the vello-gpu backend).
    ///
    /// `supports_offscreen_targets()` reports `true` only once both
    /// handles are present — a capability that is conditionally true is
    /// honest; unconditionally claiming `true` before a device exists
    /// would not be.
    pub fn set_gpu_handles(&mut self, device: Arc<Device>, queue: Arc<Queue>) {
        self.gpu_device = Some(device);
        self.gpu_queue  = Some(queue);
    }

    /// Begin a new frame.
    ///
    /// Re-creates the `vello_hybrid::Scene` when `width` or `height` changes;
    /// otherwise calls `reset()` to clear draw commands without reallocation.
    ///
    /// Also resets per-frame drawing state (transform, clip stack, save stack).
    pub fn begin_frame(&mut self, width: u32, height: u32) {
        let w16 = width.min(u16::MAX as u32) as u16;
        let h16 = height.min(u16::MAX as u32) as u16;

        let needs_new = self.scene.is_none()
            || self.width  != width
            || self.height != height;

        if needs_new {
            self.scene  = Some(Scene::new(w16, h16));
            self.width  = width;
            self.height = height;
        } else if let Some(ref mut s) = self.scene {
            s.reset();
        }

        // Reset per-frame drawing state
        self.transform   = Affine::IDENTITY;
        self.clip_active = false;
        self.state_stack.clear();
        self.path        = None;
    }

    /// Submit the encoded scene to the GPU for rasterization.
    ///
    /// Must be called after all drawing calls for the frame are complete.
    /// `encoder` should be submitted to the `queue` by the caller.
    ///
    /// # Errors
    ///
    /// Returns a `vello_hybrid::RenderError` if the GPU upload or draw fails.
    pub fn render(
        &mut self,
        renderer: &mut vello_hybrid::Renderer,
        device:   &Device,
        queue:    &Queue,
        encoder:  &mut CommandEncoder,
        view:     &TextureView,
    ) -> Result<(), vello_hybrid::RenderError> {
        let Some(ref scene) = self.scene else {
            return Ok(());
        };
        // vello_hybrid 0.0.9: render() takes &mut Resources + &TextureBindings.
        // Bind every live cached offscreen target so any `draw_texture_rects`
        // call this scene emitted (via `draw_cached_target`) resolves —
        // `Renderer::render` errors with `MissingTextureBinding` otherwise.
        // Built before the `&mut self.resources` borrow below (both are
        // disjoint fields, but the local avoids borrowing `self` twice at
        // the call site).
        let bindings = self.cached_target_bindings();
        renderer.render(
            scene,
            &mut self.resources,
            device,
            queue,
            encoder,
            &RenderSize {
                width:  self.width,
                height: self.height,
            },
            view,
            &bindings,
        )
    }

    /// Build the `TextureBindings` set for every currently-live cached
    /// offscreen target, keyed by the `TextureId` `draw_cached_target`
    /// bound it under. Used both by `render()` above (so a root-scene
    /// draw referencing a cached target resolves) and internally by
    /// `pop_offscreen_target`/`draw_cached_target`'s own bookkeeping.
    fn cached_target_bindings(&self) -> TextureBindings {
        let mut bindings = TextureBindings::new();
        for target in self.offscreen_targets.values() {
            bindings.insert(target.texture_id, target.view.clone());
        }
        bindings
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn current_stroke(&self) -> Stroke {
        Stroke {
            width:        self.stroke_width,
            join:         self.line_join,
            miter_limit:  4.0,
            start_cap:    self.line_cap,
            end_cap:      self.line_cap,
            dash_pattern: Default::default(),
            dash_offset:  0.0,
        }
    }

    fn effective_fill_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.fill_color.with_alpha(self.global_alpha as f32)
        } else {
            self.fill_color
        }
    }

    fn effective_stroke_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.stroke_color.with_alpha(self.global_alpha as f32)
        } else {
            self.stroke_color
        }
    }

    fn apply_fill_paint(&mut self) {
        let color = self.effective_fill_color();
        let blend = Self::blend_to_hybrid(self.blend_mode);
        if let Some(ref mut s) = self.scene {
            s.set_blend_mode(blend);
            s.set_paint(color);
        }
    }

    fn apply_stroke_paint(&mut self) {
        let color = self.effective_stroke_color();
        let blend = Self::blend_to_hybrid(self.blend_mode);
        if let Some(ref mut s) = self.scene {
            s.set_blend_mode(blend);
            s.set_paint(color);
        }
    }

    fn push_save_state(&mut self, has_clip: bool) {
        self.state_stack.push(SavedState {
            transform:     self.transform,
            stroke_color:  self.stroke_color,
            stroke_width:  self.stroke_width,
            fill_color:    self.fill_color,
            line_cap:      self.line_cap,
            line_join:     self.line_join,
            global_alpha:  self.global_alpha,
            font_info:     self.font_info.clone(),
            text_align:    self.text_align,
            text_baseline: self.text_baseline,
            has_clip,
            blend_mode:    self.blend_mode,
        });
    }

    fn pop_save_state(&mut self) -> Option<SavedState> {
        let s = self.state_stack.pop()?;
        self.transform     = s.transform;
        self.stroke_color  = s.stroke_color;
        self.stroke_width  = s.stroke_width;
        self.fill_color    = s.fill_color;
        self.line_cap      = s.line_cap;
        self.line_join     = s.line_join;
        self.global_alpha  = s.global_alpha;
        self.font_info     = s.font_info.clone();
        self.text_align    = s.text_align;
        self.text_baseline = s.text_baseline;
        self.blend_mode    = s.blend_mode;
        Some(s)
    }

    /// Map a `UzorBlendMode` to a `vello_common::peniko::BlendMode`.
    fn blend_to_hybrid(mode: UzorBlendMode) -> vello_common::peniko::BlendMode {
        match mode {
            UzorBlendMode::Normal     => Mix::Normal.into(),
            UzorBlendMode::Multiply   => Mix::Multiply.into(),
            UzorBlendMode::Screen     => Mix::Screen.into(),
            UzorBlendMode::Overlay    => Mix::Overlay.into(),
            UzorBlendMode::Darken     => Mix::Darken.into(),
            UzorBlendMode::Lighten    => Mix::Lighten.into(),
            UzorBlendMode::ColorDodge => Mix::ColorDodge.into(),
            UzorBlendMode::ColorBurn  => Mix::ColorBurn.into(),
            UzorBlendMode::HardLight  => Mix::HardLight.into(),
            UzorBlendMode::SoftLight  => Mix::SoftLight.into(),
            UzorBlendMode::Difference => Mix::Difference.into(),
            UzorBlendMode::Exclusion  => Mix::Exclusion.into(),
            UzorBlendMode::Plus       => Compose::Plus.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

impl Painter for VelloHybridRenderContext {
    fn save(&mut self) {
        let has_clip = self.clip_active;
        self.push_save_state(has_clip);
        self.clip_active = false;
    }

    fn restore(&mut self) {
        if let Some(saved) = self.pop_save_state() {
            if self.clip_active {
                if let Some(ref mut s) = self.scene {
                    s.pop_clip_path();
                }
            }
            self.clip_active = saved.has_clip;
            let transform = self.transform;
            if let Some(ref mut s) = self.scene {
                s.set_transform(transform);
            }
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_translate((x, y).into());
    }

    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.then_rotate(angle);
    }

    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_scale_non_uniform(x, y);
    }

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_color(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

    fn set_stroke_color(&mut self, color: &str) {
        self.stroke_color = parse_color(color);
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width;
        let stroke = self.current_stroke();
        if let Some(ref mut s) = self.scene { s.set_stroke(stroke); }
    }

    fn set_line_dash(&mut self, _pattern: &[f64]) {
        // Dash support deferred — calls silently accepted for API compat.
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap { "round" => Cap::Round, "square" => Cap::Square, _ => Cap::Butt };
        let stroke = self.current_stroke();
        if let Some(ref mut s) = self.scene { s.set_stroke(stroke); }
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join { "round" => Join::Round, "bevel" => Join::Bevel, _ => Join::Miter };
        let stroke = self.current_stroke();
        if let Some(ref mut s) = self.scene { s.set_stroke(stroke); }
    }

    fn begin_path(&mut self) { self.path = Some(BezPath::new()); }

    fn move_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut p) = self.path { p.move_to(kurbo::Point::new(x, y)); }
    }

    fn line_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut p) = self.path { p.line_to(kurbo::Point::new(x, y)); }
    }

    fn close_path(&mut self) {
        if let Some(ref mut p) = self.path { p.close_path(); }
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if let Some(ref mut p) = self.path {
            p.move_to(kurbo::Point::new(x, y));
            p.line_to(kurbo::Point::new(x + w, y));
            p.line_to(kurbo::Point::new(x + w, y + h));
            p.line_to(kurbo::Point::new(x, y + h));
            p.close_path();
        }
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        if let Some(ref mut p) = self.path {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy), kurbo::Vec2::new(radius, radius),
                start_angle, end_angle - start_angle, 0.0,
            );
            let path_has_elements = !p.elements().is_empty();
            let mut is_first = true;
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(pt) => {
                    if is_first && path_has_elements { p.line_to(pt); } else { p.move_to(pt); }
                    is_first = false;
                }
                kurbo::PathEl::LineTo(pt) => { p.line_to(pt); is_first = false; }
                kurbo::PathEl::QuadTo(c, pt) => { p.quad_to(c, pt); is_first = false; }
                kurbo::PathEl::CurveTo(c1, c2, pt) => { p.curve_to(c1, c2, pt); is_first = false; }
                kurbo::PathEl::ClosePath => p.close_path(),
            });
        }
    }

    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, _rotation: f64, start: f64, end: f64) {
        if let Some(ref mut p) = self.path {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy), kurbo::Vec2::new(rx, ry),
                start, end - start, 0.0,
            );
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(pt)          => p.move_to(pt),
                kurbo::PathEl::LineTo(pt)          => p.line_to(pt),
                kurbo::PathEl::QuadTo(c, pt)       => p.quad_to(c, pt),
                kurbo::PathEl::CurveTo(c1, c2, pt) => p.curve_to(c1, c2, pt),
                kurbo::PathEl::ClosePath           => p.close_path(),
            });
        }
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.quad_to(kurbo::Point::new(cpx, cpy), kurbo::Point::new(x, y));
        }
    }

    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.curve_to(kurbo::Point::new(cp1x, cp1y), kurbo::Point::new(cp2x, cp2y), kurbo::Point::new(x, y));
        }
    }

    fn stroke(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform  = self.transform;
        let stroke_val = self.current_stroke();
        if let Some(ref sh) = self.shadow.clone() {
            let shadow_transform = transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
            if let Some(ref mut s) = self.scene {
                s.set_blend_mode(vello_common::peniko::BlendMode::default());
                s.set_transform(shadow_transform);
                s.set_paint(sh.color);
                s.set_stroke(stroke_val.clone());
                s.stroke_path(&path);
            }
        }
        self.apply_stroke_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_stroke(stroke_val);
            s.stroke_path(&path);
        }
    }

    fn fill(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform  = self.transform;
        if let Some(ref sh) = self.shadow.clone() {
            let shadow_transform = transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
            if let Some(ref mut s) = self.scene {
                s.set_blend_mode(vello_common::peniko::BlendMode::default());
                s.set_transform(shadow_transform);
                s.set_fill_rule(Fill::NonZero);
                s.set_paint(sh.color);
                s.fill_path(&path);
            }
        }
        self.apply_fill_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_fill_rule(Fill::NonZero);
            s.fill_path(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl TextRenderer for VelloHybridRenderContext {
    fn set_font(&mut self, font: &str) { self.font_info = parse_css_font(font); }
    fn set_text_align(&mut self, align: TextAlign) { self.text_align = align; }
    fn set_text_baseline(&mut self, baseline: TextBaseline) { self.text_baseline = baseline; }

    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() { return; }

        let font_info    = self.font_info.clone();
        let primary_font = get_font(font_info.family, font_info.bold, font_info.italic);
        let font_size    = font_info.size as f32;

        let text_width = measure_text_width(text, &font_info);
        let x_off = match self.text_align {
            TextAlign::Center => -text_width / 2.0,
            TextAlign::Right  => -text_width,
            _                 => 0.0,
        };
        // Exhaustive match (no `_` wildcard) — `TextBaseline::Alphabetic`
        // used to silently fall into a wildcard arm that applied
        // `Middle`'s offset, i.e. an extra `size * 0.35` shift downward.
        // "Alphabetic" means the caller's own `y` coordinate ALREADY IS
        // the baseline (the standard Canvas2D/CSS definition) — the
        // correct offset is `0.0`, matching `Bottom`'s own value, not
        // `Middle`'s. `uzor-render-vello-cpu` and `uzor-render-urx` had
        // the IDENTICAL bug (fixed 2026-07-24 during the typography
        // calibration fixture pass) — this crate's own copy was found
        // and closed the same way in the Wave 7 tail tech-debt sweep.
        let y_off = match self.text_baseline {
            TextBaseline::Top        => font_info.size * 0.8,
            TextBaseline::Middle     => font_info.size * 0.35,
            TextBaseline::Bottom     => 0.0,
            TextBaseline::Alphabetic => 0.0,
        };

        let Some(primary_ref) = to_font_ref(primary_font) else { return };
        let resolved = resolve_glyphs_with_fallback(text, &primary_ref, font_size);
        let fallbacks = get_fallback_fonts();

        let text_transform = Affine::translate((x + x_off, y + y_off));
        let combined       = self.transform * text_transform;

        // Fallback index 5 = NotoColorEmoji in chain:
        // [0]=NerdFont, [1]=Symbols2, [2]=CjkSc, [3]=Arabic, [4]=Devanagari,
        // [5]=NotoColorEmoji, [6]=NotoEmoji
        const COLOR_EMOJI_FALLBACK_IDX: usize = 5;
        let fill_color = self.effective_fill_color();
        let white = Color::from_rgba8(255, 255, 255, 255);

        // vello_hybrid 0.0.9: Scene::glyph_run now takes &mut Resources first.
        let resources = &mut self.resources;
        if let Some(ref mut s) = self.scene {
            s.set_transform(combined);
            let mut i = 0;
            while i < resolved.len() {
                let run_font_index = resolved[i].font_index;
                let run_start = i;
                while i < resolved.len() && resolved[i].font_index == run_font_index { i += 1; }
                let run = &resolved[run_start..i];
                let is_color_emoji = run_font_index == Some(COLOR_EMOJI_FALLBACK_IDX);
                let font = match run_font_index {
                    None => primary_font,
                    Some(idx) if idx < fallbacks.len() => &fallbacks[idx],
                    _ => primary_font,
                };
                s.set_paint(if is_color_emoji { white } else { fill_color });
                let glyphs = run.iter().map(|g| Glyph { id: g.glyph_id, x: g.x, y: 0.0 });
                // Aligned to `uzor-render-vello-gpu`'s own convention
                // (`.hint(!is_color_emoji)`) — same family, same fix as
                // `uzor-render-vello-cpu`'s identical `.hint(false)`
                // found in the Wave 7 tail tech-debt sweep (2026-07-24).
                s.glyph_run(resources, font).font_size(font_size).hint(!is_color_emoji).normalized_coords(&[]).fill_glyphs(glyphs);
            }
            s.set_transform(self.transform);
        }
    }

    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {
        // Deferred — callers can overlay stroke on top of fill_text if needed.
    }
}

// ---------------------------------------------------------------------------
// TextMetrics
// ---------------------------------------------------------------------------

impl TextMetrics for VelloHybridRenderContext {
    fn measure_text(&self, text: &str) -> f64 {
        measure_text_width(text, &self.font_info)
    }

    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        let info = parse_css_font(font);
        let font_size = info.size as f32;
        let primary_font = get_font(info.family, info.bold, info.italic);
        let Some(font_ref) = to_font_ref(primary_font) else {
            let w = text.chars().count() as f64 * info.size * 0.6;
            let ascent  = info.size * 0.9;
            let descent = info.size * 0.3;
            return TextBounds { x: 0.0, y: -ascent, w, h: ascent + descent, ascent, descent };
        };
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = font_ref.metrics(size, var_loc);
        let ascent  = metrics.ascent  as f64;
        let descent = (-metrics.descent) as f64;
        let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_size);
        let w = resolved_total_width(&glyphs) as f64;
        TextBounds {
            x: 0.0,
            y: -ascent,
            w,
            h: ascent + descent,
            ascent,
            descent,
        }
    }

    /// Real cluster shaping via cosmic-text.
    ///
    /// Correctly handles Unicode grapheme clusters (`é` as one cluster),
    /// emoji ZWJ sequences, and returns visual left-to-right order for LTR text.
    /// Results are cached per `(font, text)` pair (unbounded cache for Phase 4).
    fn measure_text_glyphs(&self, text: &str, font: &str) -> Vec<uzor::render::GlyphMetric> {
        uzor::shaper::measure_glyphs(text, font)
    }

    /// Real word-wrap via cosmic-text `Wrap::Word`.
    ///
    /// Delegates to [`uzor::shaper::measure_glyphs_wrapped`], which owns its
    /// own `(font, text, max_width)`-keyed cache separate from the unwrapped
    /// `measure_glyphs`/`text_to_path` cache.
    fn measure_text_wrapped(&self, text: &str, font: &str, max_width: f64) -> Vec<uzor::render::WrappedLine> {
        uzor::shaper::measure_glyphs_wrapped(text, font, max_width)
    }

    fn text_to_path(&self, text: &str, font: &str) -> String {
        uzor::shaper::text_to_path(text, font)
    }
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

impl Masking for VelloHybridRenderContext {
    fn clip(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform  = self.transform;
        self.clip_active = true;
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.push_clip_path(&path);
        }
    }

    /// Even-odd fill rule override: clips using `Fill::EvenOdd` so two-subpath
    /// paths (outer rect CW + inner shape CCW) produce a ring-shaped clip.
    fn push_clip_svg_path_even_odd(&mut self, d: &str) {
        uzor::render::emit_svg_path(self, d);
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        self.clip_active = true;
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_fill_rule(Fill::EvenOdd);
            s.push_clip_path(&path);
            s.set_fill_rule(Fill::NonZero);
        }
        self.save();
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

impl Effects for VelloHybridRenderContext {
    fn set_shadow(&mut self, dx: f64, dy: f64, _blur: f64, color: &str) {
        let shadow_color = parse_color(color);
        self.shadow = Some(ShadowState { dx, dy, color: shadow_color });
    }

    fn clear_shadow(&mut self) {
        self.shadow = None;
    }

    fn set_blend_mode(&mut self, mode: UzorBlendMode) {
        self.blend_mode = mode;
        let blend = Self::blend_to_hybrid(mode);
        if let Some(ref mut s) = self.scene { s.set_blend_mode(blend); }
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers
// ---------------------------------------------------------------------------

impl ShapeHelpers for VelloHybridRenderContext {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let r         = Rect::new(x, y, x + w, y + h);
        let transform = self.transform;
        if let Some(ref sh) = self.shadow.clone() {
            let shadow_transform = transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
            if let Some(ref mut s) = self.scene {
                s.set_blend_mode(vello_common::peniko::BlendMode::default());
                s.set_transform(shadow_transform);
                s.set_paint(sh.color);
                s.fill_rect(&r);
            }
        }
        self.apply_fill_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.fill_rect(&r);
        }
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let r          = Rect::new(x, y, x + w, y + h);
        let transform  = self.transform;
        let stroke_val = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_stroke(stroke_val);
            s.stroke_rect(&r);
        }
    }

    fn rounded_rect_corners(&mut self, x: f64, y: f64, w: f64, h: f64, tl: f64, tr: f64, br: f64, bl: f64) {
        let max_r = (w / 2.0).min(h / 2.0).max(0.0);
        let tl = tl.clamp(0.0, max_r);
        let tr = tr.clamp(0.0, max_r);
        let br = br.clamp(0.0, max_r);
        let bl = bl.clamp(0.0, max_r);
        self.begin_path();
        self.move_to(x + tl, y);
        self.line_to(x + w - tr, y);
        self.arc(x + w - tr, y + tr, tr, -std::f64::consts::FRAC_PI_2, 0.0);
        self.line_to(x + w, y + h - br);
        self.arc(x + w - br, y + h - br, br, 0.0, std::f64::consts::FRAC_PI_2);
        self.line_to(x + bl, y + h);
        self.arc(x + bl, y + h - bl, bl, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        self.line_to(x, y + tl);
        self.arc(x + tl, y + tl, tl, std::f64::consts::PI, std::f64::consts::PI * 1.5);
        self.close_path();
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// BatchPainter — optimized: single merged BezPath per call
// ---------------------------------------------------------------------------

impl BatchPainter for VelloHybridRenderContext {
    fn draw_line_batch(&mut self, lines: &[LineSegment], color: &str, width: f64) {
        if lines.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut path = BezPath::new();
        for l in lines {
            path.move_to(kurbo::Point::new(l.x1, l.y1));
            path.line_to(kurbo::Point::new(l.x2, l.y2));
        }
        let transform  = self.transform;
        let stroke_val = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_stroke(stroke_val);
            s.stroke_path(&path);
        }
    }

    fn draw_circle_batch(&mut self, circles: &[CircleBatch], color: &str) {
        if circles.is_empty() {
            return;
        }
        self.set_fill_color(color);
        let mut path = BezPath::new();
        for c in circles {
            let circle = kurbo::Circle::new(kurbo::Point::new(c.cx, c.cy), c.r);
            path.extend(circle.path_elements(0.1));
        }
        let transform = self.transform;
        self.apply_fill_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_fill_rule(Fill::NonZero);
            s.fill_path(&path);
        }
    }

    fn stroke_polyline(&mut self, pts: &[(f64, f64)], color: &str, width: f64) {
        if pts.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut path = BezPath::new();
        path.move_to(kurbo::Point::new(pts[0].0, pts[0].1));
        for &(x, y) in &pts[1..] {
            path.line_to(kurbo::Point::new(x, y));
        }
        let transform  = self.transform;
        let stroke_val = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_stroke(stroke_val);
            s.stroke_path(&path);
        }
    }
}

// GradientPainter
// ---------------------------------------------------------------------------

impl GradientPainter for VelloHybridRenderContext {
    fn fill_linear_gradient(&mut self, stops: &[(f32, &str)], x1: f64, y1: f64, x2: f64, y2: f64) {
        let Some(path) = self.path.clone() else { return };
        use vello_common::peniko::{ColorStop, Gradient};
        let color_stops: Vec<ColorStop> = stops.iter()
            .map(|(offset, hex)| ColorStop::from((*offset, parse_color(hex))))
            .collect();
        let gradient = Gradient::new_linear(kurbo::Point::new(x1, y1), kurbo::Point::new(x2, y2))
            .with_stops(color_stops.as_slice());
        let transform = self.transform;
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_fill_rule(Fill::NonZero);
            s.set_paint(gradient);
            s.fill_path(&path);
        }
    }

    fn fill_radial_gradient(&mut self, cx: f64, cy: f64, r: f64, stops: &[(f32, &str)], x: f64, y: f64, w: f64, h: f64) {
        let _ = (x, y, w, h);
        let Some(path) = self.path.clone() else { return };
        use vello_common::peniko::{ColorStop, Gradient};
        let color_stops: Vec<ColorStop> = stops.iter()
            .map(|(offset, hex)| ColorStop::from((*offset, parse_color(hex))))
            .collect();
        let gradient = Gradient::new_radial(kurbo::Point::new(cx, cy), r as f32)
            .with_stops(color_stops.as_slice());
        let transform = self.transform;
        if let Some(ref mut s) = self.scene {
            s.set_transform(transform);
            s.set_fill_rule(Fill::NonZero);
            s.set_paint(gradient);
            s.fill_path(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// UiEffectHelpers — all defaults (no blur support)
// ---------------------------------------------------------------------------

impl uzor::render::UiEffectHelpers for VelloHybridRenderContext {}

// ---------------------------------------------------------------------------
// RenderContext — offscreen targets are a rasterised-texture cache,
// capability-gated on `set_gpu_handles`
// ---------------------------------------------------------------------------
//
// `vello_hybrid::Scene` has no `vello::Scene::append`-equivalent: it is a
// CPU-side sparse-strips command encoder feeding a GPU fragment shader
// (`render/{wgpu,webgl}.rs`), not an encoding tree that can be merged with
// another scene's encoding. There is also no CPU-only raster path (unlike
// `vello_cpu`, which is fully self-contained) — the only way to turn a
// `Scene` into reusable content is `Renderer::render()`, which needs a
// `Device`/`Queue` this context does not own by default (`new(dpr: f64)`
// stays GPU-handle-free so the context is usable before a device exists).
//
// The implementation below is the texture-level cache the divergence note
// (formerly here) predicted as the legitimate follow-up: `set_gpu_handles`
// (setter, not a constructor change — same precedent as vello-gpu's
// `set_blur_image`) supplies `Device`/`Queue`; `push_offscreen_target`
// swaps in a fresh `Scene` recording exactly like the CPU/GPU backends;
// `pop_offscreen_target` allocates a `wgpu::Texture` (RENDER_ATTACHMENT |
// TEXTURE_BINDING, `Rgba8Unorm`) sized to the target, builds a scratch
// `Renderer` for that render-target format/size, and rasterises the
// recorded scene into it via `Renderer::render()` + a locally-submitted
// `CommandEncoder`. `draw_cached_target` binds the texture's view into a
// `TextureBindings` and replays it with `Scene::draw_texture_rects` — no
// re-rasterisation on every draw, matching the "cache" contract.
//
// `supports_offscreen_targets()` reports `true` only when both GPU
// handles are present — a capability that is conditionally true is
// honest; a stub that unconditionally claims `true` is not.
impl UzorRenderContext for VelloHybridRenderContext {
    fn dpr(&self) -> f64 { self.dpr }

    fn supports_offscreen_targets(&self) -> bool {
        self.gpu_device.is_some() && self.gpu_queue.is_some()
    }

    fn push_offscreen_target(&mut self, desc: OffscreenTargetDesc) -> OffscreenTarget {
        if !self.supports_offscreen_targets() {
            return None;
        }
        let w = desc.width_px.max(1);
        let h = desc.height_px.max(1);
        let w16 = w.min(u16::MAX as u32) as u16;
        let h16 = h.min(u16::MAX as u32) as u16;

        let id = OffscreenTargetId(self.next_offscreen_id);
        self.next_offscreen_id += 1;

        let fresh_scene = Scene::new(w16, h16);
        let saved_scene = std::mem::replace(&mut self.scene, Some(fresh_scene));
        let saved_width = std::mem::replace(&mut self.width, w);
        let saved_height = std::mem::replace(&mut self.height, h);

        self.offscreen_stack.push(SavedRecording {
            id,
            scene:       saved_scene,
            width:       saved_width,
            height:      saved_height,
            transform:   self.transform,
            clip_active: std::mem::take(&mut self.clip_active),
            state_stack: std::mem::take(&mut self.state_stack),
            path:        self.path.take(),
        });

        // The offscreen subtree paints at its own local origin — fresh
        // state, exactly like `begin_frame`.
        self.transform = Affine::IDENTITY;

        Some(id)
    }

    fn pop_offscreen_target(&mut self) {
        let Some(saved) = self.offscreen_stack.pop() else {
            return;
        };
        let recorded_scene = std::mem::replace(&mut self.scene, saved.scene);
        let recorded_width = std::mem::replace(&mut self.width, saved.width);
        let recorded_height = std::mem::replace(&mut self.height, saved.height);
        self.transform   = saved.transform;
        self.clip_active = saved.clip_active;
        self.state_stack = saved.state_stack;
        self.path        = saved.path;

        let (Some(scene), Some(device), Some(queue)) =
            (recorded_scene, self.gpu_device.clone(), self.gpu_queue.clone())
        else {
            // Handles were revoked mid-recording (should not happen given
            // the `push` gate, but stay total rather than panicking) —
            // drop the recording, nothing to cache.
            return;
        };

        let w = recorded_width.max(1);
        let h = recorded_height.max(1);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor-vello-hybrid-offscreen-target"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut renderer = Renderer::new(&device, &RenderTargetConfig {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width:  w,
            height: h,
        });
        let mut resources = vello_hybrid::Resources::new();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uzor-vello-hybrid-offscreen-target-encoder"),
        });
        // Bind any already-cached targets this recording itself referenced
        // via `draw_cached_target` (nested offscreen boundaries) — without
        // this, `Renderer::render` would fail with `MissingTextureBinding`
        // for a target-of-a-target.
        let render_result = renderer.render(
            &scene,
            &mut resources,
            &device,
            &queue,
            &mut encoder,
            &RenderSize { width: w, height: h },
            &view,
            &self.cached_target_bindings(),
        );
        if render_result.is_err() {
            // Rasterisation failed (e.g. slot exhaustion on a pathological
            // scene) — nothing to cache; the target simply stays absent
            // and `draw_cached_target` reports `false` for this id.
            return;
        }
        queue.submit([encoder.finish()]);

        // `desc` (the caller-requested logical size/dpr) is intentionally
        // not stored — the actual raster size (`w`, `h`, already clamped
        // to >=1) is what the texture was allocated at and what
        // `resize_offscreen_target` compares against; keeping a second,
        // possibly-divergent copy would invite drift.
        let texture_id = TextureId(saved.id.0);
        self.offscreen_targets.insert(saved.id, CachedTarget {
            view,
            texture_id,
            width: w,
            height: h,
        });
    }

    fn draw_cached_target(&mut self, id: OffscreenTargetId, dst_rect: UzorRect) -> bool {
        if dst_rect.width <= 0.0 || dst_rect.height <= 0.0 {
            return false;
        }
        let Some(target) = self.offscreen_targets.get(&id) else {
            return false;
        };
        let Some(ref mut scene) = self.scene else {
            return false;
        };

        // `draw_texture_rects` samples via an externally-bound `TextureId`
        // — the binding itself is call-site state (`TextureBindings`)
        // consulted only at `Renderer::render()` time, not stored on the
        // `Scene`. `render()` above builds that binding set via
        // `cached_target_bindings()`, covering every id this call could
        // have referenced (root or nested recording).
        let source_region = vello_common::geometry::RectU16::new(
            0, 0,
            target.width.min(u16::MAX as u32) as u16,
            target.height.min(u16::MAX as u32) as u16,
        );
        let scale_x = dst_rect.width / f64::from(target.width.max(1));
        let scale_y = dst_rect.height / f64::from(target.height.max(1));
        let rect_transform = Affine::translate((dst_rect.x, dst_rect.y))
            * Affine::scale_non_uniform(scale_x, scale_y);

        scene.set_transform(self.transform);
        scene.draw_texture_rects(
            target.texture_id,
            vello_common::peniko::ImageQuality::Medium,
            [vello_hybrid::SampleRect { source_region, transform: rect_transform }],
        );
        true
    }

    fn resize_offscreen_target(&mut self, id: OffscreenTargetId, desc: OffscreenTargetDesc) -> bool {
        // Rasterised content no longer matches the requested size once
        // resized — same "report success only for a no-op resize"
        // contract `VelloCpuRenderContext::resize_offscreen_target` uses;
        // callers otherwise `free` + `push` a new target at the new size.
        self.offscreen_targets
            .get(&id)
            .is_some_and(|t| t.width == desc.width_px.max(1) && t.height == desc.height_px.max(1))
    }

    fn free_offscreen_target(&mut self, id: OffscreenTargetId) {
        self.offscreen_targets.remove(&id);
    }
}

// ---------------------------------------------------------------------------
// RenderContextExt — blur/glass effects (no-op for hybrid backend)
// ---------------------------------------------------------------------------

impl RenderContextExt for VelloHybridRenderContext {
    /// Hybrid backend carries no CPU-side blur image state.
    type BlurImage = ();

    fn set_blur_image(&mut self, _image: Option<()>, _width: u32, _height: u32) {
        // Blur backgrounds are a vello-gpu-specific feature.
    }

    fn set_use_convex_glass_buttons(&mut self, _use_convex: bool) {
        // Convex glass buttons are a vello-gpu-specific feature.
    }
}

// ---------------------------------------------------------------------------
// Tests — capability gate on `set_gpu_handles`
// ---------------------------------------------------------------------------
//
// A real push/pop/draw round trip needs an actual `wgpu::Device`/`Queue`
// (adapter request is async and requires a live backend — not something a
// headless unit test can construct deterministically, unlike tiny-skia's
// `Pixmap`/vello-cpu's/`vello`'s pure-CPU/pure-vector paths). What IS real
// and headlessly testable is the capability gate itself: `false` before
// `set_gpu_handles`, every method a total (never-panics) no-op in that
// state — exactly the honest-`None`-until-wired contract this backend's
// `RenderContext` impl documents above.
#[cfg(test)]
mod tests {
    use super::*;
    use uzor::render::{OffscreenTargetDesc, OffscreenTargetId};

    #[test]
    fn offscreen_targets_unsupported_before_gpu_handles_are_set() {
        let mut ctx = VelloHybridRenderContext::new(1.0);
        ctx.begin_frame(64, 64);

        assert!(!ctx.supports_offscreen_targets());

        let desc = OffscreenTargetDesc { width_px: 32, height_px: 32, dpr: 1.0 };
        assert!(ctx.push_offscreen_target(desc).is_none());

        // Safe to call even without a successful push (contract: total, no panic).
        ctx.pop_offscreen_target();

        let id = OffscreenTargetId(0);
        let dst = uzor::core::types::Rect { x: 0.0, y: 0.0, width: 32.0, height: 32.0 };
        assert!(!ctx.draw_cached_target(id, dst));
        assert!(!ctx.resize_offscreen_target(id, desc));

        // Safe to call for an id that was never allocated.
        ctx.free_offscreen_target(id);
    }
}
