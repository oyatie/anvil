use super::Syntax;
use proc_macro2::{TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream, Parser as SynParser};
use syn::punctuated::Punctuated;
use syn::token::{Comma, Else, Eq as EqToken, FatArrow, If, Semi};
use syn::visit::Visit;
use syn::{Expr, Item, Meta};

pub(super) fn affected_async_trait_item(item: &Item, syntax: &Syntax<'_>) -> bool {
    let attributes = match item {
        Item::Impl(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        _ => return false,
    };
    fn affected_attribute(meta: &Meta, syntax: &Syntax<'_>) -> bool {
        if meta.path().is_ident("cfg_attr") {
            let Meta::List(list) = meta else { return false };
            let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)
            else {
                return false;
            };
            if nested.first().is_some_and(|cfg| {
                crate::source_scan::cfg::truth_when_test_is_false(cfg)
                    == crate::source_scan::cfg::Truth::AlwaysFalse
            }) {
                return false;
            }
            return nested
                .iter()
                .skip(1)
                .any(|meta| affected_attribute(meta, syntax));
        }
        meta.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "async_trait")
            && syntax.symbols.is_audited_attribute(
                &meta
                    .path()
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>(),
                &syntax.logical_module,
                &syntax.lexical.aliases,
                &syntax.lexical.macros,
            )
    }
    if !attributes
        .iter()
        .any(|attribute| affected_attribute(&attribute.meta, syntax))
    {
        return false;
    }
    struct ReceiverMacros(bool);
    impl<'ast> Visit<'ast> for ReceiverMacros {
        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            self.0 |= has_self(mac.tokens.clone());
        }
    }
    let mut visitor = ReceiverMacros(false);
    visitor.visit_item(item);
    visitor.0
}

fn has_self(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident == "self",
        TokenTree::Group(group) => has_self(group.stream()),
        _ => false,
    })
}

pub(super) fn public_function_tokens(identity: (&str, &str), tokens: TokenStream) -> bool {
    match identity {
        ("tokio", "join" | "try_join") => (|input: ParseStream<'_>| {
            biased(input)?;
            let expressions: Punctuated<Expr, Comma> =
                input.parse_terminated(Expr::parse, Comma)?;
            if identity.1 == "try_join" && expressions.is_empty() {
                return Err(input.error("try_join requires a nonempty public expression list"));
            }
            Ok(())
        })
        .parse2(tokens)
        .is_ok(),
        ("tokio", "select") => syn::parse2::<Select>(tokens).is_ok(),
        ("tracing", "error" | "info" | "warn" | "info_span")
        | ("anyhow", "anyhow" | "bail")
        | ("serde_json", "json") => true,
        _ => false,
    }
}

fn biased(input: ParseStream<'_>) -> syn::Result<()> {
    let fork = input.fork();
    if fork
        .parse::<syn::Ident>()
        .is_ok_and(|ident| ident == "biased")
        && fork.peek(Semi)
    {
        let _: syn::Ident = input.parse()?;
        let _: Semi = input.parse()?;
    }
    Ok(())
}

struct Select;
impl Parse for Select {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        biased(input)?;
        let mut branches = 0usize;
        while !input.is_empty() {
            if input.peek(Else) {
                let _: Else = input.parse()?;
                let _: FatArrow = input.parse()?;
                let _: Expr = input.parse()?;
                if input.peek(Comma) {
                    let _: Comma = input.parse()?;
                }
                if !input.is_empty() {
                    return Err(input.error("else must be last"));
                }
                break;
            }
            let _ = syn::Pat::parse_multi_with_leading_vert(input)?;
            let _: EqToken = input.parse()?;
            let _: Expr = input.parse()?;
            if input.peek(Comma) {
                let _: Comma = input.parse()?;
                let _: If = input.parse()?;
                let _: Expr = input.parse()?;
            }
            let _: FatArrow = input.parse()?;
            let handler: Expr = input.parse()?;
            if input.peek(Comma) {
                let _: Comma = input.parse()?;
            } else if !input.is_empty() && !matches!(handler, Expr::Block(_)) {
                return Err(input.error("expected branch comma"));
            }
            branches += 1;
        }
        if !(1..=64).contains(&branches) {
            return Err(input.error("expected one to 64 public select branches"));
        }
        Ok(Self)
    }
}
