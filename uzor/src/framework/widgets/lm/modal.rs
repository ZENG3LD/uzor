//! `ModalBuilder` — chainable, default-friendly wrapper around
//! `register_layout_manager_modal`.
//!
//! Usage:
//! ```ignore
//! let h = layout.add_modal("settings");           // once at init
//! lm::modal(&h)                                    // every frame
//!     .title("Settings")
//!     .resizable(true)
//!     .rect(Rect::new(200.0, 150.0, 600.0, 400.0))
//!     .build(&mut layout, &mut render);
//! // ... draw body content via lm::body_rect_for(&h) lookup
//! ```
//!
//! All view / settings / kind / parent / slot_id parameters are hidden under
//! sensible defaults; chainable methods expose them when overrides are needed.

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, ModalHandle, ModalNode};
use crate::render::RenderContext;
use crate::ui::widgets::composite::modal::input::register_layout_manager_modal;
use crate::ui::widgets::composite::modal::settings::ModalSettings;
use crate::ui::widgets::composite::modal::style::{DefaultModalStyle, ModalStyle};
use crate::ui::widgets::composite::modal::theme::ModalTheme;
use crate::tokens::{Tokens, TokenTheme};
use crate::ui::widgets::composite::modal::types::{
    BackdropKind, FooterBtn, ModalRenderKind, ModalView, WizardPageInfo,
};
use crate::types::OverflowMode;

/// Builds `ModalSettings` from the layout's live token set (H1 Brief 9, §3
/// "StyleManager — removed") — replaces the pre-H1 `StyledModalTheme`
/// bridge, which hand-copied a handful of `StyleManager` keys and fell back
/// to a dark `TokenTheme` for the rest.
fn modal_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> ModalSettings {
    ModalSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
        style: Box::<DefaultModalStyle>::default(),
    }
}

/// Chainable builder for a modal overlay frame.
///
/// `.build(layout, render)` is the terminal call; the body content
/// inside the modal is drawn separately by the caller.
pub struct ModalBuilder<'a> {
    handle:       &'a ModalHandle,
    parent:       LayoutNodeId,
    slot_id:      Option<&'a str>,
    overlay_rect: Option<Rect>,
    anchor:       Option<Rect>,
    title:        Option<&'a str>,
    tabs:         &'a [&'a str],
    footer:       &'a [FooterBtn<'a>],
    wizard_pages: &'a [WizardPageInfo<'a>],
    backdrop:     BackdropKind,
    overflow:     OverflowMode,
    resizable:    bool,
    settings:     Option<ModalSettings>,
    /// Override only the colour-token bundle.  Wins over the
    /// token-set-derived default but loses to a full
    /// `.settings(...)` call.
    theme_override: Option<Box<dyn ModalTheme>>,
    /// Override only the geometry bundle.  Same precedence rules as
    /// `theme_override`.
    style_override: Option<Box<dyn ModalStyle>>,
    kind:         ModalRenderKind,
}

/// Entry point: start a `ModalBuilder` for the given handle.
pub fn modal<'a>(handle: &'a ModalHandle) -> ModalBuilder<'a> {
    ModalBuilder::new(handle)
}

impl<'a> ModalBuilder<'a> {
    /// New builder with all fields at default.
    pub fn new(handle: &'a ModalHandle) -> Self {
        Self {
            handle,
            parent:       LayoutNodeId::ROOT,
            slot_id:      None,
            overlay_rect: None,
            anchor:       None,
            title:        None,
            tabs:         &[],
            footer:       &[],
            wizard_pages: &[],
            backdrop:     BackdropKind::Dim,
            overflow:     OverflowMode::Clip,
            resizable:    false,
            settings:     None,
            theme_override: None,
            style_override: None,
            kind:         ModalRenderKind::WithHeader,
        }
    }

    /// Override the parent layout node (default `LayoutNodeId::ROOT`).
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Override the overlay slot id (default = handle id).
    ///
    /// Use when the modal is anchored to a non-default overlay slot in the
    /// layout's overlay registry — otherwise the handle id is used.
    pub fn slot_id(mut self, s: &'a str) -> Self { self.slot_id = Some(s); self }

    /// Explicit overlay rect (default: auto-centered in last solved viewport
    /// using `(600, 400)` size).
    pub fn rect(mut self, r: Rect) -> Self { self.overlay_rect = Some(r); self }

    /// Optional anchor rect (e.g. trigger button) used by re-positioning
    /// logic. Default `None`.
    pub fn anchor(mut self, r: Rect) -> Self { self.anchor = Some(r); self }

    /// Modal title (default `None`).
    pub fn title(mut self, t: &'a str) -> Self { self.title = Some(t); self }

    /// Tab labels for `TopTabs` / `SideTabs` kinds (default empty).
    pub fn tabs(mut self, ts: &'a [&'a str]) -> Self { self.tabs = ts; self }

    /// Footer action buttons (default empty).
    pub fn footer(mut self, btns: &'a [FooterBtn<'a>]) -> Self { self.footer = btns; self }

