//! An outbound network tool is spawned in one place, or the environment
//! decision is optional.
//!
//! `curl` is the fourth outbound tool class, after a model turn, a build and a
//! forge call, and the last one to get a seam. A bare `Command::new` hands the
//! transport the daemon's whole environment -- `GITHUB_WEBHOOK_SECRET`,
//! `GH_TOKEN`, every model provider key -- into a process that resolves a name,
//! opens a socket and writes a body.
//!
//! # What this is and is not
//!
//! Lower severity than the build seam, and this file will not pretend
//! otherwise. `exec::build_env` bounds a process that runs a CONTRIBUTOR'S
//! `#[test]` code, and a test can read an environment variable. Nothing
//! comparable holds here: the argv is fixed by the caller and no payload makes
//! `curl` print its environment. What this refuses is the default -- the next
//! network call written in the transport module inheriting everything because
//! the bare spelling is the shortest one.

use anvil::source_scan::paths::{is_test_source, module_source};
use anvil::source_scan::{without_commentary, without_test_modules};
use std::fs;
use std::path::{Path, PathBuf};

/// The seam, as a module rather than a file: this tree splits files routinely
/// and a filename-keyed check goes blind rather than red the day it happens.
const SEAM: &str = "src/exec/net";

/// The module that makes the outbound request, keyed the same way.
const TRANSPORT: &str = "src/supply_chain_guard";

/// Programs whose whole job is to talk to something off-box.
///
/// Not a census of every binary that can open a socket -- `git` and `gh` can
/// too, and they have their own seams. These are the general-purpose
/// transports, which is the class `exec::net` exists for.
const NETWORK_TOOLS: &[&str] = &["curl", "wget", "httpie", "http", "nc"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

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

/// Names a file binds to a network tool, so `Command::new(CURL)` is judged the
/// same as `Command::new("curl")`.
///
/// The transport spells it exactly that way -- `const CURL: &str = "curl"` and
/// `post_json(CURL, ..)` -- so a scan that knew only the literal would be blind
/// to the one shape this repository actually writes.
fn tool_aliases(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let Some((lhs, rhs)) = line.split_once('=') else {
            continue;
        };
        let value = rhs.trim().trim_end_matches(';').trim();
        let Some(tool) = value
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
        else {
            continue;
        };
        if !NETWORK_TOOLS.contains(&tool) {
            continue;
        }
        let decl = lhs.trim().trim_start_matches("pub ").trim();
        let Some(rest) = decl
            .strip_prefix("const ")
            .or_else(|| decl.strip_prefix("static "))
            .or_else(|| decl.strip_prefix("let "))
        else {
            continue;
        };
        let name = rest
            .split(':')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_start_matches("mut ")
            .trim();
        if !name.is_empty() {
            out.push(name.to_string());
        }
    }
    out
}

/// Every `Command::new` in `src` whose program is a network tool, by the text
/// of its argument.
///
/// `without_commentary` rather than `code_only`: the needle is spelled as a
/// string literal, and stripping literal bodies would blank the very thing this
/// looks for. Commentary still goes, because the seam's own module
/// documentation names both `Command::new` and `curl` in prose, and a scan that
/// reads a sentence as a call site accuses the fix of being the defect.
///
/// A nested call in the argument position (`Command::new(pick(x))`) is read up
/// to the first `)` and will not match. Stated rather than implied: it cannot
/// invent a hit, and no such spelling exists in this tree.
fn network_spawns(src: &str) -> Vec<String> {
    let body = without_commentary(&without_test_modules(src));
    let aliases = tool_aliases(&body);
    let mut hits = Vec::new();
    for (at, needle) in body.match_indices("Command::new(") {
        let rest = &body[at + needle.len()..];
        let Some(end) = rest.find(')') else {
            continue;
        };
        let arg = rest[..end].trim();
        let named = arg.trim_matches('"');
        if NETWORK_TOOLS.contains(&named) || aliases.iter().any(|a| a == named) {
            hits.push(arg.to_string());
        }
    }
    hits
}

/// Whether this path is the seam's own source, as a file or as a directory.
fn is_seam(rel: &str) -> bool {
    rel == format!("{SEAM}.rs") || rel.starts_with(&format!("{SEAM}/"))
}

/// Network-tool spawns outside the seam, as `path: argument`.
fn offenders() -> Vec<String> {
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
        if is_test_source(&rel) || is_seam(&rel) {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&p) else {
            continue;
        };
        for arg in network_spawns(&raw) {
            found.push(format!("{rel}: {arg}"));
        }
    }
    found
}

/// The instrument, before its verdict.
///
/// The live tree has zero hits, so this scan's silence is indistinguishable
/// from a scan that cannot match anything at all. Invariant I1 runs in both
/// directions, and this is the direction where absent evidence would read as a
/// pass. The fixture carries both spellings the tree can write.
#[test]
fn the_scan_can_see_what_it_refuses() {
    let fixture = concat!(
        "const CURL: &str = \"curl\";\n",
        "fn direct() { let _ = std::process::Command::new(\"curl\"); }\n",
        "fn aliased() { let _ = tokio::process::Command::new(CURL); }\n",
        "fn innocent() { let _ = Command::new(\"cargo\"); }\n",
    );
    assert_eq!(
        network_spawns(fixture),
        vec!["\"curl\"".to_string(), "CURL".to_string()],
        "the scan must find both the literal and the aliased spawn, and must \
         not accuse a build tool of being a transport"
    );
}

