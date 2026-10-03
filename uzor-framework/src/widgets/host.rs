//! The paint door shared by [`PanelCx`] and [`OverlayCx`].

use uzor::render::RenderContext;
use uzor::tokens::Tokens;

use crate::handle::{OverlayCx, PanelCx, VisualView, Widgets};
use crate::types::spec::Spec;

/// A content hook context a widget convenience function can paint into.
///
/// Implemented for [`PanelCx`] and [`OverlayCx`]. The closure receives the
/// disjoint pieces of one frame (paint target, registration face, this
/// frame's input truth, tokens) so a widget can register and draw without
/// reaching an engine.
pub trait ContentCx<'a> {
    /// The app vocabulary of the context.
    type Spec: Spec;

    /// Run `f` against this frame's paint target and registration face.
    fn with_paint<R>(
        &mut self,
        f: impl FnOnce(&mut dyn RenderContext, &mut Widgets<'a>, &VisualView<Self::Spec>, &Tokens) -> R,
    ) -> R;
}

impl<'a, S: Spec> ContentCx<'a> for PanelCx<'a, S> {
    type Spec = S;

    fn with_paint<R>(
        &mut self,
        f: impl FnOnce(&mut dyn RenderContext, &mut Widgets<'a>, &VisualView<S>, &Tokens) -> R,
    ) -> R {
        f(self.render, &mut self.widgets, self.view, self.tokens)
    }
}

impl<'a, S: Spec> ContentCx<'a> for OverlayCx<'a, S> {
    type Spec = S;

    fn with_paint<R>(
        &mut self,
        f: impl FnOnce(&mut dyn RenderContext, &mut Widgets<'a>, &VisualView<S>, &Tokens) -> R,
    ) -> R {
        f(self.render, &mut self.widgets, self.view, self.tokens)
    }
}
