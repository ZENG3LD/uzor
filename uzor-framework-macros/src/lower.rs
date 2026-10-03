//! Lowering: parsed AST → `TokenStream` of imperative `lm::*` builder calls.
//!
//! Each node receives an implicit `__rect: Rect` from its parent. Layout
//! containers (`row`, `col`) split that rect into child rects via
//! `uzor_framework::flex::flex_solve()` and recurse.
//!
//! Atomic tags (`button`, `text`, `checkbox`, `toggle`, `separator`) and
//! `<chrome>` emit one
//! `uzor_framework::widgets::lm::*(…).…build(cx)` call. The caller must have
//! `cx: &mut impl uzor_framework::widgets::ContentCx` in scope.
//!
//! Overlay tags (`modal`, `popup`, `dropdown`, `context_menu`) emit a
//! compile error in M1: F8 left those layout façade-backed chain builders
//! unported; M1 does not revive them. Use `OverlayCmd` plus the
//! `widgets::{modal,popup,…}` convenience functions until a later brief.
//!
//! Auto-IDs derive from a path string `view::<tag>[<index>]::…` collected at
//! compile time. User-provided `id="..."` overrides.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::Ident;

use crate::parse::{Element, Node, Prop};

pub fn lower_root(node: &Node) -> TokenStream {
    let path = String::from("v");
    match node {
        Node::Element(el) => {
            let tag = el.tag.to_string();
            // Flex containers (`<row>`/`<col>`) need a parent rect — they
            // distribute it to children. Overlays/chrome/atomics establish
            // their own rect (handle, body lookup, anchor) and don't need
            // one from the macro's outside.
            let needs_rect = matches!(tag.as_str(), "row" | "col");
            if needs_rect {
                let rect = match find_prop(el, "rect") {
                    Some(p) => p.value_tokens(),
                    None => {
                        return syn::Error::new(
                            el.span,
                            "root <row>/<col> needs a `rect={...}` prop",
                        )
                        .to_compile_error()
                    }
                };
                let inner = lower_element_body(el, &path, 0);
                quote! {
                    {
                        let __rect: ::uzor::Rect = #rect;
                        #inner
                    }
                }
            } else {
                // Atomic / composite at the root: no parent rect needed.
                // Provide a dummy `__rect` so atomic builders that read it
                // (e.g. `<text>`, `<button>`) still compile — but in
                // practice the root is usually `<modal>`/`<chrome>` etc.
                let inner = lower_element_body(el, &path, 0);
                quote! {
                    {
                        let __rect: ::uzor::Rect =
                            ::uzor::Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
                        let _ = __rect;
                        #inner
                    }
                }
            }
        }
        Node::Expr(e) => quote!({ #e }),
    }
}

/// Lower an element assuming `__rect: Rect` is already in scope.
fn lower_element(el: &Element, path: &str, idx: usize) -> TokenStream {
    lower_element_body(el, path, idx)
}

fn lower_element_body(el: &Element, path: &str, idx: usize) -> TokenStream {
    let tag_str = el.tag.to_string();
    let child_path = format!("{}::{}[{}]", path, tag_str, idx);

    match tag_str.as_str() {
        "row" => lower_flex(el, &child_path, FlexDir::Row),
        "col" => lower_flex(el, &child_path, FlexDir::Col),
        "button"    => lower_atom(el, &child_path, AtomKind::Button),
        "text"      => lower_atom(el, &child_path, AtomKind::Text),
        "checkbox"  => lower_atom(el, &child_path, AtomKind::Checkbox),
        "toggle"    => lower_atom(el, &child_path, AtomKind::Toggle),
        "separator" => lower_atom(el, &child_path, AtomKind::Separator),
        "modal" | "popup" | "dropdown" | "context_menu" => lower_overlay_deferred(el),
        "chrome"       => lower_chrome(el),
        _ => syn::Error::new(
            el.tag.span(),
            format!(
                "unknown view tag `<{}>` (M1 supports: row, col, button, text, checkbox, toggle, separator, chrome; overlay tags deferred)",
                tag_str
            ),
        )
        .to_compile_error(),
    }
}

/// Overlay chain builders still need layout façade handles (F8 left them
/// unported). M1 only retargets paths onto `uzor_framework::{widgets, flex}`;
/// it does not revive those builders. Emit a clear error instead of a
/// layout façade call chain.
fn lower_overlay_deferred(el: &Element) -> TokenStream {
    let tag = el.tag.to_string();
    syn::Error::new(
        el.tag.span(),
        format!(
            "<{tag}> is deferred past M1: F8 left layout façade-backed overlay chain builders unported, and M1 does not revive layout façade. Open overlays with OverlayCmd and paint with uzor_framework::widgets::{{{tag}, …}} for now"
        ),
    )
    .to_compile_error()
}

/// `<chrome>` — singleton window chrome strip.  Props: `tabs={&[...]}`,
/// `active_tab="id"`, `cursor={(x,y)}`, `time_ms={ms}`.  No children.
fn lower_chrome(el: &Element) -> TokenStream {
    let tabs = find_prop(el, "tabs").map(|p| p.value_tokens());
    let active_tab = find_prop(el, "active_tab").map(|p| p.value_tokens());
    let cursor = find_prop(el, "cursor").map(|p| p.value_tokens());
    let time_ms = find_prop(el, "time_ms").map(|p| p.value_tokens());
    let new_tab = find_prop(el, "show_new_tab").map(|p| p.value_tokens());
    let menu_btn = find_prop(el, "show_menu").map(|p| p.value_tokens());
    let new_win = find_prop(el, "show_new_window").map(|p| p.value_tokens());

    let mut chain = quote!(::uzor_framework::widgets::lm::chrome());
    if let Some(t) = tabs {
        chain = quote!(#chain.tabs(#t));
    }
    if let Some(a) = active_tab {
        chain = quote!(#chain.active_tab(#a));
    }
    if let Some(c) = cursor {
        chain = quote!(#chain.cursor(#c));
    }
    if let Some(tm) = time_ms {
        chain = quote!(#chain.time_ms(#tm));
    }
    if let Some(n) = new_tab {
        chain = quote!(#chain.show_new_tab_btn(#n));
    }
    if let Some(m) = menu_btn {
        chain = quote!(#chain.show_menu_btn(#m));
    }
    if let Some(w) = new_win {
        chain = quote!(#chain.show_new_window_btn(#w));
    }

    if !el.children.is_empty() {
        return syn::Error::new(
            el.tag.span(),
            "<chrome> does not accept children — use `tabs={&[...]}` instead",
        )
        .to_compile_error();
    }

    quote!({ #chain.build(cx); })
}

#[derive(Copy, Clone)]
enum FlexDir {
    Row,
    Col,
}

fn lower_flex(el: &Element, path: &str, dir: FlexDir) -> TokenStream {
    let gap = find_prop(el, "gap")
        .map(|p| p.value_tokens())
        .unwrap_or_else(|| quote!(0.0_f64));
    let pad = find_prop(el, "pad")
        .map(|p| p.value_tokens())
        .unwrap_or_else(|| quote!(0.0_f64));

    // Element children — only those that are <Element>s participate in the
    // flex layout; bare `{expr}` children pass through verbatim and use the
    // parent rect as-is.
    let mut child_specs: Vec<TokenStream> = Vec::new();
    let mut child_lowers: Vec<TokenStream> = Vec::new();

    let mut element_idx: usize = 0;
    for child in &el.children {
        match child {
            Node::Element(ce) => {
                let weight = find_prop(ce, "flex")
                    .map(|p| p.value_tokens())
                    .unwrap_or_else(|| quote!(1.0_f64));
                let basis = find_prop(ce, "size")
                    .map(|p| p.value_tokens())
                    .unwrap_or_else(|| quote!(0.0_f64));
                child_specs.push(quote! {
                    ::uzor_framework::flex::FlexChild { basis: #basis, flex: #weight }
                });
                let i = element_idx;
                let body = lower_element(ce, path, i);
                child_lowers.push(quote! {
                    {
                        let __rect = __rects[#i];
                        #body
                    }
                });
                element_idx += 1;
            }
            Node::Expr(e) => {
                child_lowers.push(quote!({ #e }));
            }
        }
    }

    let dir_token = match dir {
        FlexDir::Row => quote!(::uzor_framework::flex::FlexDir::Row),
        FlexDir::Col => quote!(::uzor_framework::flex::FlexDir::Col),
    };

    quote! {
        {
            let __children: &[::uzor_framework::flex::FlexChild] = &[ #(#child_specs),* ];
            let __rects: ::std::vec::Vec<::uzor::Rect> =
                ::uzor_framework::flex::flex_solve(__rect, #dir_token, #gap as f64, #pad as f64, __children);
            #(#child_lowers)*
        }
    }
}

#[derive(Copy, Clone)]
enum AtomKind {
    Button,
    Text,
    Checkbox,
    Toggle,
    Separator,
}

fn lower_atom(el: &Element, path: &str, kind: AtomKind) -> TokenStream {
    // Resolve id: `id="foo"` literal or auto path string.
    let id_expr = match find_prop(el, "id") {
        Some(p) => p.value_tokens(),
        None => {
            let s = path.to_string();
            quote!(#s)
        }
    };
    let id_make = quote! {
        ::uzor::unsafe_widget_id(#id_expr)
    };

    match kind {
        AtomKind::Button => {
            let text = find_prop(el, "text").map(|p| p.value_tokens());
            let on_click = find_prop(el, "on_click").map(|p| p.value_tokens());
            let bind_count = find_prop(el, "bind_count").map(|p| p.value_tokens());
            let active = find_prop(el, "active").map(|p| p.value_tokens());
            let disabled = find_prop(el, "disabled").map(|p| p.value_tokens());

            let mut chain = quote!(::uzor_framework::widgets::lm::button(#id_make, __rect));
            if let Some(t) = text {
                chain = quote!(#chain.text(#t));
            }
            if let Some(a) = active {
                chain = quote!(#chain.active(#a));
            }
            if let Some(d) = disabled {
                chain = quote!(#chain.disabled(#d));
            }
            if let Some(c) = on_click {
                chain = quote!(#chain.on_click(#c));
            }
            if let Some(b) = bind_count {
                chain = quote!(#chain.bind_count(#b));
            }

            quote! {
                {
                    #chain.build(cx);
                }
            }
        }

        AtomKind::Text => {
            let text = find_prop(el, "text")
                .map(|p| p.value_tokens())
                .unwrap_or_else(|| quote!(""));
            let color = find_prop(el, "color").map(|p| p.value_tokens());
            let mut chain = quote!(::uzor_framework::widgets::lm::text(#id_make, __rect, #text));
            if let Some(c) = color {
                chain = quote!(#chain.color(#c));
            }
            quote!({ #chain.build(cx); })
        }

        AtomKind::Checkbox => {
            let bind = find_prop(el, "bind").map(|p| p.value_tokens());
            let label = find_prop(el, "label").map(|p| p.value_tokens());
            let checked = find_prop(el, "checked").map(|p| p.value_tokens());
            let mut chain = quote!(::uzor_framework::widgets::lm::checkbox(#id_make, __rect));
            if let Some(b) = bind {
                chain = quote!(#chain.bind(#b));
            }
            if let Some(l) = label {
                chain = quote!(#chain.label(#l));
            }
            if let Some(c) = checked {
                chain = quote!(#chain.checked(#c));
            }
            quote!({ #chain.build(cx); })
        }

        AtomKind::Toggle => {
            let bind = find_prop(el, "bind").map(|p| p.value_tokens());
            let label = find_prop(el, "label").map(|p| p.value_tokens());
            let mut chain = quote!(::uzor_framework::widgets::lm::toggle(#id_make, __rect));
            if let Some(b) = bind {
                chain = quote!(#chain.bind(#b));
            }
            if let Some(l) = label {
                chain = quote!(#chain.label(#l));
            }
            quote!({ #chain.build(cx); })
        }

        AtomKind::Separator => {
            let chain = quote!(::uzor_framework::widgets::lm::separator(#id_make, __rect));
            quote!({ #chain.build(cx); })
        }
    }
}

fn find_prop<'a>(el: &'a Element, name: &str) -> Option<&'a Prop> {
    el.props.iter().find(|p| p.name == name)
}

// silence unused-import warnings if quote_spanned/format_ident not used yet
#[allow(dead_code)]
fn _suppress_unused() {
    let _: Span = Span::call_site();
    let _ = format_ident!("x");
    let _ = quote_spanned!(Span::call_site() => 0);
    let _ = Ident::new("x", Span::call_site());
}
