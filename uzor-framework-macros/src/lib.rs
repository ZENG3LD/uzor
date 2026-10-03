//! Procedural macros for `uzor-framework`.
//!
//! Exports `view!` — JSX-mimicking DSL that lowers to `widgets::lm::*`
//! builder calls against a [`uzor_framework::widgets::ContentCx`] (a
//! `PanelCx` or `OverlayCx`). Pure compile-time rewriting; no second
//! manager.
//!
//! The macro expects one implicit identifier in scope:
//! - `cx: &mut impl uzor_framework::widgets::ContentCx`
//!
//! And one prop-supplied rect on a root flex node (`<row rect={r}>` /
//! `<col rect={r}>`), from which children rects are derived via
//! `uzor_framework::flex::flex_solve`.

extern crate proc_macro;

mod lower;
mod parse;

use proc_macro::TokenStream;

/// JSX-style view tree. Lowers to imperative
/// `uzor_framework::widgets::lm::*` calls.
///
/// ```ignore
/// view! {
///     <col rect={body} gap=8>
///         <button text="Save" on_click={|| self.save()} />
///         <checkbox bind={&mut self.dark} label="Dark" />
///     </col>
/// }
/// ```
///
/// Overlay tags (`modal`, `popup`, `dropdown`, `context_menu`) are deferred
/// past M1 — they still needed LayoutManager handles after F8.
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let node = match syn::parse::<parse::Node>(input) {
        Ok(n) => n,
        Err(e) => return e.to_compile_error().into(),
    };
    lower::lower_root(&node).into()
}
