//! Structural ratchets for the typed model-command boundary.
//!
//! The typed API and definition-resolved lint own the active-build guarantee:
//! only `exec::agent::provider` can mutate `AgentCommand`, and the public model
//! transport accepts no raw command or raw prompt. A Rust-token-aware census
//! additionally covers inactive platform/feature code and keeps the few raw
//! subprocess seams finite.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::Visit;

const PROVIDER_SEAM: &str = "src/exec/agent/provider.rs";
const MODEL_TRANSPORT: &str = "src/exec/agent/transport.rs";
const NON_MODEL_TRANSPORT: &str = "src/exec/non_model.rs";
const CLIPPY_CONFIG: &str = "clippy.toml";
const DIRECT_EXECUTION_SEAMS: &[&str] = &[
    "src/exec/agent/transport.rs",
    "src/exec/non_model/transport.rs",
    "src/exec/replacement.rs",
];
const EXPECTED_EXECUTION_SITES: &[(&str, &str, &str)] = &[
    (
        "src/exec/agent/transport.rs",
        "deliver_with_stdin",
        "method:command:spawn",
    ),
    (
        "src/exec/agent/transport.rs",
        "probe",
        "method:command:output",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_for",
        "method:command:output",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_status",
        "method:command:status",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_sync_bounded",
        "method:command:spawn",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_with_stdin",
        "method:command:spawn",
    ),
    ("src/exec/replacement.rs", "spawn", "method:command:spawn"),
];
const EXPECTED_LINT_EXPECTATIONS: &[(&str, &str)] = &[
    ("src/exec/agent/transport.rs", "deliver_with_stdin"),
    ("src/exec/agent/transport.rs", "probe"),
    ("src/exec/non_model/transport.rs", "run_for"),
    ("src/exec/non_model/transport.rs", "run_status"),
    ("src/exec/non_model/transport.rs", "run_sync_bounded"),
    ("src/exec/non_model/transport.rs", "run_with_stdin"),
    ("src/exec/replacement.rs", "spawn"),
];
const EXPECTED_AGENT_CAPABILITY_EVENTS: &[(&str, &str, &str)] = &[
    (
        "src/exec/agent.rs",
        "",
        "import:tokio::process::Command->Command",
    ),
    ("src/exec/agent.rs", "args", "argv:self.command:args"),
    ("src/exec/agent.rs", "args", "raw-field:self.command"),
    ("src/exec/agent.rs", "command", "call:command_in:tool"),
    (
        "src/exec/agent.rs",
        "command_in",
        "bind-command-new:cmd:Command::new:tool",
    ),
    (
        "src/exec/agent.rs",
        "command_in",
        "command-new:Command::new:tool",
    ),
    (
        "src/exec/agent.rs",
        "command_in",
        "construct-agent:AgentCommand:command=cmd:framing=framing",
    ),
    ("src/exec/agent.rs", "command_in", "mut-ref:cmd"),
    ("src/exec/agent.rs", "deliver", "reference:command"),
    (
        "src/exec/agent/provider.rs",
        "",
        "import:super::AgentCommand->AgentCommand",
    ),
    (
        "src/exec/agent/provider.rs",
        "",
        "import:super::ProviderProbeCommand->ProviderProbeCommand",
    ),
    (
        "src/exec/agent/provider.rs",
        "agy_help_probe",
        "argv:command:arg",
    ),
    (
        "src/exec/agent/provider.rs",
        "agy_help_probe",
        "bind-command-new:command:tokio::process::Command::new:str:\"agy\"",
    ),
    (
        "src/exec/agent/provider.rs",
        "agy_help_probe",
        "command-new:tokio::process::Command::new:str:\"agy\"",
    ),
    (
        "src/exec/agent/provider.rs",
        "agy_help_probe",
        "construct-probe:ProviderProbeCommand",
    ),
    ("src/exec/agent/provider.rs", "agy_agent", "argv:cmd:args"),
    ("src/exec/agent/provider.rs", "agy_agent", "argv:cmd:args"),
    (
        "src/exec/agent/provider.rs",
        "agy_agent",
        "call:super::command:str:\"agy\"",
    ),
    (
        "src/exec/agent/provider.rs",
        "claude_agent",
        "argv:cmd:args",
    ),
    (
        "src/exec/agent/provider.rs",
        "claude_agent",
        "call:super::command:str:\"claude\"",
    ),
    ("src/exec/agent/provider.rs", "codex_agent", "argv:cmd:args"),
    (
        "src/exec/agent/provider.rs",
        "codex_agent",
        "call:super::command:str:\"codex\"",
    ),
    (
        "src/exec/agent/provider.rs",
        "cursor_agent",
        "argv:cmd:args",
    ),
    (
        "src/exec/agent/provider.rs",
        "cursor_agent",
        "argv:cmd:args",
    ),
    (
        "src/exec/agent/provider.rs",
        "cursor_agent",
        "call:super::command:str:\"cursor\"",
    ),
    ("src/exec/agent/provider.rs", "grok_agent", "argv:cmd:args"),
    (
        "src/exec/agent/provider.rs",
        "grok_agent",
        "call:super::command:str:\"grok\"",
    ),
    (
        "src/exec/agent/transport.rs",
        "",
        "import:super::AgentCommand->AgentCommand",
    ),
    (
        "src/exec/agent/transport.rs",
        "",
        "import:super::ProviderProbeCommand->ProviderProbeCommand",
    ),
    (
        "src/exec/agent/transport.rs",
        "",
        "import:tokio::process::Command->Command",
    ),
    (
        "src/exec/agent/transport.rs",
        "deliver",
        "destructure-agent:AgentCommand",
    ),
    (
        "src/exec/agent/transport.rs",
        "probe",
        "destructure-probe:ProviderProbeCommand",
    ),
];
const EXPECTED_NONMODEL_CAPABILITY_EVENTS: &[(&str, &str, &str)] = &[
    (
        "src/exec/non_model.rs",
        "checked",
        "construct-self:NonModelCommand",
    ),
    (
        "src/exec/non_model.rs",
        "checked",
        "construct-self:SyncNonModelCommand",
    ),
    (
        "src/exec/non_model.rs",
        "run",
        "call:NonModelCommand::checked",
    ),
    (
        "src/exec/non_model.rs",
        "run_for",
        "call:NonModelCommand::checked",
    ),
    (
        "src/exec/non_model.rs",
        "run_status",
        "call:NonModelCommand::checked",
    ),
    (
        "src/exec/non_model.rs",
        "run_sync_bounded",
        "call:SyncNonModelCommand::checked",
    ),
    (
        "src/exec/non_model.rs",
        "run_with_stdin",
        "call:NonModelCommand::checked",
    ),
    (
        "src/exec/non_model/transport.rs",
        "",
        "import:super::NonModelCommand->NonModelCommand",
    ),
    (
        "src/exec/non_model/transport.rs",
        "",
        "import:super::SyncNonModelCommand->SyncNonModelCommand",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_for",
        "destructure:NonModelCommand",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_status",
        "destructure:NonModelCommand",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_sync_bounded",
        "destructure:SyncNonModelCommand",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_with_stdin",
        "destructure:NonModelCommand",
    ),
];
const EXPECTED_SAFE_ASSOCIATED_SPAWNS: &[(&str, &str, &str)] = &[
    (
        "src/cli/server.rs",
        "run_server",
        "crate::cli::sweep_task::spawn",
    ),
    ("src/cli/server.rs", "run_server", "tokio::spawn"),
    ("src/cli/server.rs", "run_server", "tokio::spawn"),
    ("src/cli/server.rs", "run_server", "tokio::spawn"),
    ("src/cli/server.rs", "run_server", "tokio::spawn"),
    ("src/cli/server.rs", "run_server", "tokio::spawn"),
    ("src/cli/server.rs", "run_server", "tokio::spawn"),
    ("src/cli/server.rs", "start_forwarders", "tokio::spawn"),
    ("src/cli/sweep_task.rs", "spawn", "tokio::spawn"),
    (
        "src/exec/mod.rs",
        "spawn_replacement_binary",
        "replacement::spawn",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_sync_bounded",
        "std::thread::spawn",
    ),
    (
        "src/exec/non_model/transport.rs",
        "run_sync_bounded",
        "std::thread::spawn",
    ),
    (
        "src/fleet_observer/mod.rs",
        "spawn_continuous_poller",
        "tokio::spawn",
    ),
    (
        "src/self_governance/mod.rs",
        "spawn_monitoring_daemon",
        "tokio::spawn",
    ),
    (
        "src/watchdog/mod.rs",
        "run_with_adaptive_watchdog",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "drain_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_certify_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_enlist_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_fix_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_heal_queue_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_reconcile_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_review_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/manual_handlers.rs",
        "manual_triage_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/webhook_handlers.rs",
        "webhook_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/webhook_handlers.rs",
        "webhook_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/webhook_handlers.rs",
        "webhook_handler",
        "tokio::spawn",
    ),
    (
        "src/webhook/webhook_handlers.rs",
        "webhook_handler",
        "tokio::spawn",
    ),
];
const PROCESS_METHODS: &[&str] = &[".exec()", ".output()", ".spawn()", ".status()"];
const PROCESS_METHOD_NAMES: &[&str] = &["exec", "output", "spawn", "status"];
const DISALLOWED_PROCESS_METHODS: &[&str] = &[
    "std::process::Command::output",
    "std::process::Command::spawn",
    "std::process::Command::status",
    "tokio::process::Command::output",
    "tokio::process::Command::spawn",
    "tokio::process::Command::status",
    "std::os::unix::process::CommandExt::exec",
];
const PROCESS_LINT_NAMES: &[&str] = &[
    "clippy::disallowed_methods",
    "clippy::all",
    "clippy::style",
    "warnings",
];
const SAFE_ASSOCIATED_SPAWNS: &[&str] = &[
    "tokio::spawn",
    "std::thread::spawn",
    "crate::cli::sweep_task::spawn",
    "replacement::spawn",
];
const SAFE_DATA_MACROS: &[&str] = &[
    "anyhow",
    "anyhow::anyhow",
    "anyhow::bail",
    "assert",
    "assert_eq",
    "assert_ne",
    "bail",
    "concat",
    "debug_assert",
    "env",
    "error",
    "format",
    "include_str",
    "info",
    "matches",
    "panic",
    "print",
    "println",
    "serde_json::json",
    "tracing::error",
    "tracing::info",
    "tracing::info_span",
    "tracing::warn",
    "unreachable",
    "vec",
    "warn",
    "write",
    "writeln",
];
const SAFE_CODE_MACROS: &[&str] = &["sqlx::query", "tokio::join", "tokio::select"];
const APPROVED_SAFE_MACRO_IMPORTS: &[&str] = &[
    "anyhow::anyhow",
    "anyhow::bail",
    // `std::env` is a module imported in the type namespace; its exact,
    // trusted source cannot replace the built-in `env!` macro.
    "std::env",
    "tracing::error",
    "tracing::info",
    "tracing::warn",
];
const APPROVED_ATTRIBUTES: &[&str] = &[
    "allow",
    "arg",
    "async_trait",
    "cfg",
    "cfg_attr",
    "command",
    "default",
    "deny",
    "deprecated",
    "derive",
    "doc",
    "expect",
    "forbid",
    "ignore",
    "macro_use",
    "must_use",
    "path",
    "repr",
    "serde",
    "test",
    "tokio::main",
    "tokio::test",
    "tracing::instrument",
    "warn",
];
const APPROVED_DERIVES: &[&str] = &[
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Deserialize",
    "Eq",
    "Hash",
    "Ord",
    "Parser",
    "PartialEq",
    "PartialOrd",
    "Serialize",
    "Subcommand",
    "serde::Deserialize",
    "serde::Serialize",
];
const APPROVED_ATTRIBUTE_IMPORTS: &[&str] = &["async_trait::async_trait", "tracing::instrument"];
const APPROVED_DERIVE_IMPORTS: &[&str] = &[
    "clap::Parser",
    "clap::Subcommand",
    "serde::Deserialize",
    "serde::Serialize",
];
const APPROVED_LOCAL_GLOB_IMPORTS: &[&str] = &["manual_handlers", "super"];
const RESERVED_TRUSTED_PATH_ROOTS: &[&str] = &[
    "anyhow",
    "arg",
    "async_trait",
    "command",
    "default",
    "manual_handlers",
    "replacement",
    "serde",
    "serde_json",
    "sqlx",
    "std",
    "tokio",
    "tracing",
];
const KNOWN_PROVIDERS: &[&str] = &[
    "agy",
    "claude",
    "codex",
    "cursor",
    "cursor-agent",
    "gemini",
    "grok",
];
const EXPECTED_RAW_STDIN_REFERENCES: &[(&str, &str)] = &[
    ("src/cedar_guard.rs", "check_parse"),
    ("src/ci_triager/publication.rs", "create_issue"),
    ("src/shape/adapters/git_tree_at_rev.rs", "read_batch"),
];
const NON_MODEL_PROGRAM_VOCABULARY: &[&str] = &[
    "cargo", "cedar", "curl", "echo", "gh", "git", "go", "node", "npm", "ps", "python3", "sleep",
];