    /// Wizard pages (default empty; only used by `Wizard` kind).
    pub fn wizard_pages(mut self, pages: &'a [WizardPageInfo<'a>]) -> Self {
        self.wizard_pages = pages;
        self
    }

    /// Backdrop fill behind the modal (default `Dim`).
    pub fn backdrop(mut self, b: BackdropKind) -> Self { self.backdrop = b; self }

    /// Body overflow strategy (default `Clip`).
    pub fn overflow(mut self, m: OverflowMode) -> Self { self.overflow = m; self }

    /// Allow user-driven resize (default `false`).
    pub fn resizable(mut self, on: bool) -> Self { self.resizable = on; self }

    /// Override visual settings (default `ModalSettings::default()`).
    pub fn settings(mut self, s: ModalSettings) -> Self { self.settings = Some(s); self }

    /// Override render kind (default `ModalRenderKind::WithHeader`).
    pub fn kind(mut self, k: ModalRenderKind) -> Self { self.kind = k; self }

    /// Override only the modal theme (colour tokens).  Useful for
    /// per-modal accents without forking the whole `ModalSettings`.
    pub fn theme(mut self, t: Box<dyn ModalTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the modal style (geometry — radius, padding, header
    /// height …).
    pub fn style(mut self, s: Box<dyn ModalStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

    /// Terminal call — register and draw the modal frame.
    /// Body contents inside the frame are drawn separately by the caller.
    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<ModalNode> {
        self.build_with_body(layout, render, |_, _, _: Rect| {})
    }

    /// Same as [`build`] but lets the caller paint the modal body
    /// inside the composite's body rect.
    ///
    /// `body` runs *after* the modal chrome (backdrop, frame, header,
    /// tabs, footer) is drawn, with the renderer already clipped to
    /// the body rect and the overflow transform applied:
    ///
    /// - [`OverflowMode::Compress`] — body is uniformly scaled when
    ///   geometry is provided via `.content_height(...)` (modal does
    ///   not currently track scroll offset, so `Scrollbar` falls back
    ///   to `Clip`).
    /// - [`OverflowMode::Clip`] / `Chevrons` — only the clip is applied.
    pub fn build_with_body<P, F>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
        body: F,
    ) -> Option<ModalNode>
    where
        P: DockPanel,
        F: FnOnce(&mut LayoutManager<P>, &mut dyn RenderContext, Rect),
    {
        let slot_id = self.slot_id
            .map(str::to_owned)
            .unwrap_or_else(|| self.handle.id_str().to_string());

        let overlay_rect = self.overlay_rect.unwrap_or_else(|| default_modal_rect(layout));

        let mut view = ModalView {
            title:          self.title,
            tabs:           self.tabs,
            footer_buttons: self.footer,
            wizard_pages:   self.wizard_pages,
            backdrop:       self.backdrop,
            overflow:       self.overflow,
            resizable:      self.resizable,
        };

        // Resolve settings: explicit `.settings(...)` wins outright,
        // otherwise build from the layout's live token set and then patch
        // in any `.theme(...)` / `.style(...)` overrides.
        let mut settings = self.settings.unwrap_or_else(|| modal_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }

        let kind = self.kind;
        let overflow = self.overflow;
        let parent = self.parent;
        let anchor = self.anchor;
        let handle = self.handle;

        let node = register_layout_manager_modal(
            layout,
            render,
            parent,
            &slot_id,
            handle,
            overlay_rect,
            anchor,
            &mut view,
            &settings,
            &kind,
        );

        // Resolve the modal's frame rect (post-drag) and paint the body.
        let frame = layout
            .rect_for_overlay(&slot_id)
            .unwrap_or(overlay_rect);
        if frame.width > 0.0 && frame.height > 0.0 {
            let body_rect = crate::ui::widgets::composite::modal::render::body_rect(
                frame, &view, &settings, &kind,
            );
            render.save();
            render.clip_rect(body_rect.x, body_rect.y, body_rect.width, body_rect.height);
            if let OverflowMode::Compress = overflow {
                // Modal has no content_height field on the builder yet —
                // callers can pre-scale inside the body closure if needed.
                // (Keeping the match arm for symmetry with panel/sidebar.)
            }
            body(layout, render, body_rect);
            render.restore();
        }

        node
    }
}

/// Default modal rect — centered in the last solved viewport at 600×400.
fn default_modal_rect<P: DockPanel>(layout: &LayoutManager<P>) -> Rect {
    let (w, h) = (600.0_f64, 400.0_f64);
    let viewport = layout.last_window().unwrap_or(Rect::new(0.0, 0.0, 1280.0, 800.0));
    Rect::new(
        viewport.x + (viewport.width  - w) / 2.0,
        viewport.y + (viewport.height - h) / 2.0,
        w, h,
    )
}
