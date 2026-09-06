//! Conservative classification of Rust that cannot exist when `test = false`.

use syn::punctuated::Punctuated;
use syn::{Attribute as SynAttribute, Meta as SynMeta};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Truth {
    AlwaysFalse,
    Maybe,
    AlwaysTrue,
}

/// Whether these attributes make their target unavailable in every non-test
/// compilation.
///
/// Target, feature, and user-defined cfg atoms stay unknown. That keeps
/// `cfg(any(test, unix))` in the production corpus while proving that
/// `cfg(all(test, unix))` cannot compile when `test` is false.
pub(crate) fn excludes_when_test_is_false(attributes: &[SynAttribute]) -> bool {
    availability_when_test_is_false(attributes) == Truth::AlwaysFalse
}

/// Whether a declaration exists in all, some, or no non-test configurations.
/// Alias analysis needs the distinction between an unconditional shadow and a
/// target-specific possible shadow; collapsing both to "not excluded" lets a
/// `#[cfg(windows)] use missing::Runner` erase an inherited Unix binding.
pub fn availability_when_test_is_false(attributes: &[SynAttribute]) -> Truth {
    conjunction(attributes.iter().map(attribute_availability))
}

fn attribute_availability(attribute: &SynAttribute) -> Truth {
    if attribute.path().is_ident("test") {
        return Truth::AlwaysFalse;
    }
    if attribute.path().is_ident("cfg") {
        return attribute
            .parse_args::<SynMeta>()
            .map_or(Truth::Maybe, |predicate| {
                truth_when_test_is_false(&predicate)
            });
    }
    if !attribute.path().is_ident("cfg_attr") {
        return Truth::AlwaysTrue;
    }
    cfg_attr_availability(attribute.meta.require_list().ok())
}

fn cfg_attr_availability(list: Option<&syn::MetaList>) -> Truth {
    let Some(list) = list else {
        return Truth::Maybe;
    };
    let parser = Punctuated::<SynMeta, syn::token::Comma>::parse_terminated;
    let Ok(parts) = syn::parse::Parser::parse2(parser, list.tokens.clone()) else {
        return Truth::Maybe;
    };
    let mut parts = parts.iter();
    let Some(condition) = parts.next() else {
        return Truth::Maybe;
    };
    let applied = conjunction(parts.map(meta_availability));
    match (truth_when_test_is_false(condition), applied) {
        (Truth::AlwaysTrue, value) => value,
        (Truth::AlwaysFalse, _) | (_, Truth::AlwaysTrue) => Truth::AlwaysTrue,
        (Truth::Maybe, Truth::AlwaysFalse | Truth::Maybe) => Truth::Maybe,
    }
}

fn meta_availability(meta: &SynMeta) -> Truth {
    let SynMeta::List(list) = meta else {
        return Truth::AlwaysTrue;
    };
    if list.path.is_ident("cfg") {
        return syn::parse2::<SynMeta>(list.tokens.clone()).map_or(Truth::Maybe, |predicate| {
            truth_when_test_is_false(&predicate)
        });
    }
    if list.path.is_ident("cfg_attr") {
        return cfg_attr_availability(Some(list));
    }
    Truth::AlwaysTrue
}

pub(crate) fn truth_when_test_is_false(meta: &SynMeta) -> Truth {
    match meta {
        SynMeta::Path(path) if path.is_ident("test") => Truth::AlwaysFalse,
        SynMeta::Path(_) | SynMeta::NameValue(_) => Truth::Maybe,
        SynMeta::List(list) => {
            let parser = Punctuated::<SynMeta, syn::token::Comma>::parse_terminated;
            let Ok(nested) = syn::parse::Parser::parse2(parser, list.tokens.clone()) else {
                return Truth::Maybe;
            };
            if list.path.is_ident("all") {
                return conjunction(nested.iter().map(truth_when_test_is_false));
            }
            if list.path.is_ident("any") {
                return disjunction(nested.iter().map(truth_when_test_is_false));
            }
            if list.path.is_ident("not") && nested.len() == 1 {
                return match truth_when_test_is_false(&nested[0]) {
                    Truth::AlwaysFalse => Truth::AlwaysTrue,
                    Truth::Maybe => Truth::Maybe,
                    Truth::AlwaysTrue => Truth::AlwaysFalse,
                };
            }
            Truth::Maybe
        }
    }
}

fn conjunction(values: impl Iterator<Item = Truth>) -> Truth {
    let mut saw_maybe = false;
    for value in values {
        match value {
            Truth::AlwaysFalse => return Truth::AlwaysFalse,
            Truth::Maybe => saw_maybe = true,
            Truth::AlwaysTrue => {}
        }
    }
    if saw_maybe {
        Truth::Maybe
    } else {
        Truth::AlwaysTrue
    }
}

fn disjunction(values: impl Iterator<Item = Truth>) -> Truth {
    let mut saw_maybe = false;
    for value in values {
        match value {
            Truth::AlwaysTrue => return Truth::AlwaysTrue,
            Truth::Maybe => saw_maybe = true,
            Truth::AlwaysFalse => {}
        }
    }
    if saw_maybe {
        Truth::Maybe
    } else {
        Truth::AlwaysFalse
    }
}

#[cfg(test)]
mod tests {
    use super::excludes_when_test_is_false;

    fn classified(attribute: &str) -> bool {
        let item: syn::ItemMod = syn::parse_str(&format!("{attribute} mod fixture {{}}")).unwrap();
        excludes_when_test_is_false(&item.attrs)
    }

    #[test]
    fn proves_only_predicates_that_cannot_be_true_without_test() {
        for attribute in [
            "#[cfg(test)]",
            "#[cfg(all(test, unix))]",
            "#[cfg(not(not(test)))]",
            "#[cfg(any())]",
            "#[cfg_attr(not(test), cfg(test))]",
        ] {
            assert!(classified(attribute), "{attribute} can never ship");
        }
        for attribute in [
            "#[cfg(unix)]",
            "#[cfg(any(test, unix))]",
            "#[cfg(all(not(test), unix))]",
            "#[cfg_attr(test, cfg(test))]",
            "#[cfg_attr(unix, cfg(test))]",
        ] {
            assert!(!classified(attribute), "{attribute} might ship");
        }
    }

    #[test]
    fn rust_test_functions_are_not_production_items() {
        let item: syn::ItemFn = syn::parse_str("#[test] fn fixture() {}").unwrap();
        assert!(excludes_when_test_is_false(&item.attrs));
    }
}