/// This is intentionally an associated-function alias through a renamed type,
/// with a contributor-selected executable and no `.output()` spelling. Clippy
/// must resolve the method definition to fulfil the expectation. Removing the
/// Tokio `disallowed-methods` entry therefore makes `cargo clippy --all-targets
/// -- -D warnings` fail with an unfulfilled lint expectation.
#[allow(dead_code)]
#[expect(
    clippy::disallowed_methods,
    reason = "compile-time adversarial seed for the process-execution boundary"
)]
fn associated_function_alias_clippy_seed(executable: &std::ffi::OsStr) {
    type Process = tokio::process::Command;
    let mut command = Process::new(executable);
    let invoke = Process::output;
    let _future = invoke(&mut command);
}

#[allow(dead_code)]
#[r#expect(
    clippy::disallowed_methods,
    reason = "compile-time raw-identifier seed for the process boundary"
)]
fn raw_identifier_alias_clippy_seed(executable: &std::ffi::OsStr) {
    type Process = tokio::process::Command;
    let mut command = Process::new(executable);
    let invoke = Process::r#output;
    let _future = invoke(&mut command);
}

#[cfg(unix)]
#[allow(dead_code)]
#[expect(
    clippy::disallowed_methods,
    reason = "compile-time adversarial seed for CommandExt::exec"
)]
fn command_ext_alias_clippy_seed(executable: &std::ffi::OsStr) {
    use std::os::unix::process::CommandExt;

    let mut command = std::process::Command::new(executable);
    let invoke = CommandExt::exec;
    let _error = invoke(&mut command);
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn rust_sources(repository: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "production Rust census cannot read {}: {error}",
            dir.display()
        )
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "production Rust census cannot read an entry under {}: {error}",
                dir.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!(
                "production Rust census cannot classify {}: {error}",
                path.display()
            )
        });
        if file_type.is_symlink() {
            panic!(
                "production Rust census refuses in-repository symlink {}",
                path.display()
            );
        }
        if path.is_dir() {
            if path == repository.join(".git") || path == repository.join("target") {
                continue;
            }
            rust_sources(repository, &path, out);
        } else if let Some(extension) = path.extension().and_then(|ext| ext.to_str())
            && extension.eq_ignore_ascii_case("rs")
        {
            assert_eq!(
                extension,
                "rs",
                "production Rust census refuses a case-folding source extension at {}",
                path.display()
            );
            out.push(path);
        }
    }
}

fn production(path: &Path) -> String {
    let source = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "production source census cannot read {}: {error}",
            path.display()
        )
    });
    anvil::source_scan::without_commentary(&source)
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn normalized_ident(ident: &proc_macro2::Ident) -> String {
    let ident = ident.to_string();
    ident.strip_prefix("r#").unwrap_or(&ident).to_owned()
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProductionTarget {
    path: PathBuf,
    custom_build: bool,
}

fn cargo_metadata() -> &'static serde_json::Value {
    static METADATA: OnceLock<serde_json::Value> = OnceLock::new();
    METADATA.get_or_init(|| {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = std::process::Command::new(cargo)
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--manifest-path",
            ])
            .arg(repo().join("Cargo.toml"))
            .output()
            .expect("run cargo metadata for the production-target census");
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata")
    })
}

fn targets_from_metadata(metadata: &serde_json::Value, repository: &Path) -> Vec<ProductionTarget> {
    let production_kinds = [
        "bin",
        "cdylib",
        "custom-build",
        "dylib",
        "lib",
        "proc-macro",
        "rlib",
        "staticlib",
    ];
    let mut roots = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|package| {
            package["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path).starts_with(repository))
        })
        .flat_map(|package| {
            package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|target| {
                    let kinds = target["kind"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>();
                    if !kinds.iter().any(|kind| production_kinds.contains(kind)) {
                        return None;
                    }
                    target["src_path"].as_str().map(|path| {
                        let path = PathBuf::from(path);
                        assert!(
                            path.starts_with(repository),
                            "production target root escaped the repository: {}",
                            path.display()
                        );
                        let relative = path
                            .strip_prefix(repository)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/");
                        assert!(
                            !is_nonproduction_rust(&relative),
                            "production target root uses an excluded test/example layout: {relative}"
                        );
                        ProductionTarget {
                            path,
                            custom_build: kinds.contains(&"custom-build"),
                        }
                    })
                })
        })
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

fn production_targets() -> Vec<ProductionTarget> {
    targets_from_metadata(cargo_metadata(), &repo())
}

fn production_target_roots() -> Vec<PathBuf> {
    production_targets()
        .into_iter()
        .map(|target| target.path)
        .collect()
}

fn is_nonproduction_rust(path: &str) -> bool {
    anvil::source_scan::paths::is_test_source(path)
        || path.starts_with("examples/")
        || path.contains("/examples/")
        || path.starts_with("benches/")
        || path.contains("/benches/")
}

fn source_paths_from_metadata(metadata: &serde_json::Value, repository: &Path) -> Vec<PathBuf> {
    let targets = targets_from_metadata(metadata, repository);
    let target_roots = targets
        .iter()
        .map(|target| target.path.clone())
        .collect::<BTreeSet<_>>();
    let mut paths = Vec::new();
    // Scan every in-repository Rust file, rather than only target-parent
    // directories. This includes build-script modules and source-controlled
    // `include!` inputs under inactive cfgs. Generated artifacts and Cargo's
    // explicit test/example layouts are excluded below.
    rust_sources(repository, repository, &mut paths);
    paths.extend(target_roots.iter().cloned());
    paths.sort();
    paths.dedup();
    paths.retain(|path| {
        let relative = path
            .strip_prefix(repository)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        target_roots.contains(path) || !is_nonproduction_rust(&relative)
    });
    paths
}

fn all_production_source_paths() -> Vec<PathBuf> {
    source_paths_from_metadata(cargo_metadata(), &repo())
}

fn all_production_sources() -> Vec<(String, String)> {
    all_production_source_paths()
        .into_iter()
        .map(|path| (relative(&path), path))
        .map(|(relative, path)| (relative, production(&path)))
        .collect()
}

fn configured_disallowed_methods() -> Vec<String> {
    let source = fs::read_to_string(repo().join(CLIPPY_CONFIG)).expect("read clippy.toml");
    let config: toml::Value = source.parse().expect("parse clippy.toml");
    let mut methods = config
        .get("disallowed-methods")
        .and_then(toml::Value::as_array)
        .expect("clippy.toml disallowed-methods array")
        .iter()
        .map(|entry| {
            entry
                .get("path")
                .and_then(toml::Value::as_str)
                .expect("every disallowed method has a path")
                .to_owned()
        })
        .collect::<Vec<_>>();
    methods.sort();
    methods
}

fn production_crate_roots() -> Vec<String> {
    let mut roots = production_target_roots()
        .iter()
        .map(|path| relative(path))
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

fn has_process_lint_deny_at_crate_root(source: &str) -> bool {
    let file = syn::parse_file(source).expect("parse production crate root");
    file.attrs.iter().any(|attribute| {
        matches!(attribute.style, syn::AttrStyle::Inner(_))
            && syn_path_is(attribute.path(), "cfg_attr")
            && attribute.meta.require_list().is_ok_and(|list| {
                list.tokens
                    .to_string()
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect::<String>()
                    == "not(test),deny(clippy::disallowed_methods)"
            })
    })
}

fn lint_names(stream: TokenStream) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = String::new();
    let mut is_value = false;

    for token in stream {
        match token {
            TokenTree::Punct(punct) if punct.as_char() == ',' => {
                if !current.is_empty() && !is_value {
                    names.push(std::mem::take(&mut current));
                }
                current.clear();
                is_value = false;
            }
            TokenTree::Punct(punct) if punct.as_char() == '=' => {
                current.clear();
                is_value = true;
            }
            TokenTree::Punct(punct) if punct.as_char() == ':' && !is_value => {
                current.push(':');
            }
            TokenTree::Ident(ident) if !is_value => current.push_str(&normalized_ident(&ident)),
            _ => {}
        }
    }
    if !current.is_empty() && !is_value {
        names.push(current);
    }
    names
}

fn collect_lint_controls(stream: TokenStream, level: &str, controls: &mut Vec<Vec<String>>) {
    let tokens = stream.into_iter().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token, TokenTree::Ident(identifier) if normalized_ident(identifier) == level)
            && let Some(TokenTree::Group(group)) = tokens.get(index + 1)
            && group.delimiter() == Delimiter::Parenthesis
        {
            controls.push(lint_names(group.stream()));
        }
        if let TokenTree::Group(group) = token {
            collect_lint_controls(group.stream(), level, controls);
        }
    }
}

fn lint_controls(source: &str, level: &str) -> Vec<Vec<String>> {
    let stream = source
        .parse::<TokenStream>()
        .expect("lex valid Rust source");
    let mut controls = Vec::new();
    collect_lint_controls(stream, level, &mut controls);
    controls
}

fn weakens_process_lint(lints: &[String]) -> bool {
    lints
        .iter()
        .any(|lint| PROCESS_LINT_NAMES.contains(&lint.as_str()))
}

fn syn_path_is(path: &syn::Path, expected: &str) -> bool {
    path.segments.len() == 1
        && path
            .segments
            .first()
            .is_some_and(|segment| normalized_ident(&segment.ident) == expected)
}

fn syn_path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| normalized_ident(&segment.ident))
        .collect::<Vec<_>>()
        .join("::")
}

fn is_test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        syn_path_is(attribute.path(), "test")
            || syn_path_name(attribute.path()) == "tokio::test"
            || (syn_path_is(attribute.path(), "cfg")
                && attribute
                    .meta
                    .require_list()
                    .is_ok_and(|list| list.tokens.to_string().replace(' ', "") == "test"))
    })
}

fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        syn::Item::Verbatim(_) => &[],
        _ => &[],
    }
}

