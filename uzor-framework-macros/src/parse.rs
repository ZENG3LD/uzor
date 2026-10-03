//! JSX-ish parser. Built on `syn` token-stream walking — not full HTML.
//!
//! Grammar (informal):
//!
//! ```text
//! Node       := Element | TextLit
//! Element    := '<' Ident Prop* ('/>' | '>' Child* '</' Ident '>')
//! Prop       := Ident ('=' (Lit | '{' Expr '}'))?       // bare ident → bool true
//! Child      := Element | '{' Expr '}'                  // bare {expr} = embed token
//! ```
//!
//! Notes:
//! - Tag names are bare idents (`row`, `button`, `Modal`).
//! - Prop values come in two flavours: literals (`text="Save"`, `gap=8`) or
//!   braced exprs (`on_click={|| ...}`, `bind={&mut self.x}`).
//! - Bare `{expr}` as a child embeds a Rust expression that evaluates to
//!   `()` (e.g. `{ for x in &xs { view!{...} } }`).

use proc_macro2::{Ident, Span, TokenStream};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    token, Expr, Lit, Token,
};

#[derive(Debug)]
pub enum Node {
    Element(Element),
    /// Free Rust expression child — embedded verbatim in lowered output.
    /// Used for `{ for x in xs { ... } }` and `{ if cond { ... } }`.
    Expr(Expr),
}

#[derive(Debug)]
pub struct Element {
    pub tag: Ident,
    pub props: Vec<Prop>,
    pub children: Vec<Node>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Prop {
    pub name: Ident,
    pub value: PropValue,
}

#[derive(Debug)]
pub enum PropValue {
    /// Bare ident (no `=`) → boolean `true`.
    Flag,
    Lit(Lit),
    Expr(Expr),
}

impl Parse for Node {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![<]) {
            let el: Element = input.parse()?;
            Ok(Node::Element(el))
        } else if input.peek(token::Brace) {
            let inner;
            braced!(inner in input);
            let expr: Expr = inner.parse()?;
            Ok(Node::Expr(expr))
        } else {
            Err(input.error("expected `<tag>` or `{expr}`"))
        }
    }
}

impl Parse for Element {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lt: Token![<] = input.parse()?;
        let span = lt.span;
        let tag: Ident = input.parse()?;

        // Props
        let mut props = Vec::new();
        while !input.peek(Token![/]) && !input.peek(Token![>]) {
            props.push(input.parse::<Prop>()?);
        }

        // Self-closing or paired
        if input.peek(Token![/]) {
            let _: Token![/] = input.parse()?;
            let _: Token![>] = input.parse()?;
            return Ok(Element {
                tag,
                props,
                children: Vec::new(),
                span,
            });
        }

        let _: Token![>] = input.parse()?;

        // Children until </tag>
        let mut children = Vec::new();
        loop {
            if input.peek(Token![<]) && input.peek2(Token![/]) {
                break;
            }
            if input.is_empty() {
                return Err(input.error("unexpected EOF — missing closing tag"));
            }
            children.push(input.parse::<Node>()?);
        }

        let _: Token![<] = input.parse()?;
        let _: Token![/] = input.parse()?;
        let close: Ident = input.parse()?;
        let _: Token![>] = input.parse()?;

        if close != tag {
            return Err(syn::Error::new(
                close.span(),
                format!("closing tag `</{}>` does not match `<{}>`", close, tag),
            ));
        }

        Ok(Element {
            tag,
            props,
            children,
            span,
        })
    }
}

impl Parse for Prop {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if !input.peek(Token![=]) {
            return Ok(Prop {
                name,
                value: PropValue::Flag,
            });
        }
        let _: Token![=] = input.parse()?;
        if input.peek(token::Brace) {
            let inner;
            braced!(inner in input);
            let expr: Expr = inner.parse()?;
            Ok(Prop {
                name,
                value: PropValue::Expr(expr),
            })
        } else {
            let lit: Lit = input.parse()?;
            Ok(Prop {
                name,
                value: PropValue::Lit(lit),
            })
        }
    }
}

impl Prop {
    /// Yield the prop value as a token stream usable in a Rust expression
    /// position.  `Flag` → `true`.
    pub fn value_tokens(&self) -> TokenStream {
        use quote::ToTokens;
        match &self.value {
            PropValue::Flag => quote::quote!(true),
            PropValue::Lit(l) => l.to_token_stream(),
            PropValue::Expr(e) => e.to_token_stream(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    #[test]
    fn parses_self_closing_button() {
        let node: Node = parse_str(r#"<button text="Save" />"#).unwrap();
        match node {
            Node::Element(el) => {
                assert_eq!(el.tag.to_string(), "button");
                assert_eq!(el.props.len(), 1);
                assert!(el.children.is_empty());
            }
            Node::Expr(_) => panic!("expected element"),
        }
    }

    #[test]
    fn parses_col_with_children() {
        let node: Node = parse_str(
            r#"<col rect={body} gap=8>
                <button text="A" />
                <text text="B" />
            </col>"#,
        )
        .unwrap();
        match node {
            Node::Element(el) => {
                assert_eq!(el.tag.to_string(), "col");
                assert_eq!(el.props.len(), 2);
                assert_eq!(el.children.len(), 2);
            }
            Node::Expr(_) => panic!("expected element"),
        }
    }

    #[test]
    fn rejects_mismatched_close_tag() {
        let err = parse_str::<Node>(r#"<row></col>"#).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("does not match"), "{msg}");
    }
}
