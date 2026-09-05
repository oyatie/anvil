use std::fs;
use std::path::Path;

use syn::visit::Visit;

use super::Collector;
use crate::source_scan::paths::module_graph::dependencies::contained_source;
use crate::source_scan::paths::module_graph::dependencies::{LexicalContext, Syntax};

impl Collector<'_> {
    pub(super) fn included(
        &mut self,
        path: &Path,
        scope: &[String],
        _context: &Path,
        lexical: LexicalContext,
    ) -> Result<(), String> {
        let canonical = contained_source(path, self.repo_root)?;
        // Logical scope is inherited, physical module lookup is not.
        let context = path.parent().unwrap_or(Path::new("."));
        if !self.active_files.insert(canonical.clone()) {
            return Err(format!(
                "production include cycle reaches {}",
                canonical.display()
            ));
        }
        let result = self.measure_included(path, scope, context, lexical, &canonical);
        self.active_files.remove(&canonical);
        result
    }

    fn measure_included(
        &mut self,
        path: &Path,
        scope: &[String],
        context: &Path,
        lexical: LexicalContext,
        canonical: &Path,
    ) -> Result<(), String> {
        let identity = (
            scope.to_vec(),
            canonical.to_path_buf(),
            context.to_path_buf(),
            lexical.identity(),
        );
        if !self.seen.insert(identity) {
            return Ok(());
        }
        let source = fs::read_to_string(path)
            .map_err(|error| format!("cannot read included source {}: {error}", path.display()))?;
        if let Ok(file) = syn::parse_file(&source) {
            return self.items(
                &file.items,
                path,
                scope,
                context,
                path.parent().unwrap_or(Path::new(".")),
                lexical,
            );
        }
        let expression = syn::parse_str::<syn::Expr>(&source).map_err(|error| {
            format!(
                "included Rust {} is neither an item source nor an expression: {error}",
                path.display()
            )
        })?;
        let mut syntax = Syntax::with_context(scope, &self.symbols, lexical);
        syntax.visit_expr(&expression);
        if let Some(reason) = syntax.uncertainties.first() {
            if self.strict {
                return Err(format!(
                    "production syntax in {} cannot be measured: {reason}",
                    path.display()
                ));
            }
            self.complete = false;
        }
        let includes = std::mem::take(&mut syntax.includes);
        let modules = std::mem::take(&mut syntax.modules);
        drop(syntax);
        for include in includes {
            let relative = include.path.map_err(|tokens| {
                format!(
                    "dynamic source include in {} cannot be measured: include!({tokens})",
                    path.display()
                )
            })?;
            let nested = path.parent().unwrap_or(Path::new(".")).join(relative);
            self.included(&nested, scope, context, include.context)?;
        }
        self.local_modules(
            modules,
            path,
            scope,
            context,
            path.parent().unwrap_or(Path::new(".")),
        )
    }
}