fn impl_item_attributes(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(item) => &item.attrs,
        syn::ImplItem::Fn(item) => &item.attrs,
        syn::ImplItem::Macro(item) => &item.attrs,
        syn::ImplItem::Type(item) => &item.attrs,
        syn::ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn trait_item_attributes(item: &syn::TraitItem) -> &[syn::Attribute] {
    match item {
        syn::TraitItem::Const(item) => &item.attrs,
        syn::TraitItem::Fn(item) => &item.attrs,
        syn::TraitItem::Macro(item) => &item.attrs,
        syn::TraitItem::Type(item) => &item.attrs,
        syn::TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn foreign_item_attributes(item: &syn::ForeignItem) -> &[syn::Attribute] {
    match item {
        syn::ForeignItem::Fn(item) => &item.attrs,
        syn::ForeignItem::Macro(item) => &item.attrs,
        syn::ForeignItem::Static(item) => &item.attrs,
        syn::ForeignItem::Type(item) => &item.attrs,
        syn::ForeignItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn expression_attributes(expression: &syn::Expr) -> &[syn::Attribute] {
    match expression {
        syn::Expr::Array(expression) => &expression.attrs,
        syn::Expr::Assign(expression) => &expression.attrs,
        syn::Expr::Async(expression) => &expression.attrs,
        syn::Expr::Await(expression) => &expression.attrs,
        syn::Expr::Binary(expression) => &expression.attrs,
        syn::Expr::Block(expression) => &expression.attrs,
        syn::Expr::Break(expression) => &expression.attrs,
        syn::Expr::Call(expression) => &expression.attrs,
        syn::Expr::Cast(expression) => &expression.attrs,
        syn::Expr::Closure(expression) => &expression.attrs,
        syn::Expr::Const(expression) => &expression.attrs,
        syn::Expr::Continue(expression) => &expression.attrs,
        syn::Expr::Field(expression) => &expression.attrs,
        syn::Expr::ForLoop(expression) => &expression.attrs,
        syn::Expr::Group(expression) => &expression.attrs,
        syn::Expr::If(expression) => &expression.attrs,
        syn::Expr::Index(expression) => &expression.attrs,
        syn::Expr::Infer(expression) => &expression.attrs,
        syn::Expr::Let(expression) => &expression.attrs,
        syn::Expr::Lit(expression) => &expression.attrs,
        syn::Expr::Loop(expression) => &expression.attrs,
        syn::Expr::Macro(expression) => &expression.attrs,
        syn::Expr::Match(expression) => &expression.attrs,
        syn::Expr::MethodCall(expression) => &expression.attrs,
        syn::Expr::Paren(expression) => &expression.attrs,
        syn::Expr::Path(expression) => &expression.attrs,
        syn::Expr::Range(expression) => &expression.attrs,
        syn::Expr::RawAddr(expression) => &expression.attrs,
        syn::Expr::Reference(expression) => &expression.attrs,
        syn::Expr::Repeat(expression) => &expression.attrs,
        syn::Expr::Return(expression) => &expression.attrs,
        syn::Expr::Struct(expression) => &expression.attrs,
        syn::Expr::Try(expression) => &expression.attrs,
        syn::Expr::TryBlock(expression) => &expression.attrs,
        syn::Expr::Tuple(expression) => &expression.attrs,
        syn::Expr::Unary(expression) => &expression.attrs,
        syn::Expr::Unsafe(expression) => &expression.attrs,
        syn::Expr::While(expression) => &expression.attrs,
        syn::Expr::Yield(expression) => &expression.attrs,
        syn::Expr::Verbatim(_) => &[],
        _ => &[],
    }
}

fn attribute_policy_violations(meta: &syn::Meta, violations: &mut Vec<String>) {
    let name = syn_path_name(meta.path());
    if !APPROVED_ATTRIBUTES.contains(&name.as_str()) {
        violations.push(format!("unapproved-attribute:{name}"));
        return;
    }

    if name == "derive" {
        let syn::Meta::List(list) = meta else {
            violations.push("malformed-derive".to_owned());
            return;
        };
        let parser = Punctuated::<syn::Path, syn::Token![,]>::parse_terminated;
        let Ok(derives) = parser.parse2(list.tokens.clone()) else {
            violations.push("malformed-derive".to_owned());
            return;
        };
        for derive in derives {
            let derive = syn_path_name(&derive);
            if !APPROVED_DERIVES.contains(&derive.as_str()) {
                violations.push(format!("unapproved-derive:{derive}"));
            }
        }
    }

    if name == "cfg_attr" {
        let syn::Meta::List(list) = meta else {
            violations.push("malformed-cfg-attr".to_owned());
            return;
        };
        let parser = Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
        let Ok(nested) = parser.parse2(list.tokens.clone()) else {
            violations.push("malformed-cfg-attr".to_owned());
            return;
        };
        for attribute in nested.into_iter().skip(1) {
            attribute_policy_violations(&attribute, violations);
        }
    }
}

fn lint_controls_in_meta(meta: &syn::Meta, controls: &mut Vec<(String, Vec<String>)>) {
    let name = syn_path_name(meta.path());
    if matches!(name.as_str(), "allow" | "expect")
        && let syn::Meta::List(list) = meta
    {
        controls.push((name.clone(), lint_names(list.tokens.clone())));
    }
    if name == "cfg_attr"
        && let syn::Meta::List(list) = meta
    {
        let parser = Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
        if let Ok(nested) = parser.parse2(list.tokens.clone()) {
            for attribute in nested.into_iter().skip(1) {
                lint_controls_in_meta(&attribute, controls);
            }
        }
    }
}

fn tokens_assign_identifier(stream: TokenStream, expected: &str) -> bool {
    let tokens = stream.into_iter().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token, TokenTree::Ident(ident) if normalized_ident(ident) == expected)
            && matches!(tokens.get(index + 1), Some(token) if punct(token, '='))
        {
            return true;
        }
        if let TokenTree::Group(group) = token
            && tokens_assign_identifier(group.stream(), expected)
        {
            return true;
        }
    }
    false
}

fn tokens_contain_identifier(stream: TokenStream, expected: &str) -> bool {
    stream.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => normalized_ident(&ident) == expected,
        TokenTree::Group(group) => tokens_contain_identifier(group.stream(), expected),
        _ => false,
    })
}

fn module_path_is_outside_census(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path.starts_with('/')
        || path
            .split('/')
            .next()
            .is_some_and(|first| first.contains(':'))
        || path
            .split('/')
            .any(|part| matches!(part, ".." | ".git" | "target"))
        || !path.ends_with(".rs")
        || is_nonproduction_rust(path.trim_start_matches("./"))
}

fn module_source_escapes_census(
    item: &syn::ItemMod,
    source_path: Option<&Path>,
    scanned_paths: Option<&BTreeSet<PathBuf>>,
    module_stack: &[String],
) -> bool {
    if item.content.is_some() {
        return false;
    }

    let mut has_direct_path = false;
    for attribute in &item.attrs {
        if syn_path_is(attribute.path(), "path") {
            has_direct_path = true;
            let syn::Meta::NameValue(name_value) = &attribute.meta else {
                return true;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(path),
                ..
            }) = &name_value.value
            else {
                return true;
            };
            if module_path_is_outside_census(&path.value()) {
                return true;
            }
            if let (Some(source_path), Some(scanned_paths)) = (source_path, scanned_paths)
                && !module_search_bases(source_path, module_stack)
                    .iter()
                    .any(|base| scanned_paths.contains(&base.join(path.value())))
            {
                return true;
            }
        }
        if syn_path_is(attribute.path(), "cfg_attr")
            && attribute
                .meta
                .require_list()
                .is_ok_and(|list| tokens_assign_identifier(list.tokens.clone(), "path"))
        {
            // Configuration-selected module paths would require resolving a
            // different file graph for every cfg. Keep that out of production
            // source unless the census grows a typed resolver for it.
            return true;
        }
    }

    let implicit_module = normalized_ident(&item.ident);
    if has_direct_path {
        return false;
    }
    if is_nonproduction_rust(&format!("{implicit_module}.rs"))
        || matches!(implicit_module.as_str(), "benches" | "examples" | "target")
    {
        return true;
    }
    if let (Some(source_path), Some(scanned_paths)) = (source_path, scanned_paths) {
        return !module_search_bases(source_path, module_stack)
            .iter()
            .flat_map(|base| {
                [
                    base.join(format!("{implicit_module}.rs")),
                    base.join(&implicit_module).join("mod.rs"),
                ]
            })
            .any(|candidate| scanned_paths.contains(&candidate));
    }
    false
}

fn module_search_bases(source_path: &Path, module_stack: &[String]) -> Vec<PathBuf> {
    let Some(parent) = source_path.parent() else {
        return Vec::new();
    };
    let mut bases = vec![parent.to_path_buf()];
    if source_path.file_name().and_then(|name| name.to_str()) != Some("mod.rs")
        && let Some(stem) = source_path.file_stem()
    {
        bases.push(parent.join(stem));
    }
    for base in &mut bases {
        for module in module_stack {
            base.push(module);
        }
    }
    bases.sort();
    bases.dedup();
    bases
}

fn safe_associated_spawn(path: &syn::Path) -> bool {
    let path = path
        .segments
        .iter()
        .map(|segment| normalized_ident(&segment.ident))
        .collect::<Vec<_>>()
        .join("::");
    SAFE_ASSOCIATED_SPAWNS.contains(&path.as_str())
}

fn is_process_method(identifier: &syn::Ident) -> bool {
    PROCESS_METHOD_NAMES.contains(&normalized_ident(identifier).as_str())
}

fn punct(token: &TokenTree, expected: char) -> bool {
    matches!(token, TokenTree::Punct(punctuation) if punctuation.as_char() == expected)
}

fn associated_token_path(tokens: &[TokenTree], method_index: usize) -> String {
    let TokenTree::Ident(method) = &tokens[method_index] else {
        return String::new();
    };
    let mut segments = vec![normalized_ident(method)];
    let mut cursor = method_index;
    while cursor >= 3
        && punct(&tokens[cursor - 2], ':')
        && punct(&tokens[cursor - 1], ':')
        && matches!(&tokens[cursor - 3], TokenTree::Ident(_))
    {
        let TokenTree::Ident(segment) = &tokens[cursor - 3] else {
            unreachable!();
        };
        segments.push(normalized_ident(segment));
        cursor -= 3;
    }
    segments.reverse();
    segments.join("::")
}

fn token_path_ending_at(tokens: &[TokenTree], end: usize) -> String {
    associated_token_path(tokens, end)
}

fn macro_token_policy(name: &str) -> Option<(bool, bool)> {
    if SAFE_DATA_MACROS.contains(&name) {
        Some((true, true))
    } else if SAFE_CODE_MACROS.contains(&name) {
        Some((false, true))
    } else if name == "macro_rules" {
        // Definitions are source-owned and their complete token bodies are
        // scanned below. Invocations must still use the finite vocabulary.
        Some((false, false))
    } else {
        None
    }
}

fn macro_tokens_execute_process(
    stream: TokenStream,
    allow_bare_process_names: bool,
    allow_trusted_roots: bool,
) -> bool {
    let tokens = stream.into_iter().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            let nested_policy = if index >= 2
                && punct(&tokens[index - 1], '!')
                && matches!(&tokens[index - 2], TokenTree::Ident(_))
            {
                let nested = token_path_ending_at(&tokens, index - 2);
                let Some(policy) = macro_token_policy(&nested) else {
                    return true;
                };
                policy
            } else {
                (allow_bare_process_names, allow_trusted_roots)
            };
            if macro_tokens_execute_process(group.stream(), nested_policy.0, nested_policy.1) {
                return true;
            }
        }

        // Reject macro_rules indirection that substitutes a method name after
        // `.` or `::`. Such a metavariable can hide `output`/`exec` at the call
        // site even though neither source location contains a complete method
        // path. Ordinary macro arguments named `status` remain harmless.
        if punct(token, '$')
            && matches!(tokens.get(index + 1), Some(TokenTree::Ident(_)))
            && (index >= 1 && punct(&tokens[index - 1], '.')
                || index >= 2 && punct(&tokens[index - 2], ':') && punct(&tokens[index - 1], ':'))
        {
            return true;
        }

        let TokenTree::Ident(method) = token else {
            continue;
        };
        let method = normalized_ident(method);
        // An unknown/local macro can mint a trusted-looking import or type
        // alias in its expansion. Fail closed on those roots both in macro
        // definitions and dependency-macro arguments. The finite data-only
        // macro vocabulary may mention these identifiers as ordinary values;
        // its import/path provenance is checked separately by the AST visitor.
        if !allow_trusted_roots && RESERVED_TRUSTED_PATH_ROOTS.contains(&method.as_str()) {
            return true;
        }
        if !PROCESS_METHOD_NAMES.contains(&method.as_str()) {
            continue;
        }

        let is_method_call = index >= 1
            && punct(&tokens[index - 1], '.')
            && matches!(tokens.get(index + 1), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis);
        if is_method_call {
            return true;
        }

        let is_associated = index >= 2
            && punct(&tokens[index - 2], ':')
            && punct(&tokens[index - 1], ':')
            && !(matches!(tokens.get(index + 1), Some(token) if punct(token, ':'))
                && matches!(tokens.get(index + 2), Some(token) if punct(token, ':')));
        if is_associated {
            // A safe task/replacement spawn is meaningful only as a parsed
            // expression whose exact file/function/path is in the separate
            // census. Inside macro arguments expansion and alias provenance
            // are unavailable, so every associated process method fails
            // closed, including `spawn` nested inside an approved data macro.
            return true;
        }

        // The definition may belong to a dependency and use this bare method
        // identifier as `$method` in `Process::$method`. Inactive cfg prevents
        // Clippy from expanding the call, so the source census must fail
        // closed on the invocation argument itself.
        if !allow_bare_process_names {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ExecutionSite {
    owner: String,
    method: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AssociatedSpawnSite {
    owner: String,
    path: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LintControlSite {
    owner: String,
    level: String,
    lints: Vec<String>,
}

type SiteCensus = Vec<(String, String, String)>;
type LintCensus = Vec<(String, String, String, Vec<String>)>;

struct ProcessExecutionVisitor<'scan> {
    owner: String,
    sites: Vec<ExecutionSite>,
    associated_spawns: Vec<AssociatedSpawnSite>,
    lint_controls: Vec<LintControlSite>,
    source_path: Option<&'scan Path>,
    scanned_paths: Option<&'scan BTreeSet<PathBuf>>,
    module_stack: Vec<String>,
}

impl ProcessExecutionVisitor<'_> {
    fn record(&mut self, method: impl Into<String>) {
        self.sites.push(ExecutionSite {
            owner: self.owner.clone(),
            method: method.into(),
        });
    }

    fn record_associated_spawn(&mut self, path: impl Into<String>) {
        self.associated_spawns.push(AssociatedSpawnSite {
            owner: self.owner.clone(),
            path: path.into(),
        });
    }

    fn record_lint_control(&mut self, level: String, lints: Vec<String>) {
        self.lint_controls.push(LintControlSite {
            owner: self.owner.clone(),
            level,
            lints,
        });
    }

    fn reserved_binding(&mut self, ident: &syn::Ident, kind: &str) {
        let binding = normalized_ident(ident);
        if RESERVED_TRUSTED_PATH_ROOTS.contains(&binding.as_str()) {
            self.record(format!("shadowed-trusted-path-root:{kind}:{binding}"));
        }
    }
}

fn expression_path(expression: &syn::Expr) -> Option<&syn::Path> {
    match expression {
        syn::Expr::Path(path) if path.qself.is_none() => Some(&path.path),
        syn::Expr::Group(group) => expression_path(&group.expr),
        syn::Expr::Paren(paren) => expression_path(&paren.expr),
        _ => None,
    }
}

fn expression_label(expression: &syn::Expr) -> String {
    match expression {
        syn::Expr::Path(path) if path.qself.is_none() => syn_path_name(&path.path),
        syn::Expr::Field(field) => {
            let base = expression_label(&field.base);
            let member = match &field.member {
                syn::Member::Named(ident) => normalized_ident(ident),
                syn::Member::Unnamed(index) => index.index.to_string(),
            };
            format!("{base}.{member}")
        }
        syn::Expr::Group(group) => expression_label(&group.expr),
        syn::Expr::Paren(paren) => expression_label(&paren.expr),
        _ => "<non-path>".to_owned(),
    }
}

fn field_label(expression: &syn::ExprField) -> String {
    let base = expression_label(&expression.base);
    let member = match &expression.member {
        syn::Member::Named(ident) => normalized_ident(ident),
        syn::Member::Unnamed(index) => index.index.to_string(),
    };
    format!("{base}.{member}")
}

fn value_label(expression: &syn::Expr) -> String {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(value),
        ..
    }) = expression
    {
        return format!("str:{:?}", value.value());
    }
    expression_label(expression)
}

