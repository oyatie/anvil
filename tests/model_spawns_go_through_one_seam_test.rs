//! Structural ratchets for the typed model-command boundary.
//!
//! The compiler owns the primary guarantee: only `exec::agent::provider` can mutate
//! `AgentCommand`, and the public model transport accepts no raw command or raw
//! prompt. These source censuses keep the few intentionally raw subprocess
//! seams finite so a new provider cannot quietly grow beside that boundary.

use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER_SEAM: &str = "src/exec/agent/provider.rs";
const MODEL_TRANSPORT: &str = "src/exec/agent/transport.rs";
const NON_MODEL_TRANSPORT: &str = "src/exec/non_model.rs";
const DIRECT_EXECUTION_SEAMS: &[&str] = &[
    "src/exec/agent/transport.rs",
    "src/exec/non_model/transport.rs",
    "src/exec/replacement.rs",
];
const PROCESS_METHODS: &[&str] = &[".exec()", ".output()", ".spawn()", ".status()"];
const PROCESS_UFCS: &[&str] = &[
    "Command::output(",
    "Command::spawn(",
    "Command::status(",
    "CommandExt::exec(",
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
const RAW_STDIN_ALLOWLIST: &[&str] = &[
    "src/cedar_guard.rs",
    "src/ci_triager/publication.rs",
    "src/exec/mod.rs",
    "src/shape/adapters/git_tree_at_rev.rs",
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn production(path: &Path) -> String {
    let source = fs::read_to_string(path).unwrap_or_default();
    let source = anvil::source_scan::without_test_modules(&source);
    anvil::source_scan::without_commentary(&source)
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn all_production_sources() -> Vec<(String, String)> {
    let mut paths = Vec::new();
    rust_sources(&repo().join("src"), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .filter(|path| {
            path.file_name().is_none_or(|name| name != "tests.rs")
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "tests")
        })
        .map(|path| (relative(&path), production(&path)))
        .collect()
}

#[test]
fn only_the_finite_provider_seam_constructs_agent_commands() {
    let mut callers = Vec::new();
    for (path, source) in all_production_sources() {
        for _ in source.match_indices("super::command(") {
            callers.push(path.clone());
        }
    }
    assert!(!callers.is_empty(), "agent-command census found no subject");
    assert!(
        callers.iter().all(|path| path == PROVIDER_SEAM),
        "AgentCommand construction escaped the finite seam: {callers:#?}"
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
    for (path, source) in all_production_sources() {
        if source.contains("run_bounded_with_stdin(") {
            callers.push(path);
        }
    }
    callers.sort();
    assert_eq!(callers, RAW_STDIN_ALLOWLIST);

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

    let vocabulary = transport
        .split("const NON_MODEL_PROGRAMS")
        .nth(1)
        .and_then(|tail| tail.split("];").next())
        .expect("finite non-model vocabulary");
    for forbidden in ["env", "sh", "bash", "zsh"] {
        assert!(
            !vocabulary.contains(&format!("\"{forbidden}\"")),
            "raw helper {forbidden} reopened direct provider indirection"
        );
    }
    for required in ["git", "gh", "cargo", "curl"] {
        assert!(
            vocabulary.contains(&format!("\"{required}\"")),
            "production tool {required} fell out of the finite seam"
        );
    }
}

/// Spelling-independent primary guard: no production module can execute any
/// `Command`, provider-named or variable/aliased, outside the private checked
/// exec seams. The narrower literal census below is secondary evidence only.
#[test]
fn every_production_process_execution_is_inside_a_private_exec_seam() {
    let mut execution_paths = Vec::new();
    for (path, source) in all_production_sources() {
        if PROCESS_METHODS.iter().any(|method| source.contains(method)) {
            execution_paths.push(path.clone());
        }
        for ufcs in PROCESS_UFCS {
            assert!(
                !source.contains(ufcs),
                "UFCS process execution bypass outside the checked method census at {path}: {ufcs}"
            );
        }
    }
    execution_paths.sort();
    assert_eq!(execution_paths, DIRECT_EXECUTION_SEAMS);
}

#[test]
fn invocation_census_detects_variable_provider_exec_without_provider_spelling() {
    let seeded_bypass = r#"
        use std::os::unix::process::CommandExt;
        let executable = contributor_selected_program();
        std::process::Command::new(executable).exec();
    "#;
    assert!(
        PROCESS_METHODS
            .iter()
            .any(|method| seeded_bypass.contains(method)),
        "the production census missed CommandExt::exec with a variable program"
    );

    let seeded_ufcs = "CommandExt::exec(&mut command);";
    assert!(
        PROCESS_UFCS
            .iter()
            .any(|method| seeded_ufcs.contains(method)),
        "the production census missed qualified CommandExt::exec"
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
