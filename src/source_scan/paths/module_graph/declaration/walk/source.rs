use super::{
    LexicalContext, Roles, Seen, Symbols, modules, scan_expression_for_classification, visit_items,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub(super) fn parse_and_visit(
    path: &Path,
    repo: &Path,
    inherited_test: bool,
    allow_expression: bool,
    scope: &[String],
    context: &Path,
    path_context: &Path,
    lexical: LexicalContext,
    symbols: &Symbols,
    seen: &mut BTreeSet<Seen>,
    active: &mut BTreeSet<PathBuf>,
    roles: &mut BTreeMap<PathBuf, Roles>,
) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read declared source {}: {error}", path.display()))?;
    if let Ok(parsed) = syn::parse_file(&source) {
        return visit_items(
            &parsed.items,
            path,
            repo,
            context,
            path_context,
            scope,
            inherited_test,
            lexical,
            symbols,
            seen,
            active,
            roles,
        );
    }
    if !allow_expression {
        return Err(format!(
            "declared module {} is not an item source",
            path.display()
        ));
    }
    let expression = syn::parse_str::<syn::Expr>(&source).map_err(|error| {
        format!(
            "included Rust {} is neither an item source nor an expression: {error}",
            path.display()
        )
    })?;
    let (syntax, _) = scan_expression_for_classification(&expression, scope, symbols, lexical);
    modules::visit_syntax(
        syntax.modules,
        syntax.includes,
        path,
        repo,
        context,
        path_context,
        scope,
        inherited_test,
        symbols,
        seen,
        active,
        roles,
    )
}
