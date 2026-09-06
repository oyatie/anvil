use std::collections::{BTreeMap, BTreeSet};

use syn::visit::{self, Visit};
use syn::{Block, Item, Stmt};

use super::Syntax;
use crate::source_scan::cfg::{
    Truth, availability_when_test_is_false, excludes_when_test_is_false,
};

impl Syntax<'_> {
    pub(super) fn visit_scoped_block<'ast>(&mut self, block: &'ast Block)
    where
        Self: Visit<'ast>,
    {
        let mut imports = Vec::new();
        let mut externs = BTreeMap::<String, Vec<Vec<String>>>::new();
        let mut definite_externs = BTreeSet::new();
        let inherited_macros = self.lexical.macros.clone();
        for statement in &block.stmts {
            if let Stmt::Item(Item::Use(item)) = statement
                && !excludes_when_test_is_false(&item.attrs)
            {
                super::super::imports::collect_imports(&item.tree, &mut Vec::new(), &mut imports);
            }
            if let Stmt::Item(Item::Macro(item)) = statement
                && !excludes_when_test_is_false(&item.attrs)
                && let Some(name) = &item.ident
            {
                let name = super::super::imports::ident_name(name);
                let bodies = self.lexical.macros.entry(name).or_default();
                let identity = item.mac.tokens.to_string();
                if !bodies.iter().any(|body| body.to_string() == identity) {
                    bodies.push(item.mac.tokens.clone());
                }
            }
            if let Stmt::Item(Item::ExternCrate(item)) = statement
                && !excludes_when_test_is_false(&item.attrs)
            {
                let binding = item
                    .rename
                    .as_ref()
                    .map(|(_, rename)| super::super::imports::ident_name(rename))
                    .unwrap_or_else(|| super::super::imports::ident_name(&item.ident));
                let target = if item.ident == "self" {
                    vec!["crate".to_owned()]
                } else {
                    vec![super::super::imports::ident_name(&item.ident)]
                };
                if availability_when_test_is_false(&item.attrs) == Truth::AlwaysTrue {
                    definite_externs.insert(binding.clone());
                }
                externs.entry(binding).or_default().push(target);
            }
        }
        let inherited = self.lexical.aliases.clone();
        self.install_block_modules(block);
        for binding in &definite_externs {
            self.lexical.aliases.remove(binding);
            self.lexical
                .aliases
                .entry(binding.clone())
                .or_default()
                .push(vec!["@shadow".to_owned()]);
        }
        self.install_imports(&imports, true);
        for (binding, targets) in externs {
            let aliases = self.lexical.aliases.entry(binding).or_default();
            for target in targets {
                if !aliases.contains(&target) {
                    aliases.push(target);
                }
            }
        }
        self.lexical.block_depth += 1;
        visit::visit_block(self, block);
        self.lexical.block_depth -= 1;
        self.lexical.aliases = inherited;
        self.lexical.macros = inherited_macros;
    }

    fn install_block_modules(&mut self, block: &Block) {
        for statement in &block.stmts {
            if let Stmt::Item(Item::Mod(module)) = statement
                && availability_when_test_is_false(&module.attrs) == Truth::AlwaysTrue
            {
                let name = super::super::imports::ident_name(&module.ident);
                self.lexical
                    .aliases
                    .entry(name)
                    .or_default()
                    .push(vec!["@shadow".to_owned()]);
            }
        }
    }
}
