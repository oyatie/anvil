use super::Syntax;

pub(in crate::source_scan::paths::module_graph::dependencies::syntax) fn known_unshadowed_builtin_macro(
    syntax: &Syntax<'_>,
    path: &[String],
) -> bool {
    let [name] = path else { return false };
    const BUILTINS: &[&str] = &[
        "assert",
        "assert_eq",
        "assert_ne",
        "cfg",
        "column",
        "compile_error",
        "concat",
        "dbg",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "env",
        "eprint",
        "eprintln",
        "file",
        "format",
        "format_args",
        "include_bytes",
        "include_str",
        "line",
        "matches",
        "module_path",
        "option_env",
        "panic",
        "print",
        "println",
        "stringify",
        "thread_local",
        "todo",
        "unimplemented",
        "unreachable",
        "vec",
        "write",
        "writeln",
    ];
    BUILTINS.contains(&name.as_str())
        && !syntax.lexical.macros.contains_key(name)
        && !syntax.lexical.aliases.contains_key(name)
        && !syntax.lexical.aliases.contains_key("*")
        && !syntax.symbols.macro_shadowed(&syntax.logical_module, name)
        && !syntax.symbols.alias_declared(&syntax.logical_module, name)
        && !syntax.symbols.glob_imported(&syntax.logical_module)
        && !syntax.symbols.unknown_macro_prelude()
}