fn approved_reserved_module(item: &syn::ItemMod, source_path: Option<&Path>) -> bool {
    if item.content.is_some() || !item.attrs.is_empty() {
        return false;
    }
    let name = normalized_ident(&item.ident);
    source_path.is_some_and(|path| {
        (name == "replacement" && path.ends_with("src/exec/mod.rs"))
            || (name == "manual_handlers" && path.ends_with("src/webhook/mod.rs"))
    })
}

impl<'ast> Visit<'ast> for ProcessExecutionVisitor<'_> {
    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if !is_test_only(expression_attributes(expression)) {
            syn::visit::visit_expr(self, expression);
        }
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if !is_test_only(&arm.attrs) {
            syn::visit::visit_arm(self, arm);
        }
    }

    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if let syn::Stmt::Macro(statement) = statement
            && is_test_only(&statement.attrs)
        {
            return;
        }
        syn::visit::visit_stmt(self, statement);
    }

    fn visit_item(&mut self, item: &'ast syn::Item) {
        if !is_test_only(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if !is_test_only(impl_item_attributes(item)) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !is_test_only(trait_item_attributes(item)) {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) {
        if !is_test_only(foreign_item_attributes(item)) {
            syn::visit::visit_foreign_item(self, item);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if !is_test_only(&local.attrs) {
            syn::visit::visit_local(self, local);
        }
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !is_test_only(&item.attrs) {
            if !approved_reserved_module(item, self.source_path) {
                self.reserved_binding(&item.ident, "module");
            }
            if module_source_escapes_census(
                item,
                self.source_path,
                self.scanned_paths,
                &self.module_stack,
            ) {
                self.record("unscanned-module");
            }
            if item.content.is_some() {
                self.module_stack.push(normalized_ident(&item.ident));
                syn::visit::visit_item_mod(self, item);
                self.module_stack.pop();
            }
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let binding = item
            .rename
            .as_ref()
            .map(|(_, rename)| normalized_ident(rename))
            .unwrap_or_else(|| normalized_ident(&item.ident));
        if RESERVED_TRUSTED_PATH_ROOTS.contains(&binding.as_str())
            || item.attrs.iter().any(|attribute| {
                syn_path_is(attribute.path(), "macro_use")
                    || (syn_path_is(attribute.path(), "cfg_attr")
                        && attribute.meta.require_list().is_ok_and(|list| {
                            tokens_contain_identifier(list.tokens.clone(), "macro_use")
                        }))
            })
        {
            self.record("untrusted-extern-crate-binding");
        }
        syn::visit::visit_item_extern_crate(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        self.reserved_binding(&item.ident, "type");
        syn::visit::visit_item_type(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.reserved_binding(&item.ident, "struct");
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.reserved_binding(&item.ident, "enum");
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        self.reserved_binding(&item.ident, "union");
        syn::visit::visit_item_union(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.reserved_binding(&item.ident, "trait");
        syn::visit::visit_item_trait(self, item);
    }

    fn visit_item_trait_alias(&mut self, item: &'ast syn::ItemTraitAlias) {
        self.reserved_binding(&item.ident, "trait-alias");
        syn::visit::visit_item_trait_alias(self, item);
    }

    fn visit_foreign_item_type(&mut self, item: &'ast syn::ForeignItemType) {
        self.reserved_binding(&item.ident, "foreign-type");
        syn::visit::visit_foreign_item_type(self, item);
    }

    fn visit_generic_param(&mut self, parameter: &'ast syn::GenericParam) {
        if let syn::GenericParam::Type(parameter) = parameter {
            self.reserved_binding(&parameter.ident, "generic-type");
        }
        syn::visit::visit_generic_param(self, parameter);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !is_test_only(&item.attrs) {
            let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
            syn::visit::visit_item_fn(self, item);
            self.owner = previous;
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if !is_test_only(&item.attrs) {
            let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
            syn::visit::visit_impl_item_fn(self, item);
            self.owner = previous;
        }
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        if item.ident.as_ref().is_some_and(|ident| {
            let name = normalized_ident(ident);
            SAFE_DATA_MACROS.contains(&name.as_str()) || SAFE_CODE_MACROS.contains(&name.as_str())
        }) {
            self.record("shadowed-safe-macro");
        }
        syn::visit::visit_item_macro(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut bindings = Vec::new();
        let mut glob_sources = Vec::new();
        collect_use_bindings(
            &item.tree,
            &mut Vec::new(),
            &mut bindings,
            &mut glob_sources,
        );
        if glob_sources
            .iter()
            .any(|source| !APPROVED_LOCAL_GLOB_IMPORTS.contains(&source.as_str()))
        {
            self.record("unapproved-glob-import");
        }
        for (source, binding) in bindings {
            let approved_macro_import = APPROVED_SAFE_MACRO_IMPORTS.contains(&source.as_str());
            let approved_attribute_import = APPROVED_ATTRIBUTE_IMPORTS.contains(&source.as_str());
            let approved_derive_import = APPROVED_DERIVE_IMPORTS.contains(&source.as_str());
            let approved_trusted_import =
                approved_macro_import || approved_attribute_import || approved_derive_import;
            if RESERVED_TRUSTED_PATH_ROOTS.contains(&binding.as_str()) && !approved_trusted_import {
                self.record("shadowed-trusted-path-root");
            }
            if SAFE_DATA_MACROS.contains(&binding.as_str()) && !approved_macro_import {
                self.record("shadowed-safe-macro");
            }
            if APPROVED_ATTRIBUTES.contains(&binding.as_str()) && !approved_trusted_import {
                self.record("shadowed-approved-attribute");
            }
            if APPROVED_DERIVES.contains(&binding.as_str()) && !approved_trusted_import {
                self.record("shadowed-approved-derive");
            }
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let mut violations = Vec::new();
        attribute_policy_violations(&attribute.meta, &mut violations);
        for violation in violations {
            self.record(violation);
        }
        let mut controls = Vec::new();
        lint_controls_in_meta(&attribute.meta, &mut controls);
        for (level, lints) in controls {
            self.record_lint_control(level, lints);
        }
        syn::visit::visit_attribute(self, attribute);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        if is_process_method(&expression.method) {
            self.record(format!(
                "method:{}:{}",
                expression_label(&expression.receiver),
                normalized_ident(&expression.method)
            ));
        }
        syn::visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(path) = expression_path(&expression.func)
            && let Some(method) = path.segments.last()
            && is_process_method(&method.ident)
            && path.segments.len() > 1
        {
            if safe_associated_spawn(path) {
                self.record_associated_spawn(syn_path_name(path));
            } else {
                self.record(format!(
                    "associated-call:{}",
                    normalized_ident(&method.ident)
                ));
            }
            for argument in &expression.args {
                self.visit_expr(argument);
            }
            return;
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        let process_method = expression
            .path
            .segments
            .last()
            .filter(|segment| is_process_method(&segment.ident));
        if let Some(method) = process_method
            && (expression.qself.is_some() || expression.path.segments.len() > 1)
        {
            self.record(format!(
                "associated-reference:{}",
                normalized_ident(&method.ident)
            ));
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        // An arbitrary include can add process syntax without a `.rs` path the
        // repository census can identify. Make every include an explicit seam
        // decision; source-controlled Rust modules are discovered directly.
        let macro_name = syn_path_name(&invocation.path);
        let policy = macro_token_policy(&macro_name);
        if syn_path_is(&invocation.path, "include") || policy.is_none() {
            self.record(format!("macro:{}", syn_path_name(&invocation.path)));
        } else if let Some((allow_bare_process_names, allow_trusted_roots)) = policy
            && macro_tokens_execute_process(
                invocation.tokens.clone(),
                allow_bare_process_names,
                allow_trusted_roots,
            )
        {
            self.record(format!("macro:{}", syn_path_name(&invocation.path)));
        }
        syn::visit::visit_macro(self, invocation);
    }
}

fn contains_process_execution_syntax(source: &str) -> bool {
    let (sites, associated_spawns, _) = process_scan_with_context(source, None, None);
    !sites.is_empty() || !associated_spawns.is_empty()
}

fn process_execution_sites_with_context(
    source: &str,
    source_path: Option<&Path>,
    scanned_paths: Option<&BTreeSet<PathBuf>>,
) -> Vec<ExecutionSite> {
    process_scan_with_context(source, source_path, scanned_paths).0
}

fn process_scan_with_context(
    source: &str,
    source_path: Option<&Path>,
    scanned_paths: Option<&BTreeSet<PathBuf>>,
) -> (
    Vec<ExecutionSite>,
    Vec<AssociatedSpawnSite>,
    Vec<LintControlSite>,
) {
    let file = syn::parse_file(source).expect("parse valid Rust production source");
    let mut visitor = ProcessExecutionVisitor {
        owner: String::new(),
        sites: Vec::new(),
        associated_spawns: Vec::new(),
        lint_controls: Vec::new(),
        source_path,
        scanned_paths,
        module_stack: Vec::new(),
    };
    visitor.visit_file(&file);
    (
        visitor.sites,
        visitor.associated_spawns,
        visitor.lint_controls,
    )
}

fn collect_use_bindings(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    bindings: &mut Vec<(String, String)>,
    glob_sources: &mut Vec<String>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(normalized_ident(&path.ident));
            collect_use_bindings(&path.tree, prefix, bindings, glob_sources);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let name = normalized_ident(&name.ident);
            let mut full = prefix.clone();
            full.push(name.clone());
            bindings.push((full.join("::"), name));
        }
        syn::UseTree::Rename(rename) => {
            let mut full = prefix.clone();
            full.push(normalized_ident(&rename.ident));
            bindings.push((full.join("::"), normalized_ident(&rename.rename)));
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_bindings(item, prefix, bindings, glob_sources);
            }
        }
        syn::UseTree::Glob(_) => glob_sources.push(prefix.join("::")),
    }
}

fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| normalized_ident(&segment.ident))
            .unwrap_or_default(),
        syn::Type::Reference(reference) => format!("&{}", type_name(&reference.elem)),
        _ => String::new(),
    }
}

fn pattern_name(pattern: &syn::Pat) -> String {
    match pattern {
        syn::Pat::Ident(ident) => normalized_ident(&ident.ident),
        syn::Pat::TupleStruct(tuple) => tuple
            .path
            .segments
            .last()
            .map(|segment| normalized_ident(&segment.ident))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn string_array_constant(source: &str, name: &str) -> Vec<String> {
    fn array(expression: &syn::Expr) -> Option<&syn::ExprArray> {
        match expression {
            syn::Expr::Array(array) => Some(array),
            syn::Expr::Reference(reference) => array(&reference.expr),
            syn::Expr::Group(group) => array(&group.expr),
            syn::Expr::Paren(paren) => array(&paren.expr),
            _ => None,
        }
    }

    let file = syn::parse_file(source).expect("parse source containing finite vocabulary");
    let constant = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Const(item) if normalized_ident(&item.ident) == name => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing string-array constant {name}"));
    array(&constant.expr)
        .unwrap_or_else(|| panic!("{name} is no longer a literal array"))
        .elems
        .iter()
        .map(|element| match element {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => value.value(),
            _ => panic!("{name} contains a non-literal entry"),
        })
        .collect()
}

fn function_shape(source: &str, name: &str) -> (bool, Vec<(String, String)>) {
    let file = syn::parse_file(source).expect("parse transport source");
    let function = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if normalized_ident(&function.sig.ident) == name => {
                Some(function)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing expected transport function {name}"));
    let parameters = function
        .sig
        .inputs
        .iter()
        .filter_map(|argument| match argument {
            syn::FnArg::Typed(argument) => {
                Some((pattern_name(&argument.pat), type_name(&argument.ty)))
            }
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    (
        matches!(function.vis, syn::Visibility::Inherited),
        parameters,
    )
}

fn top_level_function<'source>(file: &'source syn::File, name: &str) -> &'source syn::ItemFn {
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if normalized_ident(&function.sig.ident) == name => {
                Some(function)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing expected function {name}"))
}

#[derive(Default)]
struct CommandFlowVisitor {
    events: Vec<String>,
}

impl CommandFlowVisitor {
    fn record(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }
}

impl<'ast> Visit<'ast> for CommandFlowVisitor {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        if normalized_ident(&pattern.ident) == "command" {
            self.record("bind:command");
        }
        syn::visit::visit_pat_ident(self, pattern);
    }

    fn visit_pat_struct(&mut self, pattern: &'ast syn::PatStruct) {
        if syn_path_name(&pattern.path).ends_with("AgentCommand") {
            let mut fields = pattern
                .fields
                .iter()
                .filter_map(|field| match &field.member {
                    syn::Member::Named(member) => Some(format!(
                        "{}={}",
                        normalized_ident(member),
                        pattern_name(&field.pat)
                    )),
                    syn::Member::Unnamed(_) => None,
                })
                .collect::<Vec<_>>();
            fields.sort();
            self.record(format!(
                "struct-bind:{}:{}",
                syn_path_name(&pattern.path),
                fields.join(",")
            ));
        }
        syn::visit::visit_pat_struct(self, pattern);
    }

    fn visit_pat_tuple_struct(&mut self, pattern: &'ast syn::PatTupleStruct) {
        let path = syn_path_name(&pattern.path);
        if matches!(
            path.rsplit("::").next(),
            Some("NonModelCommand" | "SyncNonModelCommand" | "ProviderProbeCommand")
        ) {
            let bindings = pattern
                .elems
                .iter()
                .map(pattern_name)
                .collect::<Vec<_>>()
                .join(",");
            self.record(format!("tuple-bind:{path}:{bindings}"));
        }
        syn::visit::visit_pat_tuple_struct(self, pattern);
    }

    fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) {
        if expression_label(&expression.left) == "command" {
            self.record("assign:command");
        }
        syn::visit::visit_expr_assign(self, expression);
    }

    fn visit_expr_reference(&mut self, expression: &'ast syn::ExprReference) {
        if expression.mutability.is_some() && expression_label(&expression.expr) == "command" {
            self.record("mut-ref:command");
        }
        syn::visit::visit_expr_reference(self, expression);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(path) = expression_path(&expression.func)
            && is_command_constructor(path)
        {
            let argument = expression
                .args
                .first()
                .map(value_label)
                .unwrap_or_else(|| "<missing>".to_owned());
            self.record(format!("command-new:{}:{argument}", syn_path_name(path)));
        }
        syn::visit::visit_expr_call(self, expression);
    }
}

fn command_flow(source: &str, function: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("parse execution seam");
    let mut visitor = CommandFlowVisitor::default();
    visitor.visit_item_fn(top_level_function(&file, function));
    visitor.events.sort();
    visitor.events
}

#[derive(Default)]
struct BoundaryEventVisitor {
    owner: String,
    events: Vec<(String, String)>,
}

impl<'ast> Visit<'ast> for BoundaryEventVisitor {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
        syn::visit::visit_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
        syn::visit::visit_impl_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if let Some(segment) = expression.path.segments.last() {
            let name = normalized_ident(&segment.ident);
            if matches!(
                name.as_str(),
                "deliver_with_stdin" | "is_provider_program" | "run_bounded_with_stdin"
            ) {
                self.events.push((self.owner.clone(), name));
            }
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        if is_process_method(&expression.method) {
            self.events
                .push((self.owner.clone(), normalized_ident(&expression.method)));
        }
        syn::visit::visit_expr_method_call(self, expression);
    }
}

fn boundary_events(source: &str) -> Vec<(String, String)> {
    let file = syn::parse_file(source).expect("parse transport source");
    let mut visitor = BoundaryEventVisitor::default();
    visitor.visit_file(&file);
    visitor.events
}

struct AgentCapabilityVisitor {
    owner: String,
    events: Vec<(String, String)>,
    track_bare_command: bool,
}

impl Default for AgentCapabilityVisitor {
    fn default() -> Self {
        Self {
            owner: String::new(),
            events: Vec::new(),
            track_bare_command: true,
        }
    }
}

impl AgentCapabilityVisitor {
    fn record(&mut self, event: impl Into<String>) {
        self.events.push((self.owner.clone(), event.into()));
    }
}

fn is_command_constructor(path: &syn::Path) -> bool {
    let mut segments = path.segments.iter().rev();
    segments
        .next()
        .is_some_and(|segment| normalized_ident(&segment.ident) == "new")
        && segments
            .next()
            .is_some_and(|segment| normalized_ident(&segment.ident) == "Command")
}

fn agent_struct_fields(expression: &syn::ExprStruct) -> Option<(String, String)> {
    let mut command = None;
    let mut framing = None;
    for field in &expression.fields {
        let syn::Member::Named(member) = &field.member else {
            continue;
        };
        match normalized_ident(member).as_str() {
            "command" => command = Some(value_label(&field.expr)),
            "framing" => framing = Some(value_label(&field.expr)),
            _ => {}
        }
    }
    command.zip(framing)
}

impl<'ast> Visit<'ast> for AgentCapabilityVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if !is_test_only(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if !is_test_only(impl_item_attributes(item)) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !is_test_only(trait_item_attributes(item)) {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if !is_test_only(expression_attributes(expression)) {
            syn::visit::visit_expr(self, expression);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
        syn::visit::visit_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
        syn::visit::visit_impl_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut bindings = Vec::new();
        collect_use_bindings(&item.tree, &mut Vec::new(), &mut bindings, &mut Vec::new());
        for (source, binding) in bindings {
            let source_subject = source.rsplit("::").next().is_some_and(|name| {
                matches!(name, "AgentCommand" | "ProviderProbeCommand" | "Command")
            });
            if source_subject
                || matches!(
                    binding.as_str(),
                    "AgentCommand" | "ProviderProbeCommand" | "Command"
                )
            {
                self.record(format!("import:{source}->{binding}"));
            }
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        self.record(format!("type-alias:{}", normalized_ident(&item.ident)));
        syn::visit::visit_item_type(self, item);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(path) = expression_path(&expression.func)
            && let Some(last) = path.segments.last()
        {
            let last = normalized_ident(&last.ident);
            if matches!(last.as_str(), "command" | "command_in") {
                let argument = expression
                    .args
                    .first()
                    .map(value_label)
                    .unwrap_or_else(|| "<missing>".to_owned());
                self.record(format!("call:{}:{argument}", syn_path_name(path)));
                for argument in &expression.args {
                    self.visit_expr(argument);
                }
                return;
            }
            if last == "ProviderProbeCommand" {
                self.record(format!("construct-probe:{}", syn_path_name(path)));
                for argument in &expression.args {
                    self.visit_expr(argument);
                }
                return;
            }
            if is_command_constructor(path) {
                let argument = expression
                    .args
                    .first()
                    .map(value_label)
                    .unwrap_or_else(|| "<missing>".to_owned());
                self.record(format!("command-new:{}:{argument}", syn_path_name(path)));
                for argument in &expression.args {
                    self.visit_expr(argument);
                }
                return;
            }
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if is_test_only(&local.attrs) {
            return;
        }
        if let Some(initializer) = &local.init
            && let syn::Expr::Call(call) = initializer.expr.as_ref()
            && let Some(path) = expression_path(&call.func)
            && is_command_constructor(path)
        {
            let argument = call
                .args
                .first()
                .map(value_label)
                .unwrap_or_else(|| "<missing>".to_owned());
            self.record(format!(
                "bind-command-new:{}:{}:{argument}",
                pattern_name(&local.pat),
                syn_path_name(path)
            ));
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if let Some(last) = expression.path.segments.last() {
            let last = normalized_ident(&last.ident);
            if last == "command_in"
                || (last == "command"
                    && (expression.path.segments.len() > 1 || self.track_bare_command))
            {
                self.record(format!("reference:{}", syn_path_name(&expression.path)));
            }
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        let path = syn_path_name(&expression.path);
        if path.ends_with("AgentCommand") || agent_struct_fields(expression).is_some() {
            let (command, framing) = agent_struct_fields(expression)
                .unwrap_or_else(|| ("<missing>".to_owned(), "<missing>".to_owned()));
            self.record(format!(
                "construct-agent:{path}:command={command}:framing={framing}"
            ));
        }
        syn::visit::visit_expr_struct(self, expression);
    }

    fn visit_pat_struct(&mut self, pattern: &'ast syn::PatStruct) {
        if syn_path_name(&pattern.path).ends_with("AgentCommand") {
            self.record(format!(
                "destructure-agent:{}",
                syn_path_name(&pattern.path)
            ));
        }
        syn::visit::visit_pat_struct(self, pattern);
    }

    fn visit_pat_tuple_struct(&mut self, pattern: &'ast syn::PatTupleStruct) {
        if syn_path_name(&pattern.path).ends_with("ProviderProbeCommand") {
            self.record(format!(
                "destructure-probe:{}",
                syn_path_name(&pattern.path)
            ));
        }
        syn::visit::visit_pat_tuple_struct(self, pattern);
    }

    fn visit_expr_field(&mut self, expression: &'ast syn::ExprField) {
        match &expression.member {
            syn::Member::Named(member) if normalized_ident(member) == "command" => {
                self.record(format!("raw-field:{}", field_label(expression)));
            }
            syn::Member::Unnamed(index) if index.index == 0 => {
                self.record(format!("raw-tuple-field:{}", field_label(expression)));
            }
            _ => {}
        }
        syn::visit::visit_expr_field(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        let method = normalized_ident(&expression.method);
        if matches!(method.as_str(), "arg" | "args") {
            self.record(format!(
                "argv:{}:{method}",
                expression_label(&expression.receiver)
            ));
        }
        syn::visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) {
        self.record(format!("assign:{}", expression_label(&expression.left)));
        syn::visit::visit_expr_assign(self, expression);
    }

    fn visit_expr_reference(&mut self, expression: &'ast syn::ExprReference) {
        if expression.mutability.is_some() {
            self.record(format!("mut-ref:{}", expression_label(&expression.expr)));
        }
        syn::visit::visit_expr_reference(self, expression);
    }
}

fn agent_capability_census() -> SiteCensus {
    let mut events = Vec::new();
    for source_path in all_production_source_paths().into_iter().filter(|path| {
        let path = relative(path);
        path == "src/exec/agent.rs" || path.starts_with("src/exec/agent/")
    }) {
        let source = fs::read_to_string(&source_path).expect("read agent-boundary source");
        let path = relative(&source_path);
        let file = syn::parse_file(&source).expect("parse agent-boundary source");
        let mut visitor = AgentCapabilityVisitor {
            track_bare_command: path == "src/exec/agent.rs",
            ..AgentCapabilityVisitor::default()
        };
        visitor.visit_file(&file);
        for (owner, event) in visitor.events {
            events.push((path.clone(), owner, event));
        }
    }
    events.sort();
    events
}

fn agent_capability_events(source: &str) -> Vec<(String, String)> {
    let file = syn::parse_file(source).expect("parse agent-boundary source");
    let mut visitor = AgentCapabilityVisitor::default();
    visitor.visit_file(&file);
    visitor.events.sort();
    visitor.events
}

fn is_nonmodel_capability(name: &str) -> bool {
    matches!(name, "NonModelCommand" | "SyncNonModelCommand")
}

#[derive(Default)]
struct NonModelCapabilityVisitor {
    owner: String,
    implementation: String,
    events: Vec<(String, String)>,
}

impl NonModelCapabilityVisitor {
    fn record(&mut self, event: impl Into<String>) {
        self.events.push((self.owner.clone(), event.into()));
    }
}

impl<'ast> Visit<'ast> for NonModelCapabilityVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if !is_test_only(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if !is_test_only(impl_item_attributes(item)) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if !is_test_only(expression_attributes(expression)) {
            syn::visit::visit_expr(self, expression);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
        syn::visit::visit_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let previous = std::mem::replace(&mut self.owner, normalized_ident(&item.sig.ident));
        syn::visit::visit_impl_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let previous = std::mem::replace(&mut self.implementation, type_name(&item.self_ty));
        syn::visit::visit_item_impl(self, item);
        self.implementation = previous;
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut bindings = Vec::new();
        collect_use_bindings(&item.tree, &mut Vec::new(), &mut bindings, &mut Vec::new());
        for (source, binding) in bindings {
            let source_subject = source
                .rsplit("::")
                .next()
                .is_some_and(is_nonmodel_capability);
            if source_subject || is_nonmodel_capability(&binding) {
                self.record(format!("import:{source}->{binding}"));
            }
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if is_nonmodel_capability(&type_name(&item.ty)) {
            self.record(format!("type-alias:{}", normalized_ident(&item.ident)));
        }
        syn::visit::visit_item_type(self, item);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(path) = expression_path(&expression.func)
            && let Some(last) = path.segments.last()
        {
            let last = normalized_ident(&last.ident);
            let previous = path
                .segments
                .iter()
                .rev()
                .nth(1)
                .map(|segment| normalized_ident(&segment.ident));
            if last == "checked" && previous.as_deref().is_some_and(is_nonmodel_capability) {
                self.record(format!("call:{}", syn_path_name(path)));
                for argument in &expression.args {
                    self.visit_expr(argument);
                }
                return;
            }
            if last == "Self" && is_nonmodel_capability(&self.implementation) {
                self.record(format!("construct-self:{}", self.implementation));
                for argument in &expression.args {
                    self.visit_expr(argument);
                }
                return;
            }
            if is_nonmodel_capability(&last) {
                self.record(format!("construct:{}", syn_path_name(path)));
                for argument in &expression.args {
                    self.visit_expr(argument);
                }
                return;
            }
        }
        syn::visit::visit_expr_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        let path = syn_path_name(&expression.path);
        let last = expression
            .path
            .segments
            .last()
            .map(|segment| normalized_ident(&segment.ident));
        if last.as_deref() == Some("checked") || last.as_deref().is_some_and(is_nonmodel_capability)
        {
            self.record(format!("reference:{path}"));
        }
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_pat_tuple_struct(&mut self, pattern: &'ast syn::PatTupleStruct) {
        let name = syn_path_name(&pattern.path);
        if name.rsplit("::").next().is_some_and(is_nonmodel_capability) {
            self.record(format!("destructure:{name}"));
        }
        syn::visit::visit_pat_tuple_struct(self, pattern);
    }

    fn visit_expr_field(&mut self, expression: &'ast syn::ExprField) {
        if matches!(&expression.member, syn::Member::Unnamed(index) if index.index == 0) {
            self.record(format!("raw-field:{}", field_label(expression)));
        }
        syn::visit::visit_expr_field(self, expression);
    }
}

fn nonmodel_capability_census() -> SiteCensus {
    let mut events = Vec::new();
    for source_path in all_production_source_paths().into_iter().filter(|path| {
        let path = relative(path);
        path == "src/exec/non_model.rs" || path.starts_with("src/exec/non_model/")
    }) {
        let source = fs::read_to_string(&source_path).expect("read non-model boundary source");
        let file = syn::parse_file(&source).expect("parse non-model boundary source");
        let mut visitor = NonModelCapabilityVisitor::default();
        visitor.visit_file(&file);
        let path = relative(&source_path);
        for (owner, event) in visitor.events {
            events.push((path.clone(), owner, event));
        }
    }
    events.sort();
    events
}

#[test]
fn only_the_finite_provider_seam_constructs_agent_commands() {
    let actual = agent_capability_census();
    let mut expected = EXPECTED_AGENT_CAPABILITY_EVENTS
        .iter()
        .map(|(path, owner, event)| ((*path).to_owned(), (*owner).to_owned(), (*event).to_owned()))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        actual, expected,
        "the opaque AgentCommand/probe constructors, argv mutators or raw fields changed"
    );

    let seam = production(&repo().join(PROVIDER_SEAM));
    for provider in KNOWN_PROVIDERS {
        assert!(
            seam.contains(&format!("\"{provider}\"")),
            "{provider} is absent from the finite provider registry"
        );
    }
}

#[test]
fn agent_argv_mutation_is_exec_private_and_centralized() {
    let agent = production(&repo().join("src/exec/agent.rs"));
    assert!(agent.contains("fn args"));
    assert!(!agent.contains("pub(super) fn args"));
    assert!(!agent.contains("pub(crate) fn arg"));
    assert!(!agent.contains("pub(crate) fn args"));

    let router = production(&repo().join("src/ai_driver/router.rs"));
    assert!(!router.contains(".arg("));
    assert!(!router.contains(".args("));
    for constructor in [
        "claude_agent(",
        "codex_agent(",
        "cursor_agent(",
        "grok_agent(",
        "agy_agent(",
    ] {
        assert!(
            router.contains(constructor),
            "router bypasses {constructor}"
        );
    }
}

#[test]
fn prompt_bytes_command_conversion_and_framing_stay_in_the_private_transport() {
    let agent = production(&repo().join("src/exec/agent.rs"));
    let provider = production(&repo().join(PROVIDER_SEAM));
    let transport = production(&repo().join(MODEL_TRANSPORT));
    let exec = production(&repo().join("src/exec/mod.rs"));
    let turn = production(&repo().join("src/exec/turn.rs"));
    let prompt = production(&repo().join("src/model_prompt.rs"));

    assert!(agent.contains("mod transport;"));
    assert!(!agent.contains("pub mod transport"));
    assert!(!agent.contains("into_command"));
    assert!(agent.contains("enum Framing"));
    assert!(provider.contains("Framing::Plain"));
    assert!(provider.contains("Framing::AgyStreamJson"));

    assert!(transport.contains("ModelPromptPermit(PrivatePermit)"));
    assert!(transport.contains("let AgentCommand"));
    assert!(transport.contains("prompt.as_str(&permit)"));
    assert!(transport.contains("async fn deliver_with_stdin"));
    assert!(prompt.contains("agent::ModelPromptPermit"));

    for source in [&exec, &turn] {
        assert!(!source.contains("ModelPromptPermit"));
        assert!(!source.contains("into_command"));
        assert!(!source.contains("deliver_with_stdin"));
        assert!(!source.contains("prompt.as_str"));
    }
}

#[test]
fn raw_stdin_transport_has_a_closed_production_census() {
    let mut callers = Vec::new();
    for source_path in all_production_source_paths() {
        let path = relative(&source_path);
        let source = fs::read_to_string(&source_path).expect("read raw-stdin census source");
        for (owner, event) in boundary_events(&source) {
            if event == "run_bounded_with_stdin" {
                callers.push((path.clone(), owner));
            }
        }
    }
    callers.sort();
    let mut expected = EXPECTED_RAW_STDIN_REFERENCES
        .iter()
        .map(|(path, owner)| ((*path).to_owned(), (*owner).to_owned()))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(callers, expected);

    let exec = production(&repo().join("src/exec/mod.rs"));
    assert!(exec.contains("pub(crate) async fn run_bounded("));
    assert!(exec.contains("pub(crate) async fn run_bounded_for("));
    assert!(exec.contains("pub(crate) async fn run_bounded_with_stdin"));
    assert!(!exec.contains("pub async fn run_bounded("));
    assert!(!exec.contains("pub async fn run_bounded_for("));
    assert!(!exec.contains("pub async fn run_bounded_with_stdin"));
    assert!(exec.contains("non_model::run(cmd, class, what).await"));
    assert!(exec.contains("non_model::run_for(cmd, limit, what).await"));
    assert!(exec.contains("non_model::run_with_stdin"));
}

#[test]
fn raw_runners_admit_only_a_finite_direct_nonmodel_tool_capability() {
    let transport = production(&repo().join(NON_MODEL_TRANSPORT));
    assert!(transport.contains("struct NonModelCommand(Command)"));
    assert!(transport.contains("NonModelCommand::checked(command)?"));
    assert!(transport.contains("const NON_MODEL_PROGRAMS"));
    assert!(transport.contains("resolve_executable"));
    assert!(transport.contains("is_provider_program(resolved.canonical.as_os_str())"));
    assert!(!transport.contains("Some(\"--help\" | \"--version\")"));

    assert_eq!(
        string_array_constant(&transport, "NON_MODEL_PROGRAMS"),
        NON_MODEL_PROGRAM_VOCABULARY,
        "the executable vocabulary changed without an explicit boundary decision"
    );

    let actual = nonmodel_capability_census();
    let mut expected = EXPECTED_NONMODEL_CAPABILITY_EVENTS
        .iter()
        .map(|(path, owner, event)| ((*path).to_owned(), (*owner).to_owned(), (*event).to_owned()))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        actual, expected,
        "checked non-model capabilities can only be minted and opened at their finite seams"
    );
}

#[test]
fn compiler_owns_process_execution_and_only_private_seams_are_exempt() {
    let mut expected = DISALLOWED_PROCESS_METHODS
        .iter()
        .map(|method| (*method).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(configured_disallowed_methods(), expected);

    let manifest: toml::Value = fs::read_to_string(repo().join("Cargo.toml"))
        .expect("read Cargo.toml")
        .parse()
        .expect("parse Cargo.toml");
    assert_eq!(
        manifest["lints"]["clippy"]["disallowed_methods"].as_str(),
        Some("allow"),
        "test targets may execute fixtures; production roots must elevate the lint"
    );

    let crate_roots = production_crate_roots();
    assert!(
        !crate_roots.is_empty(),
        "production crate-root census is empty"
    );
    for path in crate_roots {
        let source = fs::read_to_string(repo().join(&path)).expect("read production crate root");
        assert!(
            has_process_lint_deny_at_crate_root(&source),
            "production crate root {path} does not deny raw process execution"
        );
    }

    let (_, _, controls) = production_process_census();
    let mut expectations = Vec::new();
    for (path, owner, level, lints) in controls {
        if !weakens_process_lint(&lints) {
            continue;
        }
        assert_eq!(
            level, "expect",
            "production source {path}::{owner} can silence the process-execution guard"
        );
        assert_eq!(
            lints,
            ["clippy::disallowed_methods"],
            "production source {path}::{owner} uses a broad process-lint exemption"
        );
        expectations.push((path, owner));
    }
    expectations.sort();
    let mut expected_expectations = EXPECTED_LINT_EXPECTATIONS
        .iter()
        .map(|(path, owner)| ((*path).to_owned(), (*owner).to_owned()))
        .collect::<Vec<_>>();
    expected_expectations.sort();
    assert_eq!(expectations, expected_expectations);

    let exemption_files = expectations
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        exemption_files,
        DIRECT_EXECUTION_SEAMS.iter().copied().collect()
    );
}

/// The compiler guard resolves aliases in the active build. This token/AST
/// census is the complementary guard for cfg-inactive and platform-specific
/// source; conservative macro handling makes an indirect process method an
/// explicit seam change instead of an invisible execution path.
fn production_process_census() -> (SiteCensus, SiteCensus, LintCensus) {
    let mut execution_sites = Vec::new();
    let mut associated_spawns = Vec::new();
    let mut lint_controls = Vec::new();
    let source_paths = all_production_source_paths();
    let scanned_paths = source_paths.iter().cloned().collect::<BTreeSet<_>>();
    for source_path in source_paths {
        let source = fs::read_to_string(&source_path).expect("read production Rust source");
        let path = relative(&source_path);
        let (sites, safe_spawns, controls) =
            process_scan_with_context(&source, Some(&source_path), Some(&scanned_paths));
        for site in sites {
            execution_sites.push((path.clone(), site.owner, site.method));
        }
        for site in safe_spawns {
            associated_spawns.push((path.clone(), site.owner, site.path));
        }
        for control in controls {
            lint_controls.push((path.clone(), control.owner, control.level, control.lints));
        }
    }
    execution_sites.sort();
    associated_spawns.sort();
    lint_controls.sort();
    (execution_sites, associated_spawns, lint_controls)
}

#[test]
fn current_process_execution_call_sites_are_the_private_exec_seams() {
    let (execution_sites, _, _) = production_process_census();
    let mut expected = EXPECTED_EXECUTION_SITES
        .iter()
        .map(|(path, owner, method)| {
            (
                (*path).to_owned(),
                (*owner).to_owned(),
                (*method).to_owned(),
            )
        })
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(execution_sites, expected);
}

#[test]
fn safe_associated_spawns_have_an_exact_site_census() {
    let (_, associated_spawns, _) = production_process_census();
    let mut expected_spawns = EXPECTED_SAFE_ASSOCIATED_SPAWNS
        .iter()
        .map(|(path, owner, spawn_path)| {
            (
                (*path).to_owned(),
                (*owner).to_owned(),
                (*spawn_path).to_owned(),
            )
        })
        .collect::<Vec<_>>();
    expected_spawns.sort();
    assert_eq!(
        associated_spawns, expected_spawns,
        "safe task/replacement spawns changed; every textual spawn path requires an explicit site"
    );
}

#[test]
fn every_execution_site_is_downstream_of_its_typed_capability() {
    let agent = fs::read_to_string(repo().join("src/exec/agent/transport.rs")).unwrap();
    let non_model = fs::read_to_string(repo().join("src/exec/non_model/transport.rs")).unwrap();
    let replacement = fs::read_to_string(repo().join("src/exec/replacement.rs")).unwrap();

    assert_eq!(
        function_shape(&agent, "probe").1,
        [
            (
                "ProviderProbeCommand".to_owned(),
                "ProviderProbeCommand".to_owned(),
            ),
            ("limit".to_owned(), "Duration".to_owned()),
            ("what".to_owned(), "&str".to_owned()),
        ]
    );
    assert_eq!(
        function_shape(&non_model, "run_for").1,
        [
            ("NonModelCommand".to_owned(), "NonModelCommand".to_owned(),),
            ("limit".to_owned(), "Duration".to_owned()),
            ("what".to_owned(), "&str".to_owned()),
            ("class".to_owned(), "Option".to_owned()),
        ]
    );
    assert_eq!(
        function_shape(&non_model, "run_status").1,
        [
            ("NonModelCommand".to_owned(), "NonModelCommand".to_owned(),),
            ("what".to_owned(), "&str".to_owned()),
        ]
    );
    assert_eq!(
        function_shape(&non_model, "run_with_stdin").1,
        [
            ("NonModelCommand".to_owned(), "NonModelCommand".to_owned(),),
            ("stdin_payload".to_owned(), "&str".to_owned()),
            ("limit".to_owned(), "Duration".to_owned()),
            ("what".to_owned(), "&str".to_owned()),
        ]
    );
    assert_eq!(
        function_shape(&non_model, "run_sync_bounded").1,
        [
            (
                "SyncNonModelCommand".to_owned(),
                "SyncNonModelCommand".to_owned(),
            ),
            ("limit".to_owned(), "Duration".to_owned()),
            ("what".to_owned(), "&str".to_owned()),
        ]
    );

    let (deliver_is_private, deliver_parameters) = function_shape(&agent, "deliver_with_stdin");
    assert!(
        deliver_is_private,
        "raw model stdin transport became reachable"
    );
    assert_eq!(
        deliver_parameters,
        [
            ("command".to_owned(), "Command".to_owned()),
            ("stdin_payload".to_owned(), "&str".to_owned()),
            ("limit".to_owned(), "Duration".to_owned()),
            ("what".to_owned(), "&str".to_owned()),
        ]
    );
    let typed_deliver = function_shape(&agent, "deliver").1;
    assert_eq!(
        typed_deliver,
        [
            ("command".to_owned(), "AgentCommand".to_owned()),
            ("prompt".to_owned(), "&ModelPrompt".to_owned()),
            ("limit".to_owned(), "Duration".to_owned()),
            ("what".to_owned(), "&str".to_owned()),
        ]
    );
    assert_eq!(
        boundary_events(&agent)
            .into_iter()
            .filter(|(_, event)| event == "deliver_with_stdin")
            .collect::<Vec<_>>(),
        [("deliver".to_owned(), "deliver_with_stdin".to_owned())],
        "raw model stdin transport must have exactly one typed caller"
    );

    let (replacement_is_private, replacement_parameters) = function_shape(&replacement, "spawn");
    assert!(
        !replacement_is_private,
        "replacement seam unexpectedly changed visibility shape"
    );
    assert_eq!(
        replacement_parameters,
        [
            ("replacement_binary".to_owned(), "&Path".to_owned()),
            ("args".to_owned(), "&".to_owned()),
        ]
    );
    assert_eq!(
        boundary_events(&replacement),
        [
            ("spawn".to_owned(), "is_provider_program".to_owned()),
            ("spawn".to_owned(), "is_provider_program".to_owned()),
            ("spawn".to_owned(), "spawn".to_owned()),
        ],
        "replacement validation must dominate its sole process spawn"
    );

    assert_eq!(
        command_flow(&agent, "deliver"),
        [
            "bind:command",
            "bind:command",
            "struct-bind:AgentCommand:command=command,framing=framing",
        ]
    );
    assert_eq!(command_flow(&agent, "deliver_with_stdin"), ["bind:command"]);
    assert_eq!(
        command_flow(&agent, "probe"),
        ["bind:command", "tuple-bind:ProviderProbeCommand:command",]
    );
    for function in ["run_for", "run_status", "run_with_stdin"] {
        assert_eq!(
            command_flow(&non_model, function),
            ["bind:command", "tuple-bind:NonModelCommand:command"],
            "{function} no longer executes the command unwrapped from its checked capability"
        );
    }
    assert_eq!(
        command_flow(&non_model, "run_sync_bounded"),
        ["bind:command", "tuple-bind:SyncNonModelCommand:command"]
    );
    assert_eq!(
        command_flow(&replacement, "spawn"),
        [
            "bind:command",
            "command-new:tokio::process::Command::new:replacement_binary",
        ],
        "replacement execution must use the path that passed provider validation"
    );
}

#[test]
fn associated_function_alias_seed_defeats_spelling_census_but_is_configured() {
    let seeded_bypass = r#"
        async fn bypass() {
            type Process = tokio::process::Command;
            let executable = contributor_selected_program();
            let mut command = Process::new(executable);
            let invoke = Process::output;
            invoke(&mut command).await;
        }
    "#;
    assert!(
        PROCESS_METHODS
            .iter()
            .all(|method| !seeded_bypass.contains(method)),
        "the adversarial seed must remain independent of method-call spelling"
    );
    assert!(
        configured_disallowed_methods().contains(&"tokio::process::Command::output".to_owned()),
        "the symbol-resolved guard does not cover the adversarial method"
    );
    assert!(
        contains_process_execution_syntax(seeded_bypass),
        "the all-configuration token/AST census missed the associated-function alias"
    );
}

#[test]
fn lint_control_census_detects_multiline_and_broad_suppression() {
    let seeded_bypass = concat!(
        "const RAW: &str = r#\"embedded \" quote\"#;\n",
        "#[allow(\n",
        "    clippy :: all,\n",
        "    reason = \"hide a definition-resolved process-execution finding\"\n",
        ")]\n",
        "fn bypass(executable: &str) {\n",
        "    type Process = tokio::process::Command;\n",
        "    let mut command = Process::new(executable);\n",
        "    let invoke = Process::output;\n",
        "    let _future = invoke(&mut command);\n",
        "}\n",
    );
    let controls = lint_controls(seeded_bypass, "allow");
    assert_eq!(controls.len(), 1);
    assert!(weakens_process_lint(&controls[0]));
    assert!(contains_process_execution_syntax(seeded_bypass));

    let module_scope = r#"
        #![expect(clippy::disallowed_methods)]
        fn seam(mut command: Process) { let _ = command.spawn(); }
    "#;
    let (_, _, controls) = process_scan_with_context(module_scope, None, None);
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].owner, "");
    assert_eq!(controls[0].level, "expect");
}

#[test]
fn crate_root_deny_must_be_an_actual_inner_attribute() {
    let string_only = r##"
        const CLAIM: &str = "#![cfg_attr(not(test), deny(clippy::disallowed_methods))]";
        fn main() {}
    "##;
    assert!(!has_process_lint_deny_at_crate_root(string_only));
    assert!(has_process_lint_deny_at_crate_root(
        "#![cfg_attr(not(test), deny(clippy::disallowed_methods))]\nfn main() {}"
    ));
}

#[test]
fn code_aware_census_normalizes_raw_identifiers() {
    let seeded_bypass = r#"
        #[r#allow(clippy::style)]
        fn bypass(mut command: tokio::process::Command) {
            type Process = tokio::process::Command;
            let invoke = Process::r#output;
            let _future = invoke(&mut command);
        }
    "#;
    let controls = lint_controls(seeded_bypass, "allow");
    assert_eq!(controls.len(), 1);
    assert!(weakens_process_lint(&controls[0]));
    assert!(contains_process_execution_syntax(seeded_bypass));
}

#[test]
fn code_aware_census_reads_inactive_cfg_and_macro_aliases() {
    let inactive = r#"
        #[cfg(windows)]
        fn bypass(mut command: tokio::process::Command) {
            type Process = tokio::process::Command;
            let invoke = Process::output;
            let _future = invoke(&mut command);
        }
    "#;
    assert!(contains_process_execution_syntax(inactive));

    let macro_alias = r#"
        macro_rules! bypass {
            ($method:ident, $command:expr) => {{
                let invoke = std::process::Command::$method;
                invoke($command)
            }};
        }
        bypass!(status, &mut command);
    "#;
    assert!(contains_process_execution_syntax(macro_alias));

    let dependency_macro_alias = r#"
        #[cfg(windows)]
        fn bypass(mut command: tokio::process::Command) {
            invoke_method!(Process, output, &mut command);
        }
    "#;
    assert!(
        contains_process_execution_syntax(dependency_macro_alias),
        "a dependency macro can substitute a bare method argument outside this repository"
    );

    let safe_spawn_alias = r#"
        #[cfg(windows)]
        fn bypass(mut command: std::process::Command) {
            use std::process::Command as replacement;
            let invoke = replacement::spawn;
            let _ = invoke(&mut command);
        }
    "#;
    assert!(
        contains_process_execution_syntax(safe_spawn_alias),
        "a Command alias cannot inherit a trusted task-spawn spelling"
    );

    let qualified_self_alias = r#"
        #[cfg(windows)]
        fn bypass(mut command: tokio::process::Command) {
            let invoke = <tokio::process::Command>::output;
            let _ = invoke(&mut command);
        }
    "#;
    assert!(
        contains_process_execution_syntax(qualified_self_alias),
        "qualified-self associated methods remain process execution sites"
    );

    let local_macro_minted_alias = r#"
        macro_rules! mint {
            () => {
                #[allow(non_camel_case_types)]
                type replacement = std::process::Command;
            };
        }
        mint!();
        #[cfg(windows)]
        fn bypass(mut command: std::process::Command) {
            let invoke = replacement::spawn;
            let _ = invoke(&mut command);
        }
    "#;
    assert!(
        contains_process_execution_syntax(local_macro_minted_alias),
        "a local macro cannot mint a trusted-looking process alias"
    );

    let dependency_macro_minted_alias = r#"
        mint_alias!(replacement, std::process::Command);
        #[cfg(windows)]
        fn bypass(mut command: std::process::Command) {
            let invoke = replacement::spawn;
            let _ = invoke(&mut command);
        }
    "#;
    assert!(
        contains_process_execution_syntax(dependency_macro_minted_alias),
        "a dependency macro cannot receive a trusted-looking process alias"
    );

    let dependency_macro_fixed_alias = r#"
        use std::process::Command as Process;
        evil_exec_macros::mint_replacement_alias!();
        #[cfg(windows)]
        fn bypass(mut command: Process) {
            let invoke = replacement::spawn;
            let _ = invoke(&mut command);
        }
    "#;
    assert!(
        contains_process_execution_syntax(dependency_macro_fixed_alias),
        "a zero-argument dependency macro cannot hide behind a textual safe-spawn path"
    );

    let nested_safe_spawn = r#"
        fn bypass(mut command: Process) {
            let _ = format!("{:?}", replacement::spawn(&mut command));
        }
    "#;
    assert!(
        contains_process_execution_syntax(nested_safe_spawn),
        "a safe-data macro cannot hide an associated spawn token"
    );

    let reusable_method = r#"
        fn bypass(mut command: Process) {
            let invoke = Process::spawn;
            let _first = invoke(&mut command);
            let _second = invoke(&mut command);
        }
    "#;
    let (sites, safe_spawns, _) = process_scan_with_context(reusable_method, None, None);
    assert_eq!(sites[0].method, "associated-reference:spawn");
    assert!(safe_spawns.is_empty());

    let reusable_task_spawn = r#"
        fn bypass(task: Task) {
            let invoke = tokio::spawn;
            let _first = invoke(task);
        }
    "#;
    let (sites, safe_spawns, _) = process_scan_with_context(reusable_task_spawn, None, None);
    assert_eq!(sites[0].method, "associated-reference:spawn");
    assert!(safe_spawns.is_empty());

    assert!(contains_process_execution_syntax(
        "#[cfg(windows)] include!(\"generated_process.inc\");"
    ));
}

#[test]
fn macro_classifier_allows_only_proved_data_macros() {
    for shadow in [
        r#"
            macro_rules! warn { ($method:ident) => {} }
            fn bypass() { warn!(output); }
        "#,
        r#"
            macro_rules! format { ($method:ident) => {} }
            fn bypass() { format!(output); }
        "#,
        r#"
            use evil::invoke as warn;
            fn bypass() { warn!(output); }
        "#,
        r#"
            use evil::*;
            fn bypass() { invoke_method!(Process, output, &mut command); }
        "#,
        r#"
            fn bypass() {
                let _ = format!("{}", invoke_method!(Process, output, &mut command));
            }
        "#,
        r#"
            use evil_exec_macros as tracing;
            fn bypass() { tracing::warn!(Process, output, &mut command); }
        "#,
        r#"
            extern crate evil_exec_macros as tracing;
            fn bypass() { tracing::warn!(Process, output, &mut command); }
        "#,
        r#"
            use evil_exec_macros as manual_handlers;
            use manual_handlers::*;
            fn bypass() { warn!(output); }
        "#,
        r#"
            #[macro_use]
            extern crate evil_exec_macros;
            fn bypass() { warn!(output); }
        "#,
        r#"
            #[cfg_attr(windows, macro_use)]
            extern crate evil_exec_macros;
            #[cfg(windows)]
            fn bypass() { warn!(output); }
        "#,
        r#"
            fn bypass() {
                tokio::select! {
                    _ = replacement::spawn(&mut command) => {}
                }
            }
        "#,
        r#"
            #[cfg(windows)]
            fn bypass() { evil_exec_macros::run_process!(); }
        "#,
        r#"
            #[cfg(windows)]
            #[evil_exec_macros::run_process]
            fn bypass() {}
        "#,
        r#"
            #[cfg(windows)]
            #[derive(evil_exec_macros::RunProcess)]
            struct Bypass;
        "#,
        r#"
            #[cfg_attr(windows, evil_exec_macros::run_process)]
            fn bypass() {}
        "#,
    ] {
        assert!(
            contains_process_execution_syntax(shadow),
            "safe-data macro shadow escaped the classifier: {shadow}"
        );
    }

    let approved = r#"
        use anyhow::anyhow;
        use tracing::{error, warn};
        async fn report(status: Status, output: Output, delay: Delay) {
            warn!("status: {}", status);
            error!("output: {}", output);
            tracing::warn!("status: {}", status);
            let _ = anyhow!("status: {status}");
            let _ = format!("{status}");
            let _ = matches!(status, Status::Ready);
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
            }
        }
    "#;
    assert!(
        !contains_process_execution_syntax(approved),
        "proved formatting/logging macros became process capabilities"
    );

    let dependency_macro = r#"
        #[cfg(windows)]
        fn bypass(mut command: Process) {
            invoke_method!(Process, output, &mut command);
        }
    "#;
    assert!(contains_process_execution_syntax(dependency_macro));
}

#[test]
fn test_only_attributes_prune_entire_ast_subtrees() {
    for source in [
        r#"
            #[cfg(test)]
            impl Runner {
                fn fixture(mut command: Process) { let _ = command.spawn(); }
            }
        "#,
        r#"
            fn production() {
                #[cfg(test)]
                { let mut command = Process::new("agy"); let _ = command.spawn(); }
            }
        "#,
        r#"
            fn production(value: bool) {
                match value {
                    #[cfg(test)]
                    true => { let mut command = Process::new("agy"); let _ = command.spawn(); }
                    false => {}
                }
            }
        "#,
    ] {
        assert!(
            !contains_process_execution_syntax(source),
            "cfg(test) subtree was classified as shipped execution: {source}"
        );
    }
}

#[test]
fn raw_model_stdin_aliases_and_agent_capability_minting_are_visible() {
    let aliased_transport = r#"
        async fn deliver(command: Command, raw: &str, limit: Duration, what: &str) {
            let invoke = deliver_with_stdin;
            let _ = invoke(command, raw, limit, what).await;
        }
    "#;
    assert_eq!(
        boundary_events(aliased_transport),
        [("deliver".to_owned(), "deliver_with_stdin".to_owned())]
    );

    let aliased_raw_stdin = r#"
        async fn bypass(command: Command, body: &str) {
            let invoke = crate::exec::run_bounded_with_stdin;
            let _ = invoke(command, body, Duration::from_secs(1), "bypass").await;
        }
    "#;
    assert_eq!(
        boundary_events(aliased_raw_stdin),
        [("bypass".to_owned(), "run_bounded_with_stdin".to_owned())]
    );

    let arbitrary_constructor = r#"
        pub(crate) fn arbitrary(tool: &str, posture: &Posture) -> AgentCommand {
            command(tool, posture, Framing::Plain)
        }
        fn literal(command: Command, framing: Framing) -> AgentCommand {
            AgentCommand { command, framing }
        }
    "#;
    let events = agent_capability_events(arbitrary_constructor);
    assert!(
        events
            .iter()
            .any(|(owner, event)| { owner == "arbitrary" && event == "call:command:tool" })
    );
    assert!(events.iter().any(|(owner, event)| {
        owner == "literal" && event.starts_with("construct-agent:AgentCommand:")
    }));

    let substituted_nonmodel = r#"
        async fn run_with_stdin(
            NonModelCommand(_checked): NonModelCommand,
            raw: Command,
        ) {
            let mut command = raw;
            let _ = command.spawn();
        }
    "#;
    assert_ne!(
        command_flow(substituted_nonmodel, "run_with_stdin"),
        ["bind:command", "tuple-bind:NonModelCommand:command"]
    );

    let compensated_model = r#"
        async fn deliver(
            command: AgentCommand,
            prompt: &ModelPrompt,
            limit: Duration,
            what: &str,
            raw: Command,
        ) {
            let AgentCommand { command: _checked, framing } = command;
            let command = raw;
            deliver_with_stdin(command, prompt.as_str(), limit, what).await;
        }
    "#;
    let actual_model = fs::read_to_string(repo().join(MODEL_TRANSPORT)).unwrap();
    assert_ne!(
        function_shape(compensated_model, "deliver").1,
        function_shape(&actual_model, "deliver").1
    );
    assert_ne!(
        command_flow(compensated_model, "deliver"),
        command_flow(&actual_model, "deliver")
    );
}

#[test]
fn module_sources_cannot_escape_the_production_census() {
    for source in [
        "#[cfg(windows)] #[path = \"hidden.inc\"] mod hidden;",
        "#[cfg(windows)] #[path = \"../hidden.rs\"] mod hidden;",
        "#[cfg(windows)] #[cfg_attr(windows, path = \"hidden.rs\")] mod hidden;",
        "#[cfg(windows)] mod tests;",
        "#[cfg(windows)] mod examples;",
        "#[cfg(windows)] mod benches;",
        "#[cfg(windows)] mod target;",
        "mod replacement { pub use evil_dep::spawn; }",
        "struct tokio;",
        "trait replacement {}",
        "fn bypass<replacement>() {}",
    ] {
        assert!(
            contains_process_execution_syntax(source),
            "unscanned module source was accepted: {source}"
        );
    }

    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("root.rs");
    fs::write(
        &root,
        "#[cfg(windows)] #[path = \"missing.rs\"] mod hidden;\n",
    )
    .unwrap();
    let scanned = BTreeSet::from([root.clone()]);
    assert!(
        !process_execution_sites_with_context(
            &fs::read_to_string(&root).unwrap(),
            Some(&root),
            Some(&scanned)
        )
        .is_empty()
    );
}

#[test]
fn cargo_metadata_census_includes_custom_workspace_and_build_roots() {
    let fixture = tempfile::tempdir().unwrap();
    let repository = fixture.path().to_path_buf();
    let custom_lib = repository.join("custom/kernel.rs");
    let member_bin = repository.join("crates/worker/app/main.rs");
    let build_script = repository.join("build.rs");
    let build_helper = repository.join("build_helper.rs");
    let nested_target_source = repository.join("src/target/hidden.rs");
    let excluded_test = repository.join("tests/fixture.rs");
    let excluded_example = repository.join("examples/demo.rs");
    let excluded_target = repository.join("target/debug/build/generated.rs");
    for path in [
        &custom_lib,
        &member_bin,
        &build_script,
        &build_helper,
        &nested_target_source,
        &excluded_test,
        &excluded_example,
        &excluded_target,
    ] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "// fixture\n").unwrap();
    }
    fs::write(&build_script, "#[cfg(windows)] mod build_helper;\n").unwrap();
    fs::write(
        &build_helper,
        "#[cfg(windows)] fn hidden(mut command: std::process::Command) {\n\
         let invoke = std::process::Command::output;\n\
         let _ = invoke(&mut command);\n}\n",
    )
    .unwrap();
    let metadata = serde_json::json!({
        "packages": [
            {
                "manifest_path": repository.join("Cargo.toml").display().to_string(),
                "targets": [
                    {
                        "kind": ["lib"],
                        "src_path": custom_lib.display().to_string(),
                    },
                    {
                        "kind": ["custom-build"],
                        "src_path": build_script.display().to_string(),
                    },
                ],
            },
            {
                "manifest_path": repository.join("crates/worker/Cargo.toml").display().to_string(),
                "targets": [{
                    "kind": ["bin"],
                    "src_path": member_bin.display().to_string(),
                }],
            },
        ],
    });
    assert_eq!(
        targets_from_metadata(&metadata, &repository),
        [
            ProductionTarget {
                path: build_script.clone(),
                custom_build: true,
            },
            ProductionTarget {
                path: member_bin.clone(),
                custom_build: false,
            },
            ProductionTarget {
                path: custom_lib.clone(),
                custom_build: false,
            },
        ]
    );
    assert_eq!(
        source_paths_from_metadata(&metadata, &repository),
        [
            build_script,
            build_helper.clone(),
            member_bin,
            custom_lib,
            nested_target_source,
        ],
        "the filesystem census includes build helpers and nested production `target` modules while excluding Cargo's root tests, examples and target layouts"
    );
    assert!(contains_process_execution_syntax(
        &fs::read_to_string(repository.join("build_helper.rs")).unwrap()
    ));

    let hidden_target = serde_json::json!({
        "packages": [{
            "manifest_path": repository.join("Cargo.toml").display().to_string(),
            "targets": [{
                "kind": ["bin"],
                "src_path": repository.join("tests/shipped.rs").display().to_string(),
            }],
        }],
    });
    assert!(
        std::panic::catch_unwind(|| targets_from_metadata(&hidden_target, &repository)).is_err(),
        "an explicit production target was discarded as though Cargo had classified it as a test"
    );

    let target_fixture = tempfile::tempdir().unwrap();
    let target_repository = target_fixture.path().to_path_buf();
    let target_root = target_repository.join("target/shipped.rs");
    let target_helper = target_repository.join("target/helper.rs");
    fs::create_dir_all(target_root.parent().unwrap()).unwrap();
    fs::write(&target_root, "#[cfg(windows)] mod helper;\n").unwrap();
    fs::write(
        &target_helper,
        "#[cfg(windows)] fn hidden(mut command: std::process::Command) {\n\
         let invoke = std::process::Command::output;\n\
         let _ = invoke(&mut command);\n}\n",
    )
    .unwrap();
    let target_metadata = serde_json::json!({
        "packages": [{
            "manifest_path": target_repository.join("Cargo.toml").display().to_string(),
            "targets": [{
                "kind": ["bin"],
                "src_path": target_root.display().to_string(),
            }],
        }],
    });
    let target_paths = source_paths_from_metadata(&target_metadata, &target_repository);
    assert_eq!(target_paths, std::slice::from_ref(&target_root));
    let scanned_target_paths = target_paths.iter().cloned().collect::<BTreeSet<_>>();
    assert!(
        !process_execution_sites_with_context(
            &fs::read_to_string(&target_root).unwrap(),
            Some(&target_root),
            Some(&scanned_target_paths),
        )
        .is_empty(),
        "an explicit target under Cargo's artifact directory cannot hide cfg-inactive helper modules"
    );

    #[cfg(unix)]
    {
        let linked_source = repository.join("src/hidden.rs");
        fs::create_dir_all(linked_source.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&build_helper, &linked_source).unwrap();
        let result = std::panic::catch_unwind(|| {
            let mut paths = Vec::new();
            rust_sources(&repository, &repository, &mut paths);
        });
        assert!(result.is_err(), "a Rust symlink escaped the closed census");
        fs::remove_file(&linked_source).unwrap();
    }

    let unreadable_as_directory = repository.join("not-a-directory.rs");
    fs::write(&unreadable_as_directory, "// fixture\n").unwrap();
    let result = std::panic::catch_unwind(|| {
        let mut paths = Vec::new();
        rust_sources(&repository, &unreadable_as_directory, &mut paths);
    });
    assert!(
        result.is_err(),
        "a traversal error was treated as an empty tree"
    );

    let case_folded = repository.join("src/hidden.RS");
    fs::write(&case_folded, "// fixture\n").unwrap();
    let result = std::panic::catch_unwind(|| {
        let mut paths = Vec::new();
        rust_sources(&repository, &repository, &mut paths);
    });
    assert!(
        result.is_err(),
        "a case-folding Rust extension escaped the host-platform census"
    );
}

#[test]
fn direct_known_provider_construction_remains_inside_the_finite_provider_seam() {
    let mut bare = Vec::new();
    for (path, source) in all_production_sources() {
        for provider in KNOWN_PROVIDERS {
            let needle = format!("Command::new(\"{provider}\")");
            for (index, _) in source.match_indices(&needle) {
                bare.push((path.clone(), provider.to_string(), index));
            }
        }
    }
    assert!(
        !bare.is_empty(),
        "known-provider probe census found no subject"
    );
    for (path, provider, _) in bare {
        assert!(
            path == PROVIDER_SEAM,
            "raw {provider} construction escaped the finite provider seam at {path}"
        );
    }
}
