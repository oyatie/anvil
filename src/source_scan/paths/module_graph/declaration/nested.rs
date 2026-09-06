use std::path::PathBuf;

use syn::visit::{self, Visit};

#[derive(Default)]
pub(super) struct NestedSources {
    pub(super) modules: Vec<syn::ItemMod>,
    pub(super) includes: Vec<Result<PathBuf, String>>,
}

impl NestedSources {
    pub(super) fn in_test_item(item: &syn::Item) -> Self {
        let mut found = Self::default();
        visit::visit_item(&mut found, item);
        found
    }
}

impl<'ast> Visit<'ast> for NestedSources {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if !super::super::dependencies::item_is_non_production(item) {
            visit::visit_item(self, item);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        // The caller walks this module with the correct logical and physical
        // contexts. Recursing here would flatten its descendants into the
        // containing module's lookup directory.
        self.modules.push(module.clone());
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if !crate::source_scan::cfg::excludes_when_test_is_false(
            super::super::dependencies::attrs::expression(expression),
        ) {
            visit::visit_expr(self, expression);
        }
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let is_include = invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "include");
        if is_include {
            self.includes.push(
                syn::parse2::<syn::LitStr>(invocation.tokens.clone())
                    .map(|literal| PathBuf::from(literal.value()))
                    .map_err(|error| error.to_string()),
            );
        } else {
            visit::visit_macro(self, invocation);
        }
    }
}
