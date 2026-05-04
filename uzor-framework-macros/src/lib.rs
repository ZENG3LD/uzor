//! Procedural macros for `uzor-framework`.
//!
//! Exports `view!` — JSX-mimicking DSL that lowers to `lm::*` builder calls
//! against the existing L3 `LayoutManager`. No new state, no second manager;
//! pure compile-time text-rewriting.
//!
//! The macro expects two implicit identifiers in scope:
//! - `layout: &mut LayoutManager<P>`
//! - `render: &mut dyn RenderContext`
//!
//! And one prop-supplied rect on the root node (`<row rect={r}>`), from which
//! children rects are derived via `uzor_framework::layout::row()/col()` flex
//! helpers.

extern crate proc_macro;

mod parse;
mod lower;

use proc_macro::TokenStream;

/// JSX-style view tree. Lowers to imperative `uzor_framework::lm::*` calls.
///
/// ```ignore
/// view! {
///     <col rect={body} gap=8>
///         <button text="Save" on_click={|| self.save()} />
///         <checkbox bind={&mut self.dark} label="Dark" />
///     </col>
/// }
/// ```
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let node = match syn::parse::<parse::Node>(input) {
        Ok(n) => n,
        Err(e) => return e.to_compile_error().into(),
    };
    lower::lower_root(&node).into()
}
