use std::str::FromStr;

use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{Expr, Meta};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Serde,
    Clap,
    Tokio,
    AsyncTrait,
}

pub(super) fn analyze(meta: &Meta, kind: Kind) -> Result<Vec<proc_macro2::TokenStream>, String> {
    let Meta::List(list) = meta else {
        return Ok(Vec::new());
    };
    if matches!(kind, Kind::AsyncTrait) {
        let arguments = list.tokens.to_string();
        return match arguments.as_str() {
            "" | "? Send" => Ok(Vec::new()),
            _ => Err(format!(
                "async_trait arguments `{arguments}` have no audited expansion"
            )),
        };
    }
    let options = list
        .parse_args_with(Punctuated::<Meta, syn::token::Comma>::parse_terminated)
        .map_err(|error| format!("helper attribute options cannot be measured: {error}"))?;
    let mut tokens = Vec::new();
    for option in &options {
        collect(option, kind, None, &mut tokens)?;
    }
    Ok(tokens)
}

fn collect(
    option: &Meta,
    kind: Kind,
    parent: Option<&str>,
    tokens: &mut Vec<proc_macro2::TokenStream>,
) -> Result<(), String> {
    match option {
        Meta::NameValue(value)
            if matches!(kind, Kind::Serde | Kind::Tokio) && value.path.is_ident("crate") =>
        {
            Err("custom macro crate redirects have no admitted source identity".to_owned())
        }
        Meta::Path(path) if matches!(kind, Kind::Tokio) => Err(format!(
            "tokio attribute option `{}` has no audited expansion",
            path_name(path)
        )),
        Meta::Path(_) => Ok(()),
        Meta::NameValue(value)
            if matches!(kind, Kind::Tokio)
                && safe_data_key(kind, parent, &path_name(&value.path)) =>
        {
            let valid = match (&value.value, path_name(&value.path).as_str()) {
                (Expr::Lit(literal), "worker_threads") => matches!(literal.lit, syn::Lit::Int(_)),
                (Expr::Lit(literal), "start_paused") => matches!(literal.lit, syn::Lit::Bool(_)),
                (Expr::Lit(literal), "flavor" | "unhandled_panic") => {
                    matches!(literal.lit, syn::Lit::Str(_))
                }
                _ => false,
            };
            valid
                .then_some(())
                .ok_or_else(|| "Tokio runtime knobs must be scalar literals".to_owned())
        }
        Meta::NameValue(value) if safe_data_key(kind, parent, &path_name(&value.path)) => Ok(()),
        Meta::NameValue(value) if matches!(kind, Kind::Tokio) && !value.path.is_ident("crate") => {
            Err(format!(
                "tokio attribute option `{}` has no audited expansion",
                path_name(&value.path)
            ))
        }
        Meta::NameValue(value) => {
            if matches!(kind, Kind::Clap) {
                // Clap's audited helper grammar accepts raw Rust expressions
                // for values such as `about`, `help`, and defaults. A string
                // literal is inert as an Expr; a caller path remains visible.
                tokens.push(value.value.to_token_stream());
            } else if let Expr::Lit(literal) = &value.value
                && let syn::Lit::Str(string) = &literal.lit
            {
                let parsed = proc_macro2::TokenStream::from_str(&string.value()).map_err(|error| {
                    format!(
                        "helper attribute path/type expression {:?} cannot be measured: {error}",
                        string.value()
                    )
                })?;
                tokens.push(parsed);
            } else {
                tokens.push(value.value.to_token_stream());
            }
            Ok(())
        }
        Meta::List(list) => {
            if matches!(kind, Kind::Clap) {
                let expressions = list
                    .parse_args_with(Punctuated::<Expr, syn::token::Comma>::parse_terminated)
                    .map_err(|error| {
                        format!("Clap method arguments cannot be measured: {error}")
                    })?;
                tokens.extend(expressions.iter().map(ToTokens::to_token_stream));
                return Ok(());
            }
            if matches!(kind, Kind::Tokio) {
                return Err(format!(
                    "nested tokio attribute option `{}` has no audited expansion",
                    path_name(&list.path)
                ));
            }
            let list_name = path_name(&list.path);
            let nested = list
                .parse_args_with(Punctuated::<Meta, syn::token::Comma>::parse_terminated)
                .map_err(|error| {
                    format!(
                        "nested helper attribute {} cannot be measured: {error}",
                        path_name(&list.path)
                    )
                })?;
            for option in &nested {
                collect(option, kind, Some(&list_name), tokens)?;
            }
            Ok(())
        }
    }
}

fn safe_data_key(kind: Kind, parent: Option<&str>, key: &str) -> bool {
    match kind {
        Kind::Serde => {
            matches!(
                key,
                "rename" | "rename_all" | "alias" | "tag" | "content" | "expecting"
            ) || (matches!(parent, Some("rename" | "rename_all"))
                && matches!(key, "serialize" | "deserialize"))
        }
        Kind::Clap => false,
        Kind::Tokio => matches!(
            key,
            "flavor" | "worker_threads" | "start_paused" | "unhandled_panic"
        ),
        Kind::AsyncTrait => false,
    }
}

fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_crate_redirects_and_non_scalar_tokio_knobs_are_not_admitted() {
        for (source, kind) in [
            ("serde(crate = \"other\")", Kind::Serde),
            ("tokio::main(crate = \"other\")", Kind::Tokio),
            ("tokio::main(worker_threads = threads())", Kind::Tokio),
        ] {
            assert!(
                analyze(&syn::parse_str(source).unwrap(), kind).is_err(),
                "{source}"
            );
        }
        assert!(
            analyze(
                &syn::parse_str("tokio::main(flavor = \"current_thread\")").unwrap(),
                Kind::Tokio
            )
            .is_ok()
        );
    }

    #[test]
    fn clap_method_arguments_remain_traversable_rust_expressions() {
        let tokens = analyze(
            &syn::parse_str("arg(value_parser(factory()), num_args(1..=3))").unwrap(),
            Kind::Clap,
        )
        .unwrap();
        assert_eq!(
            tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["factory ()", "1 ..= 3"]
        );
    }
}