/// The seam must be findable, or everything below reports nothing wrong.
///
/// `module_source` panics when the module is absent, which is the behaviour
/// this wants: a seam renamed away must fail loudly rather than leave a scan
/// pointing at nothing.
#[test]
fn the_seam_holds_the_bare_spawn() {
    let seam = without_commentary(&module_source(SEAM, &repo()));
    assert!(
        seam.contains("Command::new(program)"),
        "`exec::net` no longer constructs the command, so there is nothing for \
         the callers below to be routed through"
    );
    assert!(
        seam.contains("env_clear()"),
        "`exec::net` hands over the daemon's whole environment, which is the \
         condition this seam exists to end"
    );
}

/// The transport goes through the seam, and holds no spawn of its own.
///
/// Stated as "no `Command::new` at all in this module" rather than "no `curl`":
/// the next outbound call written here will be a different tool with the same
/// unbounded default, and naming the tool would let it through.
#[test]
fn the_transport_module_spawns_only_through_the_seam() {
    let src = without_commentary(&module_source(TRANSPORT, &repo()));
    assert!(
        src.contains("crate::exec::net(program)"),
        "the OSV request no longer goes through `exec::net`, so it carries the \
         webhook secret and every provider key to a public advisory database"
    );
    assert!(
        !src.contains("Command::new("),
        "`{TRANSPORT}` builds a subprocess of its own. A bare `Command::new` \
         inherits the daemon's whole environment; build it with \
         `crate::exec::net(program)`."
    );
}

#[test]
fn no_network_tool_is_spawned_outside_the_seam() {
    let found = offenders();
    assert!(
        found.is_empty(),
        "a network tool is spawned outside `exec::net`: {found:?}\n\
         A bare `Command::new` inherits the daemon's whole environment, so the \
         request carries the webhook secret and every provider key off-box. \
         Build it with `crate::exec::net(program)`."
    );
}

/// The list is an allowlist, so what is NOT on it is the assertion.
#[test]
fn the_seam_hands_over_no_secret_a_transport_has_no_use_for() {
    for forbidden in anvil::exec::net::NEVER_HANDED_OVER {
        assert!(
            !anvil::exec::net::NET_INHERITED.contains(forbidden),
            "{forbidden} reaches a general-purpose network tool, which has no \
             use for it."
        );
    }
}

/// The forge credential belongs at one seam, and this completes the claim.
///
/// `gh_spawns_go_through_one_seam_test` asserts it reaches `exec::gh` and not
/// `exec::build_env`. With a third seam holding an inherit list, that pair no
/// longer says "only", so the third leg is asserted rather than left implied.
#[test]
fn the_forge_credential_does_not_reach_a_transport() {
    for name in ["GH_TOKEN", "GITHUB_TOKEN"] {
        assert!(
            anvil::exec::gh::GH_INHERITED.contains(&name),
            "{name} is how a deployment without a keyring authenticates `gh`."
        );
        assert!(
            !anvil::exec::net::NET_INHERITED.contains(&name),
            "{name} must not reach a transport talking to a public endpoint \
             that authenticates nobody."
        );
    }
}

/// A real child, not the list.
///
/// `/usr/bin/env` rather than `curl`: the subject is the environment the seam
/// hands over, and a runner without `curl` installed must still measure it.
/// The proxy half is asserted positively -- inheriting `HTTPS_PROXY` is the one
/// case where handing something over is the correct answer, and a seam that
/// dropped it would send every request nowhere behind a corporate egress.
#[tokio::test]
async fn a_real_child_keeps_its_proxy_and_receives_no_secret() {
    let sentinel = "anvil-net-seam-sentinel";
    unsafe {
        std::env::set_var("GITHUB_WEBHOOK_SECRET", sentinel);
        std::env::set_var("GH_TOKEN", sentinel);
        std::env::set_var("ANTHROPIC_API_KEY", sentinel);
        std::env::set_var("HTTPS_PROXY", sentinel);
    }

    let mut cmd = tokio::process::Command::new("/usr/bin/env");
    anvil::exec::net::apply(&mut cmd);
    let out = cmd.output().await.expect("env runs");
    let seen = String::from_utf8_lossy(&out.stdout).to_string();

    unsafe {
        std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        std::env::remove_var("GH_TOKEN");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("HTTPS_PROXY");
    }

    let leaked: Vec<&str> = anvil::exec::net::NEVER_HANDED_OVER
        .iter()
        .copied()
        .filter(|n| seen.lines().any(|l| l.starts_with(&format!("{n}="))))
        .collect();
    assert!(
        leaked.is_empty(),
        "{leaked:?} reached a general-purpose network tool"
    );
    assert!(
        seen.lines().any(|l| l.starts_with("PATH=")),
        "PATH did not survive, so the tool itself would not be found and every \
         request would fail as a spawn error rather than a network one"
    );
    assert!(
        seen.lines().any(|l| l == format!("HTTPS_PROXY={sentinel}")),
        "HTTPS_PROXY did not reach the child, so a deployment behind a \
         corporate egress sends every request nowhere and reads the failure as \
         an outage at the far end"
    );
}
