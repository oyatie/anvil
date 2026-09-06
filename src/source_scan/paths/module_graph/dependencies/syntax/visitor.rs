use std::path::PathBuf;

use syn::visit::{self, Visit};
use syn::{Block, Item, ItemMod, ItemUse, LitStr, Stmt};

use super::super::attrs;
use super::super::imports::ident_name;
use super::super::include_provenance::{IncludeProvenance, classify_include};
use super::{SourceInclude, SourceModule, Syntax};
use crate::source_scan::cfg::excludes_when_test_is_false;

impl<'ast> Visit<'ast> for Syntax<'_> {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let provenance = super::super::attribute_provenance::Provenance {
            symbols: self.symbols,
            scope: &self.logical_module,
            aliases: &self.lexical.aliases,
            macros: &self.lexical.macros,
        };
        if let Some(reason) = super::super::attribute_provenance::uncertainty(attribute, provenance)
        {
            self.uncertainties.push(reason);
            return;
        }
        match super::super::attribute_provenance::referenced_tokens(attribute, provenance) {
            Ok(tokens) => {
                for tokens in tokens {
                    self.record_reachable_macro_body(tokens);
                }
            }
            Err(reason) => self.uncertainties.push(reason),
        }
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if !excludes_when_test_is_false(attrs::expression(expression)) {
            visit::visit_expr(self, expression);
        }
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if !excludes_when_test_is_false(&arm.attrs) {
            visit::visit_arm(self, arm);
        }
    }

    fn visit_stmt(&mut self, statement: &'ast Stmt) {
        if matches!(statement, Stmt::Macro(item) if excludes_when_test_is_false(&item.attrs)) {
            return;
        }
        visit::visit_stmt(self, statement);
    }

    fn visit_item(&mut self, item: &'ast Item) {
        if !super::super::item_is_non_production(item) {
            if super::macro_contract::affected_async_trait_item(item, self) {
                self.uncertainties
                    .push("async_trait receiver rewrite affects a macro token tree".to_owned());
            }
            visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if !excludes_when_test_is_false(attrs::impl_item(item)) {
            visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !excludes_when_test_is_false(attrs::trait_item(item)) {
            visit::visit_trait_item(self, item);
        }
    }

    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) {
        if !excludes_when_test_is_false(attrs::foreign_item(item)) {
            visit::visit_foreign_item(self, item);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if !excludes_when_test_is_false(&local.attrs) {
            visit::visit_local(self, local);
        }
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if !excludes_when_test_is_false(&field.attrs) {
            visit::visit_field(self, field);
        }
    }

    fn visit_field_pat(&mut self, field: &'ast syn::FieldPat) {
        if !excludes_when_test_is_false(&field.attrs) {
            visit::visit_field_pat(self, field);
        }
    }

    fn visit_field_value(&mut self, field: &'ast syn::FieldValue) {
        if !excludes_when_test_is_false(&field.attrs) {
            visit::visit_field_value(self, field);
        }
    }

    fn visit_generic_param(&mut self, item: &'ast syn::GenericParam) {
        if !excludes_when_test_is_false(attrs::generic_param(item)) {
            visit::visit_generic_param(self, item);
        }
    }

    fn visit_pat(&mut self, pattern: &'ast syn::Pat) {
        if !excludes_when_test_is_false(attrs::pattern(pattern)) {
            visit::visit_pat(self, pattern);
        }
    }

    fn visit_receiver(&mut self, item: &'ast syn::Receiver) {
        if !excludes_when_test_is_false(&item.attrs) {
            visit::visit_receiver(self, item);
        }
    }

    fn visit_bare_fn_arg(&mut self, item: &'ast syn::BareFnArg) {
        if !excludes_when_test_is_false(&item.attrs) {
            visit::visit_bare_fn_arg(self, item);
        }
    }

    fn visit_bare_variadic(&mut self, item: &'ast syn::BareVariadic) {
        if !excludes_when_test_is_false(&item.attrs) {
            visit::visit_bare_variadic(self, item);
        }
    }

    fn visit_variadic(&mut self, item: &'ast syn::Variadic) {
        if !excludes_when_test_is_false(&item.attrs) {
            visit::visit_variadic(self, item);
        }
    }

    fn visit_variant(&mut self, item: &'ast syn::Variant) {
        if !excludes_when_test_is_false(&item.attrs) {
            visit::visit_variant(self, item);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        for attribute in &module.attrs {
            self.visit_attribute(attribute);
        }
        let context = self.lexical.module_child();
        // Rust item/type/value imports in a function block are not inherited
        // by a module declared in that block. Textual macro_rules scope is.
        self.modules.push(SourceModule {
            module: module.clone(),
            context,
            block_local: self.lexical.block_depth > 0,
        });
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        for attribute in &item.attrs {
            self.visit_attribute(attribute);
        }
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        for attribute in &item.attrs {
            self.visit_attribute(attribute);
        }
        // A named macro_rules definition is inert. Its body is measured when
        // a local invocation makes that expansion reachable. Item-position
        // invocations have no declared identifier and must still be visited.
        if item.ident.is_none() {
            visit::visit_item_macro(self, item);
        }
    }

    fn visit_block(&mut self, block: &'ast Block) {
        self.visit_scoped_block(block);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.record_path(path);
        visit::visit_path(self, path);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        match classify_include(
            mac,
            self.symbols,
            &self.logical_module,
            &self.lexical.aliases,
            &self.lexical.macros,
        ) {
            IncludeProvenance::Builtin => self.includes.push(SourceInclude {
                path: syn::parse2::<LitStr>(mac.tokens.clone())
                    .map(|literal| PathBuf::from(literal.value()))
                    .map_err(|_| mac.tokens.to_string()),
                context: self.lexical.clone(),
            }),
            IncludeProvenance::Ambiguous => self.uncertainties.push(format!(
                "cannot prove builtin include provenance for {}!",
                mac.path
                    .segments
                    .iter()
                    .map(|part| ident_name(&part.ident))
                    .collect::<Vec<_>>()
                    .join("::")
            )),
            IncludeProvenance::Other => {
                let path = mac
                    .path
                    .segments
                    .iter()
                    .map(|part| ident_name(&part.ident))
                    .collect::<Vec<_>>();
                let bodies =
                    self.symbols
                        .macro_bodies(&self.logical_module, &path, &self.lexical.aliases);
                let mut locally_measured = !bodies.is_empty();
                for body in bodies {
                    self.record_reachable_macro_body(body);
                }
                if path.len() == 1
                    && let Some(bodies) = self.lexical.macros.get(&path[0]).cloned()
                {
                    locally_measured = true;
                    for body in bodies {
                        self.record_reachable_macro_body(body);
                    }
                }
                if self
                    .symbols
                    .audited_function_macro(
                        &path,
                        &self.logical_module,
                        &self.lexical.aliases,
                        &self.lexical.macros,
                    )
                    .is_some_and(|(package, symbol)| {
                        super::macro_contract::public_function_tokens(
                            (&package, &symbol),
                            mac.tokens.clone(),
                        )
                    })
                {
                    locally_measured = true;
                }
                if !locally_measured && !known_unshadowed_builtin_macro(self, &path) {
                    self.uncertainties.push(format!(
                        "macro {}! has no locally measurable expansion",
                        path.join("::")
                    ));
                }
                // Arguments are Rust tokens too. An otherwise safe data macro
                // can contain a block-local module or nested include; treating
                // its token tree as inert omits compiled sources.
                self.record_reachable_macro_body(mac.tokens.clone());
            }
        }
    }
}

mod builtins;
pub(super) use builtins::known_unshadowed_builtin_macro;
