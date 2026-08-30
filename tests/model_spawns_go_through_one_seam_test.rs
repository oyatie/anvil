//! A model turn is spawned in one place, or the environment decision is optional.
//!
//! Six sites spawned a provider CLI and none of them called `env_clear`, so an
//! agent acting on a prompt built from an attacker's diff held
//! `GITHUB_WEBHOOK_SECRET`, `GITHUB_TOKEN` and `SSH_AUTH_SOCK`. That was not six
//! oversights; it was the default, and a bare `Command::new` will make it the
//! default again at the seventh site.
//!
//! `exec::agent` is the seam. This refuses a spawn that skips it, so the next
//! model turn cannot be written without the isolation decision — the class,
//! rather than the six instances.

use std::fs;
use std::path::{Path, PathBuf};

/// Command names that start a model turn.
const PROVIDER_CLIS: &[&str] = &[
    "agy",
    "claude",
    "codex",
    "cursor",
    "cursor-agent",
    "gemini",
    "grok",
];

/// Flags that hand a provider CLI something to act on. A spawn carrying one of
/// these is a model turn; a spawn carrying none is a presence probe.
const PROMPT_FLAGS: &[&str] = &[
    "--print",
    "--prompt",
    "--prompt-file",
    "--input-format",
    "\"-p\"",
    "\"exec\"",
    "\"agent\"",
];

/// The seam itself, which is where the bare `Command::new` is supposed to be.
const SEAM: &str = "src/exec/agent.rs";

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Production Rust under `src/`, with `#[cfg(test)]` modules removed.
///
/// A fixture that builds a `Command` only to read its argv back is not a spawn
/// site, and judging it as one would push every such fixture out of the module
/// it tests. `without_test_modules` is the stripper the rest of this codebase
/// already relies on, rather than a fourth hand-rolled one.
fn production(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|b| anvil::source_scan::without_test_modules(&b))
}

/// Every `Command::new("<provider>")` in production `src/`, as (path, the 400
/// characters that follow it).
fn provider_spawns() -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_sources(&repo().join("src"), &mut files);
    files.sort();
    let mut found = Vec::new();
    for p in files {
        let rel = p
            .strip_prefix(repo())
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        let Some(body) = production(&p) else {
            continue;
        };
        for cli in PROVIDER_CLIS {
            let needle = format!("Command::new(\"{cli}\")");
            for (at, _) in body.match_indices(&needle) {
                let tail: String = body[at..].chars().take(400).collect();
                found.push((rel.clone(), tail));
            }
        }
    }
    found
}

/// The scan must be able to find its subject, or it reports nothing wrong with
/// anything.
#[test]
fn the_seam_holds_the_only_bare_provider_spawn() {
    let spawns = provider_spawns();
    assert!(
        !spawns.is_empty(),
        "the scan matched no provider spawn anywhere, so it would pass whatever \
         the tree did"
    );
    let seam = std::fs::read_to_string(repo().join(SEAM)).expect("the seam exists");
    assert!(
        seam.contains("Command::new(tool)"),
        "{SEAM} no longer constructs the command it is the seam for. Either the \
         seam moved — in which case this test must follow it — or a spawn site \
         has nowhere left to go. Provider spawns found: {:?}",
        spawns.iter().map(|(r, _)| r).collect::<Vec<_>>()
    );
}

#[test]
fn no_model_turn_is_spawned_outside_the_seam() {
    let offenders: Vec<String> = provider_spawns()
        .into_iter()
        .filter(|(rel, _)| rel != SEAM)
        .filter(|(_, tail)| PROMPT_FLAGS.iter().any(|f| tail.contains(f)))
        .map(|(rel, tail)| {
            format!(
                "{rel}: {}",
                tail.lines().take(3).collect::<Vec<_>>().join(" ").trim()
            )
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "a model turn is spawned outside `exec::agent`:\n{}\n\
         A bare `Command::new` inherits the daemon's whole environment, so this \
         spawn hands `GITHUB_TOKEN` and `SSH_AUTH_SOCK` to a process acting on \
         a prompt an outsider helped write. Build it with \
         `exec::agent(tool, &Posture::in_workspace(dir))`, which has no \
         `Default` and so cannot be written without saying where the turn runs.",
        offenders.join("\n")
    );
}

/// The preflight probe is the one exception, and it is an exception because it
/// hands the CLI nothing to act on — not because of where it lives.
#[test]
fn the_only_bare_spawn_outside_the_seam_carries_no_prompt() {
    let bare: Vec<(String, String)> = provider_spawns()
        .into_iter()
        .filter(|(rel, _)| rel != SEAM)
        .collect();

    for (rel, tail) in &bare {
        let statement = tail.split(';').take(2).collect::<Vec<_>>().join(";");
        assert!(
            statement.contains("--help") || statement.contains("--version"),
            "{rel} spawns a provider CLI outside the seam and does not ask it \
             for its help or version:\n  {}\n\
             A spawn that is not a presence probe is a model turn, and a model \
             turn goes through `exec::agent`.",
            statement.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }
}

/// No prompt travels on argv.
///
/// argv is world-readable through `ps` and is recorded by process accounting.
/// The prompts these sites build carry the diff, review-comment bodies, merge
/// conflict text and pull-request titles -- text an outsider wrote -- so a
/// prompt on argv discloses attacker-supplied content to every process on the
/// host, and Anvil's own reasoning about it alongside.
///
/// Keyed to the shape rather than to a file: `--print` immediately followed by
/// anything that is not the empty string is a prompt on argv, whoever wrote it.
#[test]
fn no_prompt_is_passed_as_a_command_line_argument() {
    let mut files = Vec::new();
    rust_sources(&repo().join("src"), &mut files);
    files.sort();

    let mut offenders = Vec::new();
    let mut checked = 0usize;
    for p in files {
        let rel = p
            .strip_prefix(repo())
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        let Some(body) = production(&p) else {
            continue;
        };
        for (at, _) in body.match_indices("\"--print\",") {
            checked += 1;
            // The next non-blank token after the flag is its value.
            let after: String = body[at + "\"--print\",".len()..].chars().take(80).collect();
            let value = after.split([',', ']']).next().unwrap_or("").trim();
            if value != "\"\"" {
                offenders.push(format!("{rel}: --print {value}"));
            }
        }
    }

    assert!(
        checked > 0,
        "no `--print` argument was found anywhere under src/. Either the flag \
         is spelled differently now -- in which case this test must follow it -- \
         or the scan would pass whatever the tree did."
    );
    assert!(
        offenders.is_empty(),
        "a prompt is passed on the command line:\n{}\n\
         argv is world-readable through `ps`, and these prompts carry text an \
         outsider wrote. Build the turn with `exec::turn::agy_turn`, which \
         passes `--print \"\"` and delivers the prompt on STDIN as a \
         stream-json message.",
        offenders.join("\n")
    );
}
