//! The authorization the execution prompt states in prose must exist as policy
//! the harness enforces before the tool call.
//!
//! `docs/plan/h1-execution-prompt.md` §1 refuses a set of commands. Until this
//! change, that refusal was English inside the agent's own context -- a policy
//! the agent reads, which every 2026 reference on pre-action authorization says
//! is not a policy. The enforced copy is `policies/agents/claude-settings.json`,
//! installed by `scripts/install-agent-policy.sh`.
//!
//! Two documents stating one rule drift. This test is what stops them: a
//! command the prompt refuses and the policy permits fails here.

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn template() -> String {
    let p = repo().join("policies/agents/claude-settings.json");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("policy template unreadable at {p:?}: {e}"))
}

/// Every command the prompt refuses, and the deny rule that has to cover it.
/// Adding a refusal to the prompt without adding it here leaves the prose
/// enforcing nothing, which is the state this change exists to end.
const MUST_DENY: &[(&str, &str)] = &[
    ("boots the production daemon", "Bash(cargo run)"),
    ("publishes the crate", "Bash(cargo publish*)"),
    ("pushes", "Bash(git push*)"),
    ("erases untracked work", "Bash(git clean*)"),
    ("discards commits", "Bash(git reset --hard*)"),
    ("discards the working tree", "Bash(git checkout -- *)"),
    ("rewrites history", "Bash(git rebase*)"),
    ("deletes a branch", "Bash(git branch -D*)"),
    ("drops stashed work", "Bash(git stash drop*)"),
    (
        "stages everything, including agent dirs",
        "Bash(git add -A*)",
    ),
    ("edits files in place", "Bash(sed -i*)"),
    ("deletes what it finds", "Bash(find * -delete*)"),
    ("executes what it finds", "Bash(find * -execdir*)"),
    ("writes through a sort", "Bash(sort -o*)"),
    ("writes through a pipe", "Bash(tee *)"),
];

#[test]
fn every_refusal_the_prompt_states_is_a_deny_rule_the_harness_enforces() {
    let policy = template();
    for (why, rule) in MUST_DENY {
        assert!(
            policy.contains(rule),
            "the prompt refuses what {why}, but the enforced policy has no `{rule}`. \
             A refusal that lives only in prose is read by the agent it constrains."
        );
    }
}

#[test]
fn the_policy_is_valid_json_with_a_non_empty_deny_list() {
    let policy = template();
    let v: serde_json::Value =
        serde_json::from_str(&policy).expect("policy template must be valid JSON");
    let deny = v["permissions"]["deny"]
        .as_array()
        .expect("permissions.deny must be an array");
    assert!(
        deny.len() >= MUST_DENY.len(),
        "deny list has {} rules, fewer than the {} refusals the prompt states",
        deny.len(),
        MUST_DENY.len()
    );
}

#[test]
fn the_installer_exists_and_is_executable() {
    let p = repo().join("scripts/install-agent-policy.sh");
    let meta = fs::metadata(&p).unwrap_or_else(|e| panic!("installer missing at {p:?}: {e}"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert!(
            meta.permissions().mode() & 0o111 != 0,
            "installer is present but not executable, so nothing runs it"
        );
    }
    let _ = meta;
}

/// The installer declares a row per harness named by pre-commit's own refusal
/// regex. A harness that gains a template must have the file that row names.
#[test]
fn every_declared_template_exists() {
    let script = fs::read_to_string(repo().join("scripts/install-agent-policy.sh"))
        .expect("installer unreadable");
    for line in script.lines() {
        let line = line.trim();
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() != 3 || parts[2] == "-" || !parts[1].starts_with('.') {
            continue;
        }
        let tmpl = repo().join("policies/agents").join(parts[2]);
        assert!(
            Path::new(&tmpl).is_file(),
            "harness `{}` declares template `{}`, which does not exist",
            parts[0],
            parts[2]
        );
    }
}
