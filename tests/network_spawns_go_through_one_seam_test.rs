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
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::Visit;

/// The seam, as a module rather than a file: this tree splits files routinely
/// and a filename-keyed check goes blind rather than red the day it happens.
const SEAM: &str = "src/exec/net";

/// The shared rebinder that preserves an explicit environment clear while it
/// replaces a requested program with its already-validated executable.
const NON_MODEL_SEAM: &str = "src/exec/non_model";

/// The module that makes the outbound request, keyed the same way.
const TRANSPORT: &str = "src/supply_chain_guard";

/// These four files are the complete finite OSV request chain. The structural
/// census below catches a new connection site elsewhere; these fingerprints
/// make any change to the permitted command, destination, payload derivation,
/// admission, or response handling require an explicit whole-chain review.
const REVIEWED_NETWORK_BOUNDARY: &[(&str, &str)] = &[
    (
        // Reviewed for #216. The only change to this file is `muse_agent` joining
        // the provider re-export list. Nothing in the OSV request chain -- the
        // permitted command, destination, payload derivation, admission or
        // response handling -- is touched.
        //
        // That a one-word export change demands a whole-chain re-review is worth
        // recording: this fingerprint covers the entire file, which holds both
        // the network seam and the provider re-exports. A check that fires on
        // unrelated edits is one that eventually gets updated without being read.
        "src/exec/mod.rs",
        "82a0f9877c9c13adb74eadab816e465c0c4c8c43c69622e53a807f97de326ba7",
    ),
    (
        "src/exec/net.rs",
        "9b2dcbf22054c01f02185ff4315e98f270071313cdfadc1346eef53bdb02ea25",
    ),
    (
        "src/exec/non_model.rs",
        "8d9451063c51126b7757b6b905e70272054db47585e8d3b8dae2e76f280d7748",
    ),
    (
        "src/supply_chain_guard/osv_stream.rs",
        "591ee5d75abe37066b6fe941eab791ab2bc13c785ea0fcc7566fce56949b1959",
    ),
];

/// Programs whose whole job is to talk to something off-box.
///
/// Not a census of every binary that can open a socket -- `git` and `gh` can
/// too, and they have their own seams. These are the general-purpose
/// transports, which is the class `exec::net` exists for.
const NETWORK_TOOLS: &[&str] = &["curl", "wget", "httpie", "http", "nc"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn raw_sha256(path: &Path) -> String {
    hex::encode(Sha256::digest(
        fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
    ))
}

fn assert_rustc_accepts(source: &str, external_tokio: bool) {
    let fixture = tempfile::tempdir().expect("network syntax fixture");
    let source_path = fixture.path().join("lib.rs");
    fs::write(&source_path, source).expect("write network syntax fixture");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());

    let dependency = fixture.path().join("libtokio.rlib");
    if external_tokio {
        let stub = fixture.path().join("tokio.rs");
        fs::write(
            &stub,
            "pub mod net {\
                 pub struct TcpStream;\
                 impl TcpStream {\
                     pub async fn connect(_: &str) {}\
                     pub fn try_write(&self, _: &[u8]) {}\
                 }\
                 pub struct UdpSocket;\
                 impl UdpSocket {\
                     pub async fn send(&self, _: &[u8]) {}\
                     pub fn try_send(&self, _: &[u8]) {}\
                     pub async fn send_to<T>(&self, _: &[u8], _: T) {}\
                     pub fn try_send_to<T>(&self, _: &[u8], _: T) {}\
                 }\
             }\
             pub mod io {\
                 pub trait AsyncWriteExt {\
                     async fn write_all(&mut self, _: &[u8]);\
                 }\
                 impl AsyncWriteExt for crate::net::TcpStream {\
                     async fn write_all(&mut self, _: &[u8]) {}\
                 }\
             }",
        )
        .expect("write tokio syntax stub");
        let output = std::process::Command::new(&rustc)
            .args(["--edition=2024", "--crate-name=tokio", "--crate-type=rlib"])
            .arg(&stub)
            .arg("-o")
            .arg(&dependency)
            .output()
            .expect("compile tokio syntax stub");
        assert!(
            output.status.success(),
            "tokio syntax stub did not compile: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut command = std::process::Command::new(rustc);
    command
        .args(["--edition=2024", "--crate-type=lib", "--emit=metadata"])
        .arg(&source_path)
        .arg("-o")
        .arg(fixture.path().join("fixture.rmeta"));
    if external_tokio {
        command.arg("--extern").arg(format!(
            "tokio={}",
            dependency
                .to_str()
                .expect("UTF-8 network syntax fixture path")
        ));
    }
    let output = command.output().expect("compile network syntax fixture");
    assert!(
        output.status.success(),
        "network syntax fixture did not compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_cargo_accepts(source: &str, dependencies: &str) {
    let fixture = tempfile::tempdir().expect("network Cargo fixture");
    fs::create_dir(fixture.path().join("src")).expect("create network Cargo source directory");
    fs::write(
        fixture.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"network-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependencies}\n"
        ),
    )
    .expect("write network Cargo manifest");
    fs::write(fixture.path().join("src/lib.rs"), source)
        .expect("write network Cargo fixture source");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(cargo)
        .args(["check", "--offline", "--quiet", "--manifest-path"])
        .arg(fixture.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", fixture.path().join("target"))
        .output()
        .expect("check network Cargo fixture");
    assert!(
        output.status.success(),
        "network Cargo fixture did not compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_rustc_accepts_with_module(root: &str, module_name: &str, module: &str) {
    assert_rustc_accepts_with_modules(root, &[(module_name, module)]);
}

fn assert_rustc_accepts_with_modules(root: &str, modules: &[(&str, &str)]) {
    let fixture = tempfile::tempdir().expect("network module fixture");
    fs::write(fixture.path().join("lib.rs"), root).expect("write network module root");
    for (module_name, module) in modules {
        fs::write(fixture.path().join(module_name), module).expect("write network module source");
    }
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = std::process::Command::new(rustc)
        .args(["--edition=2024", "--crate-type=lib", "--emit=metadata"])
        .arg(fixture.path().join("lib.rs"))
        .arg("-o")
        .arg(fixture.path().join("fixture.rmeta"))
        .output()
        .expect("compile cross-file network fixture");
    assert!(
        output.status.success(),
        "cross-file network fixture did not compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read production source {}: {error}", dir.display()));
    for e in entries {
        let e = e.unwrap_or_else(|error| {
            panic!("read production entry under {}: {error}", dir.display())
        });
        let p = e.path();
        if p.is_dir() {
            // #218. A directory holding its own `.git` is a separate checkout,
            // and this census claims to be closed over THIS one. Anvil keeps
            // agent worktrees under `.claude/worktrees/` and a `devtree`
            // beside them; each contributed a full copy of every real site.
            //
            // Both call sites root at `src/` today, so this cannot fire in the
            // suite as it stands. It is here because the walker takes a `dir`
            // and the next caller need not: the rule belongs to the walk, not
            // to who happens to call it. The predicate and its proof live in
            // `anvil::source_scan`.
            if anvil::source_scan::is_separate_checkout(&p) {
                continue;
            }
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

fn is_network_namespace(path: &str) -> bool {
    [
        "std::net",
        "tokio::net",
        "socket2",
        "reqwest",
        "hyper",
        "hyper_util",
        "ureq",
        "isahc",
        "surf",
    ]
    .iter()
    .any(|namespace| path == *namespace || path.starts_with(&format!("{namespace}::")))
}

fn is_network_crate_root(name: &str) -> bool {
    [
        "std",
        "tokio",
        "socket2",
        "reqwest",
        "hyper",
        "hyper_util",
        "ureq",
        "isahc",
        "surf",
    ]
    .contains(&name)
}

fn is_outbound_instance_method(name: &str) -> bool {
    // Finite spelling policy, not receiver inference. Includes buffer drains
    // and shutdown/close because supported I/O traits can flush pending writes.
    matches!(
        name,
        "connect"
            | "connect_timeout"
            | "send"
            | "send_with_flags"
            | "send_vectored_with_flags"
            | "send_out_of_band"
            | "sendmsg"
            | "sendfile"
            | "poll_send"
            | "poll_send_to"
            | "try_send"
            | "send_to"
            | "send_to_with_flags"
            | "try_send_to"
            | "send_vectored"
            | "send_to_vectored"
            | "send_to_vectored_with_flags"
            | "try_send_vectored"
            | "try_send_to_vectored"
            | "write"
            | "write_all"
            | "write_fmt"
            | "write_all_vectored"
            | "write_all_buf"
            | "write_buf"
            | "write_vectored"
            | "try_write"
            | "try_write_vectored"
            | "poll_write"
            | "poll_write_vectored"
            | "flush"
            | "poll_flush"
            | "shutdown"
            | "poll_shutdown"
            | "close"
            | "poll_close"
            | "write_u8"
            | "write_i8"
            | "write_u16"
            | "write_i16"
            | "write_u32"
            | "write_i32"
            | "write_u64"
            | "write_i64"
            | "write_u128"
            | "write_i128"
            | "write_f32"
            | "write_f64"
            | "write_u16_le"
            | "write_i16_le"
            | "write_u32_le"
            | "write_i32_le"
            | "write_u64_le"
            | "write_i64_le"
            | "write_u128_le"
            | "write_i128_le"
            | "write_f32_le"
            | "write_f64_le"
    )
}

fn is_finite_seam_name(name: &str) -> bool {
    name.rsplit("::").next() == Some("post_osv_batch")
}

fn is_network_function_item_path(name: &str) -> bool {
    is_network_namespace(name)
        && name
            .rsplit("::")
            .next()
            .and_then(|segment| segment.chars().next())
            .is_some_and(|first| first == '_' || first.is_ascii_lowercase())
}

fn outbound_ufcs_operation(name: &str) -> Option<&str> {
    let name = name.trim_start_matches("::");
    let (receiver, method) = name.rsplit_once("::")?;
    if !is_outbound_instance_method(method) {
        return None;
    }
    matches!(
        receiver,
        "std::io::Write"
            | "std::io::prelude::Write"
            | "tokio::io::AsyncWrite"
            | "tokio::io::AsyncWriteExt"
            | "futures::io::AsyncWrite"
            | "futures::io::AsyncWriteExt"
            | "futures_io::AsyncWrite"
            | "futures_util::io::AsyncWrite"
            | "futures_util::io::AsyncWriteExt"
    )
    .then_some(method)
}

fn is_outbound_copy_helper(name: &str) -> bool {
    matches!(
        name.trim_start_matches("::"),
        "std::io::copy"
            | "tokio::io::copy"
            | "tokio::io::copy_buf"
            | "tokio::io::copy_bidirectional"
            | "tokio::io::copy_bidirectional_with_sizes"
            | "futures::io::copy"
            | "futures::io::copy_buf"
            | "futures_util::io::copy"
            | "futures_util::io::copy_buf"
    )
}

fn cargo_network_bindings_from_metadata(
    metadata: &serde_json::Value,
    repository: &Path,
) -> BTreeMap<String, String> {
    let mut bindings = BTreeMap::new();
    for dependency in metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|package| {
            package["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path).starts_with(repository))
        })
        .flat_map(|package| package["dependencies"].as_array().into_iter().flatten())
        .filter(|dependency| dependency["kind"].as_str() != Some("dev"))
    {
        let package = dependency["name"]
            .as_str()
            .expect("Cargo dependency package name")
            .replace('-', "_");
        if !is_network_crate_root(&package) {
            continue;
        }
        let binding = dependency["rename"]
            .as_str()
            .unwrap_or_else(|| {
                dependency["name"]
                    .as_str()
                    .expect("Cargo dependency package name")
            })
            .replace('-', "_");
        if let Some(previous) = bindings.insert(binding.clone(), package.clone()) {
            assert_eq!(
                previous, package,
                "Cargo alias {binding} can name multiple network packages; the finite census cannot choose one"
            );
        }
    }
    bindings
}

fn production_cargo_network_bindings() -> BTreeMap<String, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = std::process::Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--locked",
            "--offline",
            "--manifest-path",
        ])
        .arg(repo().join("Cargo.toml"))
        .current_dir(repo())
        .output()
        .expect("run Cargo metadata for network dependency bindings");
    assert!(
        output.status.success(),
        "Cargo metadata for network dependency bindings failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse network Cargo metadata");
    cargo_network_bindings_from_metadata(&metadata, &repo())
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

fn is_declared_test_file(path: &Path, declared: &BTreeSet<PathBuf>) -> bool {
    let canonical = fs::canonicalize(path)
        .unwrap_or_else(|error| panic!("canonical source identity {}: {error}", path.display()));
    declared.contains(&canonical)
}

#[test]
fn declared_test_membership_uses_existing_canonical_file_identity() {
    let root = tempfile::tempdir().expect("identity fixture");
    let path = root.path().join("source.rs");
    fs::write(&path, "").expect("ordinary source file");
    let canonical = fs::canonicalize(&path).expect("canonical fixture identity");
    let declared = [canonical.clone()].into_iter().collect();
    assert!(is_declared_test_file(&path, &declared));
    assert!(is_declared_test_file(&canonical, &declared));
    assert!(!is_declared_test_file(&path, &BTreeSet::new()));
}

#[test]
#[should_panic(expected = "canonical source identity")]
fn declared_test_membership_does_not_excuse_a_missing_file() {
    let root = tempfile::tempdir().expect("identity fixture");
    is_declared_test_file(&root.path().join("missing.rs"), &BTreeSet::new());
}

/// Network-tool spawns outside the seam, as `path: argument`.
fn offenders() -> Vec<String> {
    let test_modules = anvil::source_scan::paths::declared_test_module_files(&repo())
        .expect("classify declared test modules");
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
        if is_test_source(&rel) || is_declared_test_file(&p, &test_modules) || is_seam(&rel) {
            continue;
        }
        let raw = fs::read_to_string(&p)
            .unwrap_or_else(|error| panic!("read production source {}: {error}", p.display()));
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
        seam.contains("Command::new(CURL)"),
        "`exec::net` no longer constructs the command, so there is nothing for \
         the callers below to be routed through"
    );
    assert!(
        seam.contains("post_osv_batch(")
            && seam.contains("packages: &[crate::supply_chain_guard::LockedPackage]")
            && seam.contains("OSV_BATCH_URL")
            && seam.contains("CURL_MAX_TIME"),
        "the network seam is no longer the finite OSV querybatch capability"
    );
    assert!(
        seam.contains("super::clear_environment(cmd)"),
        "`exec::net` no longer delegates environment clearing to the checked \
         non-model rebinder"
    );

    let non_model = without_commentary(&module_source(NON_MODEL_SEAM, &repo()));
    assert!(
        non_model.contains("fn clear_environment(command: &mut Command)")
            && non_model.contains("command.env_clear()")
            && non_model.contains("command.env(CLEARED_ENV_MARKER, \"1\")")
            && non_model.contains("bound.env_clear()"),
        "the shared environment-clear capability no longer clears both the \
         caller's command and the canonical executable rebound for launch"
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
        src.contains("crate::exec::post_osv_batch(packages)"),
        "the OSV request no longer goes through the finite exec transport, so it carries the \
         webhook secret and every provider key to a public advisory database"
    );
    assert!(
        !src.contains("Command::new("),
        "`{TRANSPORT}` builds a subprocess of its own. A bare `Command::new` \
         inherits the daemon's whole environment; build it with \
         the finite OSV transport."
    );
}

/// A public program/URL/payload POST was a second direct-model transport: an
/// Anvil caller could send contributor text to a provider API without either
/// `AgentCommand` or `ModelPrompt`. The production surface now fixes every
/// transport choice except the body sent to OSV.
#[test]
fn no_generic_network_transport_can_be_called_by_anvil_or_library_users() {
    // Inspect the physical `exec/mod.rs` root here. Aggregating `src/exec`
    // intentionally includes its private `non_model::net` child, while this
    // assertion is specifically about `net` not being an `exec` sibling.
    let exec = without_commentary(&module_source("src/exec/mod", &repo()));
    let net = without_commentary(&module_source(SEAM, &repo()));
    let osv = without_commentary(&module_source("src/supply_chain_guard/osv_stream", &repo()));

    assert!(!exec.contains("mod net;"));
    assert!(!exec.contains("pub mod net;"));
    let non_model = without_commentary(&module_source(NON_MODEL_SEAM, &repo()));
    assert!(non_model.contains("mod net;"));
    assert!(!non_model.contains("run_osv"));
    assert!(!non_model.contains("pub(super) fn checked_for"));
    assert!(!exec.contains("pub use net::command"));
    assert!(!exec.contains("pub fn post_osv_batch"));
    assert!(exec.contains("pub(crate) async fn post_osv_batch("));
    assert!(exec.contains("packages: &[crate::supply_chain_guard::LockedPackage]"));
    assert!(!osv.contains("pub async fn post_json"));
    assert!(!osv.contains("url: &str"));
    assert!(!osv.contains("program: &str"));
    assert!(net.contains("crate::supply_chain_guard::osv_stream::OSV_BATCH_URL"));
    assert!(!net.contains("url: &str"));
    assert!(!net.contains("pub fn apply"));
    assert!(!net.contains("pub(super) fn command"));
    assert!(!net.contains("OsvBatchCommand"));
    assert!(net.contains("let command = super::NonModelCommand::checked_for"));

    let census = network_capability_census();
    let conservative_method_events = census
        .iter()
        .filter(|(_, _, event)| {
            event.starts_with("network-instance-method:")
                || event.starts_with("network-macro-outbound-method:")
                || event.starts_with("network-macro-method-definition:")
                || event.starts_with("network-ufcs-outbound:")
                || event.starts_with("network-copy-outbound:")
        })
        .cloned()
        .collect::<Vec<_>>();
    // Pin the overinclusive scanned corpus, not a claim that every file ships.
    // Five newly covered occurrences are two child stdin shutdowns, a temporary
    // review-body file flush, DelimSpan::close, and a `close` variable in format!.
    // Declaration classification excludes the model_prompt external test
    // occurrence. The stale git_manager/evidence_objects/tests.rs expectation
    // names an absent file, not a newly classified test. These eight production
    // occurrences remain exact.
    let added_method_events = [
        // Reviewed for #215. `RunScope::declare` writes the stage's declared
        // write scope to `.anvil/run-scope` for the length of one turn. Same
        // shape as `PromptFile` below: the receiver is a `std::fs::File` /
        // `OpenOptions`, no socket is constructed, imported or named in that
        // module, and it is a module of its own for exactly that reason.
        //
        // The `opts.write(true)` occurrence is under `create`, not `declare`:
        // the scanner attributes by ENCLOSING FUNCTION, and the open moved into
        // an extracted helper when the declaration gained a stale-takeover
        // retry. Re-attributed, not removed -- the total stays 44 and the
        // occurrence stays REVIEWED. Deleting the line instead would have
        // demoted it into the 34 pinned only by digest, which is how a
        // reviewed record quietly becomes an unreviewed one.
        (
            "src/ai_driver/chain/run_scope.rs",
            "create",
            "network-instance-method:write",
        ),
        (
            "src/ai_driver/chain/run_scope.rs",
            "declare",
            "network-instance-method:flush",
        ),
        (
            "src/clean_architecture_guard/scan.rs",
            "expand_use_groups",
            "network-macro-outbound-method:close:4e733b3c4cbaab79203a65a3be872e700366f3a8da84b24ca93aa63449a89e67",
        ),
        // Reviewed for #216. All three are `PromptFile::write`, and the receiver
        // is a `std::fs::File` / `OpenOptions` in every case: `opts.write(true)`
        // selects the open mode, `write_all` puts the rendered prompt in the
        // file, `flush` closes the write out before the path is handed to the
        // provider. No socket is constructed, imported or named anywhere in that
        // module -- which is why it is a module of its own and not part of the
        // transport.
        (
            "src/exec/agent/prompt_file.rs",
            "write",
            "network-instance-method:flush",
        ),
        (
            "src/exec/agent/prompt_file.rs",
            "write",
            "network-instance-method:write",
        ),
        (
            "src/exec/agent/prompt_file.rs",
            "write",
            "network-instance-method:write_all",
        ),
        (
            "src/exec/agent/transport.rs",
            "deliver_with_stdin",
            "network-instance-method:shutdown",
        ),
        (
            "src/exec/non_model/transport.rs",
            "run_with_stdin",
            "network-instance-method:shutdown",
        ),
        (
            "src/github/reviews.rs",
            "submit_pr_review_with_diff",
            "network-instance-method:flush",
        ),
        (
            "src/source_scan/test_modules.rs",
            "module_range",
            "network-instance-method:close",
        ),
    ]
    .map(|(path, owner, event)| (path.to_owned(), owner.to_owned(), event.to_owned()));
    assert_eq!(
        conservative_method_events
            .iter()
            .filter(|event| added_method_events.contains(event))
            .cloned()
            .collect::<Vec<_>>(),
        added_method_events,
        "newly reviewed scanned-corpus occurrences must remain exact"
    );
    let prior_method_events = conservative_method_events
        .iter()
        .filter(|event| !added_method_events.contains(event))
        .cloned()
        .collect::<Vec<_>>();
    let conservative_method_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&prior_method_events).expect("serialize method census"),
    ));
    // Receiver types are deliberately not inferred: `send`/`write_all` on a
    // socket and on a channel/file have the same syntax. The false-positive
    // spellings already present in the scanned corpus are therefore pinned as a
    // separate exact set instead of weakening the outbound-method policy.
    assert_eq!(
        conservative_method_events.len(),
        44,
        "{conservative_method_events:#?}"
    );
    // This complete current set was reviewed by owner and source expression:
    // 44 occurrences = the ten explicit production records above + 34 below.
    // The 34 and their digest are unchanged: newly reviewed occurrences go in
    // the explicit list, so adding one cannot perturb the historical set.
    // The former historical 38-entry digest could not be reproduced, so this
    // is a current-set binding, not a claimed historical subtraction. Duplicate
    // lock/file occurrences remain significant; Windows pair connect/write
    // describe the already-reviewed local pipe and its writer open option.
    assert_eq!(prior_method_events.len(), 34);
    assert_eq!(
        conservative_method_digest,
        "48595cd700ecfd02768022877572a4f91d0ee7973b9e5086117a11c121581844",
        "the conservative outbound-method spelling census changed: {conservative_method_events:#?}"
    );
    // Complete current structural set: 18 existing records plus eight reviewed
    // Windows local-IPC type/import/expression records, retaining duplicates.
    // Four stale evaluate_corpus expectations name no current source operation.
    let structural_census = census
        .into_iter()
        .filter(|event| !conservative_method_events.contains(event))
        .collect::<Vec<_>>();
    assert_eq!(
        structural_census,
        [
            (
                "src/cli/server.rs".to_owned(),
                "".to_owned(),
                "network-import:std::net::SocketAddr->SocketAddr".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-expression:socket2::Domain::IPV4".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-expression:socket2::Domain::IPV6".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-expression:socket2::Protocol::TCP".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-expression:socket2::Socket::new".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-expression:socket2::Type::STREAM".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-expression:tokio::net::TcpListener::from_std".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-type:std::net::SocketAddr".to_owned(),
            ),
            (
                "src/cli/server.rs".to_owned(),
                "run_server".to_owned(),
                "network-type:std::net::TcpListener".to_owned(),
            ),
            (
                "src/exec/mod.rs".to_owned(),
                "post_osv_batch".to_owned(),
                "finite-authority-reference:non_model::post_osv_batch".to_owned(),
            ),
            (
                "src/exec/mod.rs".to_owned(),
                "post_osv_batch".to_owned(),
                "finite-call:non_model::post_osv_batch".to_owned(),
            ),
            (
                "src/exec/non_model.rs".to_owned(),
                "post_osv_batch".to_owned(),
                "finite-authority-reference:net::post_osv_batch".to_owned(),
            ),
            (
                "src/exec/non_model.rs".to_owned(),
                "post_osv_batch".to_owned(),
                "finite-call:net::post_osv_batch".to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture.rs".to_owned(),
                "".to_owned(),
                "network-type:tokio::net::windows::named_pipe::NamedPipeServer".to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture.rs".to_owned(),
                "".to_owned(),
                "network-type:tokio::net::windows::named_pipe::NamedPipeServer".to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture/windows.rs".to_owned(),
                "".to_owned(),
                "network-import:tokio::net::windows::named_pipe::NamedPipeServer->NamedPipeServer"
                    .to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture/windows.rs".to_owned(),
                "".to_owned(),
                "network-import:tokio::net::windows::named_pipe::PipeMode->PipeMode".to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture/windows.rs".to_owned(),
                "".to_owned(),
                "network-import:tokio::net::windows::named_pipe::ServerOptions->ServerOptions"
                    .to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture/windows.rs".to_owned(),
                "pair".to_owned(),
                "network-expression:tokio::net::windows::named_pipe::PipeMode::Byte".to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture/windows.rs".to_owned(),
                "pair".to_owned(),
                "network-expression:tokio::net::windows::named_pipe::ServerOptions::new".to_owned(),
            ),
            (
                "src/exec/non_model/transport/sync_capture/windows.rs".to_owned(),
                "pair".to_owned(),
                "network-type:tokio::net::windows::named_pipe::NamedPipeServer".to_owned(),
            ),
            (
                "src/recovery/blue_green_supervisor.rs".to_owned(),
                "spawn_green_and_drain_blue".to_owned(),
                "network-expression:tokio::net::TcpStream::connect".to_owned(),
            ),
            (
                "src/supply_chain_guard/osv_stream.rs".to_owned(),
                "post_batch".to_owned(),
                "finite-authority-reference:crate::exec::post_osv_batch".to_owned(),
            ),
            (
                "src/supply_chain_guard/osv_stream.rs".to_owned(),
                "post_batch".to_owned(),
                "finite-call:crate::exec::post_osv_batch".to_owned(),
            ),
            (
                "src/webhook/admin_auth.rs".to_owned(),
                "".to_owned(),
                "network-import:std::net::IpAddr->IpAddr".to_owned(),
            ),
            (
                "src/webhook/admin_auth.rs".to_owned(),
                "is_loopback".to_owned(),
                "network-type:std::net::IpAddr".to_owned(),
            ),
        ],
        "the finite OSV call graph or another direct network construction changed"
    );
}

// Kept separate so source-only verification never invokes compiler fixtures.
#[test]
fn network_capability_compiler_fixtures_remain_distinct_from_repository_census() {
    let bypass = network_capability_events(
        r#"
            use std::net as wire;
            struct Config { endpoint: String }
            pub fn relay(config: Config) -> impl Fn(&[u8]) {
                move |_data| { let _ = wire::TcpStream::connect(&config.endpoint); }
            }
        "#,
    );
    assert!(
        bypass
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect")
    );
    let function_value = network_capability_events(
        r#"
            fn relay(endpoint: &str) {
                let dial = std::net::TcpStream::connect;
                let _ = dial(endpoint);
            }
        "#,
    );
    assert!(
        function_value
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect"),
        "a network constructor copied into a function value must remain visible: {function_value:?}"
    );

    let scoped = network_capability_events(
        r#"
            mod harmless { pub struct Wire; impl Wire { pub fn connect(_: &str) {} } }
            mod outer {
                use std::net::TcpStream as Wire;
                mod inner {
                    use crate::harmless::Wire;
                    fn innocent() { Wire::connect("local"); }
                }
                fn relay() { let _ = Wire::connect("example.invalid:443"); }
            }
        "#,
    );
    assert!(
        scoped
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect"),
        "a nested harmless alias must not erase its parent's network alias: {scoped:?}"
    );

    let block_scoped = r#"
        struct Local;
        impl Local { fn connect(_: &str) {} }
        fn relay() {
            use std::net::TcpStream as Wire;
            {
                use crate::Local as Wire;
                Wire::connect("local");
            }
            let _ = Wire::connect("example.invalid:443");
        }
    "#;
    assert_rustc_accepts(block_scoped, false);
    let block_scoped_events = network_capability_events(block_scoped);
    assert!(
        block_scoped_events
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect"),
        "a nested block alias must not erase its parent's network binding: {block_scoped_events:?}"
    );

    let std_extern = r#"
        mod client {
            fn relay() {
                let _ = hidden::net::TcpStream::connect("example.invalid:443");
            }
            extern crate std as hidden;
        }
    "#;
    assert_rustc_accepts(std_extern, false);
    let std_extern_events = network_capability_events(std_extern);
    assert!(
        std_extern_events
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect"),
        "an extern-crate alias must retain its network provenance: {std_extern_events:?}"
    );

    let raw_std = r#"
        mod client {
            fn aliased() {
                let _ = hidden::net::TcpStream::connect("example.invalid:443");
            }
            fn direct() {
                let _ = r#std::net::TcpStream::connect("example.invalid:443");
            }
            extern crate r#std as hidden;
        }
    "#;
    assert_rustc_accepts(raw_std, false);
    let raw_std_events = network_capability_events(raw_std);
    assert!(
        raw_std_events
            .iter()
            .filter(|event| *event == "network-expression:std::net::TcpStream::connect")
            .count()
            >= 2,
        "raw identifiers must retain std network provenance in direct and aliased paths: {raw_std_events:?}"
    );

    let block_extern = r#"
        fn relay() {
            let _ = hidden::net::TcpStream::connect("example.invalid:443");
            extern crate std as hidden;
        }
    "#;
    assert_rustc_accepts(block_extern, false);
    let block_extern_events = network_capability_events(block_extern);
    assert!(
        block_extern_events
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect"),
        "a block item alias declared after use must retain network provenance: {block_extern_events:?}"
    );

    let qualified_extern = r#"
        extern crate std as hidden;
        fn root() {
            let _ = crate::hidden::net::TcpStream::connect("example.invalid:443");
            let _ = self::hidden::net::TcpStream::connect("example.invalid:443");
        }
        mod nested {
            use crate::hidden as other;
            fn relay() {
                let _ = super::hidden::net::TcpStream::connect("example.invalid:443");
                let _ = other::net::TcpStream::connect("example.invalid:443");
            }
        }
    "#;
    assert_rustc_accepts(qualified_extern, false);
    let qualified_extern_events = network_capability_events(qualified_extern);
    assert!(
        qualified_extern_events
            .iter()
            .any(|event| event == "network-extern-crate:std->hidden"),
        "a network extern-crate introduction must remain visible even through root qualifiers: {qualified_extern_events:?}"
    );
    assert!(
        qualified_extern_events
            .iter()
            .any(|event| event == "network-alias-import:crate::hidden->other"),
        "a qualified import of a known network alias must propagate authority: {qualified_extern_events:?}"
    );
    assert!(
        qualified_extern_events
            .iter()
            .any(|event| event == "network-alias-expression:other::net::TcpStream::connect"),
        "a renamed qualified alias reference must remain visible: {qualified_extern_events:?}"
    );

    let qualified_glob = r#"
        extern crate std as hidden;
        mod nested {
            use crate::hidden::*;
            fn relay() {
                let _ = net::TcpStream::connect("example.invalid:443");
            }
        }
    "#;
    assert_rustc_accepts(qualified_glob, false);
    let qualified_glob_events = network_capability_events(qualified_glob);
    assert!(
        qualified_glob_events
            .iter()
            .any(|event| event == "network-alias-glob-import:crate::hidden"),
        "a glob through a known network alias must propagate authority: {qualified_glob_events:?}"
    );
    assert!(
        qualified_glob_events
            .iter()
            .any(|event| event == "network-glob-expression:net::TcpStream::connect"),
        "a path introduced by a qualified network glob must remain visible: {qualified_glob_events:?}"
    );

    let qualified_use = r#"
        use std as hidden;
        fn root() {
            let _ = crate::hidden::net::TcpStream::connect("example.invalid:443");
        }
    "#;
    assert_rustc_accepts(qualified_use, false);
    let qualified_use_events = network_capability_events(qualified_use);
    assert!(
        qualified_use_events
            .iter()
            .any(|event| event == "network-root-import:std->hidden"),
        "a std root import must remain visible even through a crate-qualified alias: {qualified_use_events:?}"
    );

    let qualified_tokio_use = r#"
        use tokio as hidden;
        async fn root() {
            hidden::net::TcpStream::connect("example.invalid:443").await;
        }
    "#;
    assert_rustc_accepts(qualified_tokio_use, true);
    let qualified_tokio_use_events = network_capability_events(qualified_tokio_use);
    assert!(
        qualified_tokio_use_events
            .iter()
            .any(|event| event == "network-root-import:tokio->hidden"),
        "a Tokio root import must remain visible independent of path spelling: {qualified_tokio_use_events:?}"
    );

    let self_imports = r#"
        use std::{self as hidden};
        use std::{net::{self as hidden_net}};
        fn relay() {
            let _ = hidden::net::TcpStream::connect("example.invalid:443");
            let _ = hidden_net::TcpStream::connect("example.invalid:443");
        }
    "#;
    assert_rustc_accepts(self_imports, false);
    let self_import_events = network_capability_events(self_imports);
    assert!(
        self_import_events
            .iter()
            .any(|event| event == "network-root-import:std->hidden"),
        "`self as` must bind the enclosing std path: {self_import_events:?}"
    );
    assert!(
        self_import_events
            .iter()
            .any(|event| event == "network-import:std::net->hidden_net"),
        "nested `self as` must bind the enclosing std::net path: {self_import_events:?}"
    );

    let root_glob = r#"
        use std::*;
        fn relay() {
            let _ = net::TcpStream::connect("example.invalid:443");
        }
    "#;
    assert_rustc_accepts(root_glob, false);
    let root_glob_events = network_capability_events(root_glob);
    assert!(
        root_glob_events
            .iter()
            .any(|event| event == "network-root-glob-import:std"),
        "a network crate-root glob must withhold the finite inventory: {root_glob_events:?}"
    );

    let macro_import = r#"
        macro_rules! import_hidden {
            () => { use std as hidden; }
        }
        import_hidden!();
        fn relay() {
            let _ = hidden::net::TcpStream::connect("example.invalid:443");
        }
    "#;
    assert_rustc_accepts(macro_import, false);
    let macro_import_events = network_capability_events(macro_import);
    assert!(
        macro_import_events
            .iter()
            .any(|event| event.starts_with("network-macro-sensitive:")),
        "a macro body with network crate-root authority must be fingerprinted: {macro_import_events:?}"
    );
    assert!(
        macro_import_events
            .iter()
            .any(|event| event == "network-alias-expression:hidden::net::TcpStream::connect"),
        "a path bound only by a local macro expansion must remain visible: {macro_import_events:?}"
    );

    let grouped_macro_import = r#"
        macro_rules! import_hidden {
            () => { use std::{fmt as harmless, net as hidden_net}; }
        }
        import_hidden!();
        fn relay() {
            let _ = hidden_net::TcpStream::connect("example.invalid:443");
        }
    "#;
    assert_rustc_accepts(grouped_macro_import, false);
    let grouped_macro_import_events = network_capability_events(grouped_macro_import);
    assert!(
        grouped_macro_import_events.iter().any(|event| {
            event == "network-alias-expression:hidden_net::TcpStream::connect"
                || event == "network-macro-context-expression:hidden_net::TcpStream::connect"
        }),
        "every branch of a macro-generated grouped import must preserve network authority: {grouped_macro_import_events:?}"
    );

    let tokio_extern = r#"
        async fn relay() {
            hidden::net::TcpStream::connect("example.invalid:443").await;
        }
        extern crate tokio as hidden;
    "#;
    assert_rustc_accepts(tokio_extern, true);
    let tokio_extern_events = network_capability_events(tokio_extern);
    assert!(
        tokio_extern_events
            .iter()
            .any(|event| event == "network-expression:tokio::net::TcpStream::connect"),
        "a renamed external Tokio crate must retain its network provenance: {tokio_extern_events:?}"
    );

    let macro_generated = network_capability_events(
        r#"
            macro_rules! dial {
                () => {{ std::net::TcpStream::connect("example.invalid:443") }};
            }
            fn relay() { let _ = dial!(); }
        "#,
    );
    assert!(
        macro_generated
            .iter()
            .any(|event| event == "network-macro-token:std::net::TcpStream::connect"),
        "a local macro body that emits a network call must remain visible: {macro_generated:?}"
    );

    let token_assembled = network_capability_events(
        r#"
            macro_rules! dial {
                ($a:ident, $b:ident) => {{ $a::$b::TcpStream::connect("example.invalid:443") }};
            }
            fn relay() { let _ = dial!(std, net); }
        "#,
    );
    assert!(
        token_assembled
            .iter()
            .any(|event| event.starts_with("network-macro-ambiguous:dynamic-path:")),
        "a local macro that assembles a path from metavariables must withhold the finite network inventory: {token_assembled:?}"
    );

    let repeated_segments = r#"
        macro_rules! dial {
            ($($segment:ident),+) => {
                $($segment)::+::TcpStream::connect("example.invalid:443")
            };
        }
        fn relay() { let _ = dial!(std, net); }
    "#;
    assert_rustc_accepts(repeated_segments, false);
    let repeated_events = network_capability_events(repeated_segments);
    assert!(
        repeated_events
            .iter()
            .any(|event| event.starts_with("network-macro-ambiguous:dynamic-path:")),
        "a macro repetition that assembles a network path must withhold the finite inventory: {repeated_events:?}"
    );

    let sensitive_definition = r#"
        macro_rules! dial {
            () => { std::net::TcpStream::connect("example.invalid:443") };
        }
    "#;
    let sensitive_invocation = r#"
        macro_rules! dial {
            () => { std::net::TcpStream::connect("example.invalid:443") };
        }
        fn attack() { let _ = dial!(); }
    "#;
    assert_rustc_accepts(sensitive_definition, false);
    assert_rustc_accepts(sensitive_invocation, false);
    let definition_events = network_capability_events(sensitive_definition);
    let invocation_events = network_capability_events(sensitive_invocation);
    assert!(
        !definition_events
            .iter()
            .any(|event| event.starts_with("network-local-macro-invocation:")),
        "an uninvoked definition is not a connection site: {definition_events:?}"
    );
    assert!(
        invocation_events
            .iter()
            .any(|event| event.starts_with("network-local-macro-invocation:sensitive:dial:")),
        "invoking a network-sensitive local macro must add a site: {invocation_events:?}"
    );

    let state_aware_definition = r#"
        use std as s;
        macro_rules! dial {
            ($a:ident, $b:ident) => { $a::$b::TcpStream::connect("example.invalid:443") };
        }
    "#;
    let state_aware_invocation = r#"
        use std as s;
        macro_rules! dial {
            ($a:ident, $b:ident) => { $a::$b::TcpStream::connect("example.invalid:443") };
        }
        fn attack() { let _ = dial!(s, net); }
    "#;
    assert_rustc_accepts(state_aware_definition, false);
    assert_rustc_accepts(state_aware_invocation, false);
    let state_definition_events = network_capability_events(state_aware_definition);
    let state_invocation_events = network_capability_events(state_aware_invocation);
    assert!(
        !state_definition_events
            .iter()
            .any(|event| event.starts_with("network-local-macro-invocation:")),
        "an uninvoked dynamic definition is not a connection site: {state_definition_events:?}"
    );
    assert!(
        state_definition_events
            .iter()
            .any(|event| event.starts_with("network-macro-ambiguous-definition:dial:")),
        "the definition must retain its unresolved metavariable authority: {state_definition_events:?}"
    );
    assert!(
        state_invocation_events
            .iter()
            .any(|event| event.starts_with("network-local-macro-invocation:sensitive:dial:")),
        "an invocation with a known outbound method and network alias must add a sensitive site: {state_invocation_events:?}"
    );

    let renamed_before_definition = r#"
        use m::dial as d;
        fn attack() { let _ = d!(); }
        mod m {
            macro_rules! dial {
                () => { std::net::TcpStream::connect("example.invalid:443") };
            }
            pub(crate) use dial;
        }
    "#;
    assert_rustc_accepts(renamed_before_definition, false);
    let renamed_events = network_capability_events(renamed_before_definition);
    assert!(
        renamed_events
            .iter()
            .any(|event| event.starts_with("network-local-macro-invocation:sensitive:d:")),
        "a renamed sensitive macro invoked before its defining module must add a site: {renamed_events:?}"
    );

    let cross_file_root = r#"
        mod m;
        use m::dial as d;
        fn attack() { let _ = d!(); }
    "#;
    let cross_file_module = r#"
        macro_rules! dial {
            () => { std::net::TcpStream::connect("example.invalid:443") };
        }
        pub(crate) use dial;
    "#;
    assert_rustc_accepts_with_module(cross_file_root, "m.rs", cross_file_module);
    let cross_file_events =
        network_capability_events_with_sources(cross_file_root, &[cross_file_module]);
    assert!(
        cross_file_events
            .iter()
            .any(|event| event.starts_with("network-local-macro-invocation:sensitive:d:")),
        "a sensitive macro definition in another module file must bind a renamed invocation: {cross_file_events:?}"
    );

    let cross_file_alias_root = r#"
        mod a;
        use crate::a::hidden as x;
        fn attack() { let _ = x::net::TcpStream::connect("example.invalid:443"); }
    "#;
    let cross_file_alias_module = "pub use std as hidden;";
    assert_rustc_accepts_with_module(cross_file_alias_root, "a.rs", cross_file_alias_module);
    let cross_file_alias_events =
        network_capability_events_with_sources(cross_file_alias_root, &[cross_file_alias_module]);
    assert!(
        cross_file_alias_events
            .iter()
            .any(|event| event == "network-alias-expression:x::net::TcpStream::connect"),
        "a network reexport from another module file must seed the consuming file: {cross_file_alias_events:?}"
    );

    let glob_renamed_macro = r#"
        mod m {
            macro_rules! dial {
                () => { std::net::TcpStream::connect("example.invalid:443") };
            }
            pub(crate) use dial as hidden_dial;
        }
        use m::*;
        fn attack() { let _ = hidden_dial!(); }
    "#;
    assert_rustc_accepts(glob_renamed_macro, false);
    let glob_renamed_events = network_capability_events(glob_renamed_macro);
    assert!(
        glob_renamed_events.iter().any(|event| {
            event.starts_with("network-local-macro-invocation:sensitive:hidden_dial:")
        }),
        "a renamed sensitive macro reached through a glob must add a site: {glob_renamed_events:?}"
    );

    let one_call = network_capability_events(
        r#"fn relay() { let _ = std::net::TcpStream::connect("one:1"); }"#,
    );
    let two_calls = network_capability_events(
        r#"fn relay() {
            let _ = std::net::TcpStream::connect("one:1");
            let _ = std::net::TcpStream::connect("two:2");
        }"#,
    );
    let connection_count = |events: &[String]| {
        events
            .iter()
            .filter(|event| *event == "network-expression:std::net::TcpStream::connect")
            .count()
    };
    assert_eq!(connection_count(&one_call), 1);
    assert_eq!(
        connection_count(&two_calls),
        2,
        "duplicate connection sites in one owner must change the inventory: {two_calls:?}"
    );
}

#[test]
fn production_does_not_store_finite_or_network_function_items() {
    let escapes = network_capability_census()
        .into_iter()
        .filter(|(_, _, event)| event.contains("function-item-escape:"))
        .collect::<Vec<_>>();
    assert!(
        escapes.is_empty(),
        "finite/network function items may only be direct callees; stored authority would require an open-ended value-flow proof: {escapes:?}"
    );
}

#[test]
fn finite_network_boundary_matches_the_reviewed_bytes() {
    for (path, expected) in REVIEWED_NETWORK_BOUNDARY {
        assert_eq!(
            raw_sha256(&repo().join(path)),
            *expected,
            "{path} changed; re-review the complete finite OSV request chain"
        );
    }
}

#[test]
fn absolute_extern_prelude_paths_ignore_local_aliases() {
    let socket2 = r#"
        mod outer {
            mod safe {}
            use self::safe as socket2;
            fn direct() {
                let _ = ::socket2::Socket::new(
                    ::socket2::Domain::IPV4,
                    ::socket2::Type::STREAM,
                    Some(::socket2::Protocol::TCP),
                );
            }
            use ::socket2 as wire;
            fn imported() {
                let _ = wire::Socket::new(
                    wire::Domain::IPV4,
                    wire::Type::STREAM,
                    Some(wire::Protocol::TCP),
                );
            }
        }
    "#;
    assert_cargo_accepts(
        socket2,
        "socket2 = { version = \"=0.5.10\", features = [\"all\"] }",
    );
    let socket2_events = network_capability_events(socket2);
    assert!(
        socket2_events
            .iter()
            .any(|event| event == "network-expression:socket2::Socket::new"),
        "an absolute extern-prelude path must not resolve through a local alias: {socket2_events:?}"
    );
    assert!(
        socket2_events
            .iter()
            .any(|event| event == "network-import:socket2->wire"),
        "an absolute use must not resolve through a same-spelled local alias: {socket2_events:?}"
    );

    let std = r#"
        mod outer {
            mod safe {}
            use self::safe as std;
            fn relay() {
                let _ = ::std::net::TcpStream::connect("example.invalid:443");
            }
        }
    "#;
    assert_rustc_accepts(std, false);
    assert!(
        network_capability_events(std)
            .iter()
            .any(|event| event == "network-expression:std::net::TcpStream::connect")
    );

    let tokio = r#"
        mod outer {
            mod safe {}
            use self::safe as tokio;
            async fn relay() {
                ::tokio::net::TcpStream::connect("example.invalid:443").await;
            }
        }
    "#;
    assert_rustc_accepts(tokio, true);
    assert!(
        network_capability_events(tokio)
            .iter()
            .any(|event| event == "network-expression:tokio::net::TcpStream::connect")
    );
}

#[test]
fn cargo_renamed_network_dependency_seeds_the_effective_extern_binding() {
    let source = r#"
        macro_rules! open {
            () => {
                socket::Socket::new(
                    socket::Domain::IPV4,
                    socket::Type::STREAM,
                    Some(socket::Protocol::TCP),
                )
            };
        }
        fn relay() {
            let _ = socket::Socket::new(
                socket::Domain::IPV4,
                socket::Type::STREAM,
                Some(socket::Protocol::TCP),
            );
            let _ = ::socket::Socket::new(
                ::socket::Domain::IPV4,
                ::socket::Type::STREAM,
                Some(::socket::Protocol::TCP),
            );
            let _ = open!();
        }
    "#;
    assert_cargo_accepts(
        source,
        "socket = { package = \"socket2\", version = \"=0.5.10\", features = [\"all\"] }",
    );
    let repository = Path::new("/fixture");
    let metadata = serde_json::json!({
        "packages": [{
            "manifest_path": "/fixture/Cargo.toml",
            "dependencies": [{
                "name": "socket2",
                "rename": "socket",
                "kind": null,
                "target": null
            }]
        }]
    });
    let bindings = cargo_network_bindings_from_metadata(&metadata, repository);
    assert_eq!(bindings.get("socket").map(String::as_str), Some("socket2"));
    let events = network_capability_events_with_cargo_bindings(source, bindings);
    assert_eq!(
        events
            .iter()
            .filter(|event| *event == "network-expression:socket2::Socket::new")
            .count(),
        2,
        "Cargo's effective extern alias must carry socket2 network provenance: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| { event.starts_with("network-local-macro-invocation:sensitive:open:") }),
        "Cargo's effective extern alias must carry network provenance through macro bodies: {events:?}"
    );
}

#[test]
fn outbound_instance_methods_add_one_event_per_call() {
    let std_baseline = network_capability_events(
        r#"fn relay(socket: &std::net::UdpSocket) { let _ = socket.local_addr(); }"#,
    );
    let std_source = r#"
        fn relay(socket: &std::net::UdpSocket) {
            let _ = socket.send_to(b"payload", "127.0.0.1:9");
            let _ = socket.send_to(b"payload", "127.0.0.1:10");
        }
    "#;
    assert_rustc_accepts(std_source, false);
    let std_events = network_capability_events(std_source);
    assert_ne!(std_baseline, std_events);
    assert_eq!(
        std_events
            .iter()
            .filter(|event| *event == "network-instance-method:send_to")
            .count(),
        2,
        "instance-method multiplicity must remain visible"
    );

    let tokio_source = r#"
        async fn relay(socket: &tokio::net::UdpSocket) {
            socket.send_to(b"payload", "127.0.0.1:9").await;
            socket.try_send_to(b"payload", "127.0.0.1:10");
        }
    "#;
    assert_rustc_accepts(tokio_source, true);
    let tokio_baseline = network_capability_events(
        r#"async fn relay(socket: &tokio::net::UdpSocket) { let _ = socket; }"#,
    );
    let tokio_events = network_capability_events(tokio_source);
    assert_ne!(tokio_baseline, tokio_events);
    assert!(
        tokio_events
            .iter()
            .any(|event| event == "network-instance-method:send_to")
    );
    assert!(
        tokio_events
            .iter()
            .any(|event| event == "network-instance-method:try_send_to")
    );

    let socket2_source = r#"
        fn relay(socket: &socket2::Socket, address: std::net::SocketAddr) {
            let address = address.into();
            let _ = socket.connect(&address);
            let _ = socket.connect_timeout(&address, std::time::Duration::from_secs(1));
            let buffers = [std::io::IoSlice::new(b"payload")];
            let _ = socket.send_to_vectored(&buffers, &address);
            let _ = socket.send_to_vectored_with_flags(&buffers, &address, 0);
        }
    "#;
    assert_cargo_accepts(
        socket2_source,
        "socket2 = { version = \"=0.5.10\", features = [\"all\"] }",
    );
    let socket2_baseline = network_capability_events(
        r#"fn relay(socket: &socket2::Socket) { let _ = socket.local_addr(); }"#,
    );
    let socket2_events = network_capability_events(socket2_source);
    assert_ne!(socket2_baseline, socket2_events);
    assert!(
        socket2_events
            .iter()
            .any(|event| event == "network-instance-method:connect")
    );
    assert!(
        socket2_events
            .iter()
            .any(|event| event == "network-instance-method:connect_timeout")
    );
    assert!(
        socket2_events
            .iter()
            .any(|event| event == "network-instance-method:send_to_vectored")
    );
    assert!(
        socket2_events
            .iter()
            .any(|event| event == "network-instance-method:send_to_vectored_with_flags")
    );
}

#[test]
fn finite_seam_and_outbound_methods_in_macro_tokens_remain_visible() {
    let finite = r#"
        mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        pub use crate::exec::post_osv_batch as exported;
        use crate::exec::post_osv_batch as send;
        async fn relay(packages: &[u8]) {
            let first = crate::exec::post_osv_batch;
            first(packages).await;
            send(packages).await;
            (crate::exec::post_osv_batch)(packages).await;
        }
    "#;
    assert_rustc_accepts(finite, false);
    assert_eq!(
        network_capability_events(finite)
            .iter()
            .filter(|event| event.starts_with("finite-authority-reference:"))
            .count(),
        3,
        "function-item acquisition, an alias, and a parenthesized reference must each change the inventory"
    );
    assert_eq!(
        network_capability_events(finite)
            .iter()
            .filter(|event| event.starts_with("finite-seam-import:"))
            .count(),
        2,
        "a finite-seam import or public reexport must itself change the authority inventory"
    );

    let macro_tokens = r#"
        mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        struct Socket;
        impl Socket { fn connect(&self, _: &str) {} }
        macro_rules! ignore { ($($tokens:tt)*) => {}; }
        fn relay(socket: Socket) {
            let _ = vec![crate::exec::post_osv_batch];
            let _ = vec![{ socket.connect("example.invalid:443"); 0 }];
            ignore!(socket, connect);
            ignore!(post_osv_batch);
        }
    "#;
    assert_rustc_accepts(macro_tokens, false);
    let macro_events = network_capability_events(macro_tokens);
    assert_eq!(
        macro_events
            .iter()
            .filter(|event| event.starts_with("finite-seam-macro-reference:"))
            .count(),
        2,
        "full and assembled finite-seam references in macro tokens must each be inventoried: {macro_events:?}"
    );
    assert_eq!(
        macro_events
            .iter()
            .filter(|event| event.starts_with("network-macro-outbound-method:connect:"))
            .count(),
        2,
        "literal and assembled outbound methods in macro tokens must each be inventoried: {macro_events:?}"
    );
}

#[test]
fn function_item_calls_and_additional_outbound_methods_change_the_inventory() {
    // Exercise retained registry authority directly: these are classification
    // assertions, with no transport implementation or executed call.
    let imports = syn::parse_file("pub use std::net::TcpStream as RecordedAuthority;").unwrap();
    let reexports =
        syn::parse_file("pub use crate::bridge::RecordedAuthority as Exported;").unwrap();
    let registry = NetworkAuthorityRegistry::from_files(&[&reexports, &imports]);
    for spelling in ["Exported", "crate::bridge::RecordedAuthority"] {
        let path: syn::ExprPath = syn::parse_str(spelling).unwrap();
        let mut reference = registry.visitor();
        reference.record_expression_path(&path, false);
        assert!(
            reference
                .events
                .iter()
                .any(|(_, event)| { event == &format!("network-function-item-escape:{spelling}") }),
            "retained alias value authority must reach the categorical rejection"
        );
        assert!(
            reference
                .events
                .iter()
                .any(|(_, event)| { event == &format!("network-alias-expression:{spelling}") }),
            "reference evidence must remain distinct from escape evidence"
        );
        let mut callee = registry.visitor();
        callee.record_expression_path(&path, true);
        assert!(
            !callee
                .events
                .iter()
                .any(|(_, event)| event.contains("function-item-escape:"))
        );
    }

    // Public write-operation families in std, Tokio, futures I/O, and socket2.
    // Placeholder receivers test the conservative spelling policy, not types.
    let added_methods = [
        "write_fmt",
        "write_all_vectored",
        "write_all_buf",
        "poll_write",
        "poll_write_vectored",
        "poll_send",
        "poll_send_to",
        "send_with_flags",
        "send_vectored_with_flags",
        "send_out_of_band",
        "sendmsg",
        "sendfile",
        "flush",
        "poll_flush",
        "shutdown",
        "poll_shutdown",
        "close",
        "poll_close",
        "write_u8",
        "write_i8",
        "write_u16",
        "write_i16",
        "write_u32",
        "write_i32",
        "write_u64",
        "write_i64",
        "write_u128",
        "write_i128",
        "write_f32",
        "write_f64",
        "write_u16_le",
        "write_i16_le",
        "write_u32_le",
        "write_i32_le",
        "write_u64_le",
        "write_i64_le",
        "write_u128_le",
        "write_i128_le",
        "write_f32_le",
        "write_f64_le",
    ];
    for method in added_methods {
        let events = network_capability_events(&format!(
            "fn policy() {{ writer.{method}(); writer.{method}(); }}"
        ));
        assert_eq!(
            events
                .iter()
                .filter(|event| *event == &format!("network-instance-method:{method}"))
                .count(),
            2,
            "each write operation must preserve occurrence multiplicity: {method}"
        );
        assert_eq!(
            outbound_ufcs_operation(&format!("tokio::io::AsyncWriteExt::{method}")),
            Some(method),
            "method and UFCS routes share the same conservative operation set"
        );
    }
    for helper in [
        "std::io::copy",
        "tokio::io::copy",
        "tokio::io::copy_buf",
        "tokio::io::copy_bidirectional",
        "tokio::io::copy_bidirectional_with_sizes",
        "futures::io::copy",
        "futures::io::copy_buf",
        "futures_util::io::copy",
        "futures_util::io::copy_buf",
    ] {
        let events = network_capability_events(&format!(
            "use {helper} as transfer; fn policy() {{ transfer(); transfer(); }}"
        ));
        assert_eq!(
            events
                .iter()
                .filter(|event| *event == &format!("network-copy-outbound:{helper}"))
                .count(),
            2
        );
    }
    let formatted = r#"
        use std::io::Write as Output;
        fn policy(mut writer: Vec<u8>) {
            let _ = writer.write_fmt(format_args!("policy"));
            let _ = Output::write_fmt(&mut writer, format_args!("policy"));
            let _ = <Vec<u8> as Output>::write_fmt(&mut writer, format_args!("policy"));
        }
    "#;
    assert_rustc_accepts(formatted, false);
    let formatted_events = network_capability_events(formatted);
    assert_eq!(
        formatted_events
            .iter()
            .filter(|event| event.starts_with("network-ufcs-outbound:write_fmt:"))
            .count(),
        2
    );
    assert_eq!(
        formatted_events
            .iter()
            .filter(|event| *event == "network-instance-method:write_fmt")
            .count(),
        1
    );
    assert!(
        network_capability_events(
            "fn policy() { value.is_write_vectored(); value.poll_write_ready(); local::copy(); }"
        )
        .is_empty()
    );

    let finite_one = r#"
        mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        async fn relay(packages: &[u8]) {
            let send = crate::exec::post_osv_batch;
            send(packages).await;
        }
    "#;
    let finite_two = r#"
        mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        async fn relay(packages: &[u8]) {
            let send = crate::exec::post_osv_batch;
            send(packages).await;
            send(packages).await;
        }
    "#;
    assert_rustc_accepts(finite_one, false);
    assert_rustc_accepts(finite_two, false);
    let finite_one_events = network_capability_events(finite_one);
    let finite_two_events = network_capability_events(finite_two);
    assert_eq!(
        finite_one_events
            .iter()
            .filter(|event| event.starts_with("finite-function-item-escape:"))
            .count(),
        1,
        "storing finite authority is categorically disallowed even when the bounded local flow is countable: {finite_one_events:?}"
    );
    assert_eq!(
        finite_one_events
            .iter()
            .filter(|event| event.starts_with("finite-function-item-invocation:send:"))
            .count(),
        1,
        "a finite function-item call must be distinguished from acquisition: {finite_one_events:?}"
    );
    assert_eq!(
        finite_two_events
            .iter()
            .filter(|event| event.starts_with("finite-function-item-invocation:send:"))
            .count(),
        2,
        "each indirect finite invocation must change the inventory: {finite_two_events:?}"
    );

    let network_one = r#"
        fn relay() {
            let dial = std::net::TcpStream::connect;
            let _ = dial("one.invalid:443");
        }
    "#;
    let network_two = r#"
        fn relay() {
            let dial = std::net::TcpStream::connect;
            let _ = dial("one.invalid:443");
            let _ = dial("two.invalid:443");
        }
    "#;
    assert_rustc_accepts(network_one, false);
    assert_rustc_accepts(network_two, false);
    let network_one_events = network_capability_events(network_one);
    let network_two_events = network_capability_events(network_two);
    assert_eq!(
        network_one_events
            .iter()
            .filter(|event| event.starts_with("network-function-item-escape:"))
            .count(),
        1,
        "storing network authority is categorically disallowed even when the bounded local flow is countable: {network_one_events:?}"
    );
    assert_eq!(
        network_one_events
            .iter()
            .filter(|event| event.starts_with("network-function-item-invocation:dial:"))
            .count(),
        1,
        "a network function-item call must be distinguished from acquisition: {network_one_events:?}"
    );
    assert_eq!(
        network_two_events
            .iter()
            .filter(|event| event.starts_with("network-function-item-invocation:dial:"))
            .count(),
        2,
        "each indirect network invocation must change the inventory: {network_two_events:?}"
    );

    let std_methods = r#"
        use std::io::Write;
        fn relay(udp: &std::net::UdpSocket, mut tcp: &std::net::TcpStream) {
            let _ = udp.send(b"packet");
            let _ = tcp.write_all(b"request");
        }
    "#;
    assert_rustc_accepts(std_methods, false);
    let std_events = network_capability_events(std_methods);
    assert!(
        std_events
            .iter()
            .any(|event| event == "network-instance-method:send"),
        "connected UDP sends must change the inventory: {std_events:?}"
    );
    assert!(
        std_events
            .iter()
            .any(|event| event == "network-instance-method:write_all"),
        "TCP writes must change the inventory: {std_events:?}"
    );

    let tokio_methods = r#"
        async fn relay(tcp: &tokio::net::TcpStream, udp: &tokio::net::UdpSocket) {
            tcp.try_write(b"request");
            udp.send(b"packet").await;
            udp.try_send(b"packet");
        }
    "#;
    assert_rustc_accepts(tokio_methods, true);
    let tokio_events = network_capability_events(tokio_methods);
    for method in ["try_write", "send", "try_send"] {
        assert!(
            tokio_events
                .iter()
                .any(|event| event == &format!("network-instance-method:{method}")),
            "Tokio outbound method {method} must change the inventory: {tokio_events:?}"
        );
    }

    let std_ufcs = r#"
        use std::io::Write as Sink;
        fn relay(mut stream: &std::net::TcpStream) {
            let _ = std::io::Write::write_all(&mut stream, b"one");
            let _ = Sink::write_all(&mut stream, b"two");
        }
    "#;
    assert_rustc_accepts(std_ufcs, false);
    let std_ufcs_events = network_capability_events(std_ufcs);
    assert_eq!(
        std_ufcs_events
            .iter()
            .filter(|event| event.starts_with("network-ufcs-outbound:write_all:"))
            .count(),
        2,
        "absolute and imported std Write UFCS calls must preserve multiplicity: {std_ufcs_events:?}"
    );

    let tokio_ufcs = r#"
        use tokio::io::AsyncWriteExt as Sink;
        async fn relay(stream: &mut tokio::net::TcpStream) {
            tokio::io::AsyncWriteExt::write_all(stream, b"one").await;
            Sink::write_all(stream, b"two").await;
        }
    "#;
    assert_rustc_accepts(tokio_ufcs, true);
    let tokio_ufcs_events = network_capability_events(tokio_ufcs);
    assert_eq!(
        tokio_ufcs_events
            .iter()
            .filter(|event| event.starts_with("network-ufcs-outbound:write_all:"))
            .count(),
        2,
        "absolute and imported Tokio AsyncWriteExt UFCS calls must preserve multiplicity: {tokio_ufcs_events:?}"
    );

    let stored_authority = r#"
        type Dial = fn(std::net::SocketAddr) -> std::io::Result<std::net::TcpStream>;
        const CONST_DIAL: Dial = std::net::TcpStream::connect::<std::net::SocketAddr>;
        static STATIC_DIAL: Dial = std::net::TcpStream::connect::<std::net::SocketAddr>;
        struct Stored { dial: Dial }
        fn harmless(_: std::net::SocketAddr) -> std::io::Result<std::net::TcpStream> {
            Err(std::io::Error::other("not a dial"))
        }
        fn forwarded() -> Dial {
            std::net::TcpStream::connect::<std::net::SocketAddr>
        }
        fn identity<T>(value: T) -> T { value }
        fn relay() {
            let tuple = (std::net::TcpStream::connect::<std::net::SocketAddr>,);
            let stored = Stored {
                dial: std::net::TcpStream::connect::<std::net::SocketAddr>,
            };
            let factory = || std::net::TcpStream::connect::<std::net::SocketAddr>;
            let typed: Dial = std::net::TcpStream::connect::<std::net::SocketAddr>;
            let mut reassigned: Dial = harmless;
            reassigned = std::net::TcpStream::connect::<std::net::SocketAddr>;
            let address = "127.0.0.1:9".parse().unwrap();
            let _ = CONST_DIAL(address);
            let _ = STATIC_DIAL(address);
            let _ = (tuple.0)(address);
            let _ = (stored.dial)(address);
            let _ = factory()(address);
            let _ = typed(address);
            let _ = reassigned(address);
            let _ = forwarded()(address);
            let _ = (identity(std::net::TcpStream::connect::<std::net::SocketAddr>))(address);
        }
    "#;
    assert_rustc_accepts(stored_authority, false);
    let stored_events = network_capability_events(stored_authority);
    assert_eq!(
        stored_events
            .iter()
            .filter(|event| event.starts_with("network-function-item-escape:"))
            .count(),
        9,
        "const, static, tuple, field, closure, return, typed local, reassignment, and nested-callee argument storage must all be rejected as escaping network authority rather than relying on incomplete value-flow inference: {stored_events:?}"
    );
}

#[test]
fn finite_authority_survives_cross_file_reexports_and_call_multiplicity() {
    let baseline = r#"
        pub mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        use crate::b::{second as send};
        mod b;
        mod a;
    "#;
    let attack = r#"
        pub mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        use crate::b::{second as send};
        async fn relay(packages: &[u8]) {
            send(packages).await;
            crate::a::exported(packages).await;
            crate::a::exported(packages).await;
        }
        mod b;
        mod a;
    "#;
    let first_hop = "pub use crate::exec::post_osv_batch as exported;";
    let second_hop = "pub use crate::a::{exported as second};";
    assert_rustc_accepts_with_modules(attack, &[("a.rs", first_hop), ("b.rs", second_hop)]);

    let baseline_events =
        network_capability_events_with_sources(baseline, &[second_hop, first_hop]);
    let attack_events = network_capability_events_with_sources(attack, &[second_hop, first_hop]);
    assert!(
        baseline_events
            .iter()
            .any(|event| event == "finite-seam-import:crate::b::second->send"),
        "the global fixed point must carry finite authority across two renamed reexports even when their modules are declared later: {baseline_events:?}"
    );
    assert_ne!(
        baseline_events, attack_events,
        "acquiring/calling a cross-file finite reexport must change the inventory"
    );
    assert_eq!(
        attack_events
            .iter()
            .filter(|event| event.starts_with("finite-authority-reference:"))
            .count(),
        3,
        "a renamed acquisition and two qualified acquisitions must preserve multiplicity: {attack_events:?}"
    );
    assert_eq!(
        attack_events
            .iter()
            .filter(|event| event.starts_with("finite-call:"))
            .count(),
        3,
        "every finite call occurrence must remain visible: {attack_events:?}"
    );

    let glob_consumer = r#"
        pub mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        use crate::a::*;
        async fn relay(packages: &[u8]) { exported(packages).await; }
        mod b;
        mod a;
    "#;
    assert_rustc_accepts_with_modules(glob_consumer, &[("a.rs", first_hop), ("b.rs", second_hop)]);
    let glob_events =
        network_capability_events_with_sources(glob_consumer, &[second_hop, first_hop]);
    assert!(
        glob_events
            .iter()
            .any(|event| event == "finite-authority-reference:exported"),
        "a finite reexport acquired through a cross-file glob must remain visible: {glob_events:?}"
    );
}

#[test]
fn named_zero_argument_macros_and_wrapper_chains_preserve_authority() {
    // Exact helper-path tokens carry the same authority as their AST paths.
    // These are policy inputs only; no macro is expanded or transport run.
    for helper in [
        "std::io::copy",
        "tokio::io::copy",
        "tokio::io::copy_buf",
        "tokio::io::copy_bidirectional",
        "tokio::io::copy_bidirectional_with_sizes",
        "futures::io::copy",
        "futures::io::copy_buf",
        "futures_util::io::copy",
        "futures_util::io::copy_buf",
    ] {
        let definition = format!(
            "macro_rules! primitive {{ () => {{ {helper} }}; }}
             macro_rules! wrapper {{ () => {{ primitive!() }}; }}
             pub(crate) use wrapper as exported;"
        );
        let definitions = network_capability_events(&definition);
        for name in ["primitive", "wrapper"] {
            assert_eq!(
                definitions
                    .iter()
                    .filter(|event| {
                        event.starts_with(&format!("network-macro-definition:{name}:"))
                    })
                    .count(),
                1,
                "copy helper authority must fingerprint {name}: {helper}"
            );
        }
        let consumer = "use crate::helpers::exported as retained;
                        fn policy() { retained!(); retained!(); }";
        let invocations = network_capability_events_with_sources(consumer, &[&definition]);
        assert!(
            invocations.iter().any(|event| {
                event == "network-sensitive-macro-import:crate::helpers::exported->retained"
            }),
            "copy wrapper authority must survive the registry and import: {helper}"
        );
        assert_eq!(
            invocations
                .iter()
                .filter(|event| {
                    event.starts_with("network-local-macro-invocation:sensitive:retained:")
                })
                .count(),
            2,
            "duplicate imported copy-wrapper invocations remain distinct: {helper}"
        );
        let tokens = network_capability_events(&format!(
            "fn policy() {{ inventory!({helper}, {helper}); }}"
        ));
        assert_eq!(
            tokens
                .iter()
                .filter(|event| { *event == &format!("network-macro-token:{helper}") })
                .count(),
            2,
            "each exact copy-helper token path remains visible: {helper}"
        );
    }
    assert!(
        network_capability_events(
            "macro_rules! local_copy { () => { local::copy }; }
         fn policy() { local_copy!(); inventory!(local::copy, copy); }"
        )
        .is_empty(),
        "a bare copy spelling or unrelated local path is not network authority"
    );

    let direct = r#"
        pub mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        struct Socket;
        impl Socket { fn connect(&self, _: &str) {} }
        async fn relay(socket: &Socket, address: &str) {
            macro_rules! query {
                () => { crate::exec::post_osv_batch(&[]).await };
            }
            macro_rules! dial { () => { socket.connect(address) }; }
            query!();
            query!();
            dial!();
        }
    "#;
    assert_rustc_accepts(direct, false);
    let direct_events = network_capability_events(direct);
    assert_eq!(
        direct_events
            .iter()
            .filter(|event| event.starts_with("finite-local-macro-invocation:query:"))
            .count(),
        2,
        "an empty invocation must retain each finite-macro occurrence: {direct_events:?}"
    );
    assert!(
        direct_events
            .iter()
            .any(|event| event.starts_with("finite-macro-definition:query:")),
        "a named definition that embeds the finite seam must be fingerprinted: {direct_events:?}"
    );
    assert!(
        direct_events
            .iter()
            .any(|event| event.starts_with("network-macro-method-definition:dial:connect:")),
        "a named definition that embeds an outbound method must be fingerprinted: {direct_events:?}"
    );
    assert!(
        direct_events
            .iter()
            .any(|event| { event.starts_with("network-local-macro-invocation:sensitive:dial:") }),
        "an empty sensitive-macro invocation must remain visible: {direct_events:?}"
    );

    let wrapper = r#"
        pub mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        async fn relay() {
            macro_rules! inner {
                () => { crate::exec::post_osv_batch(&[]).await };
            }
            macro_rules! outer { () => { inner!() }; }
            outer!();
        }
    "#;
    assert_rustc_accepts(wrapper, false);
    let wrapper_events = network_capability_events(wrapper);
    assert!(
        wrapper_events
            .iter()
            .any(|event| event.starts_with("finite-macro-definition:outer:")),
        "finite authority must flow from an inner macro into its wrapper definition: {wrapper_events:?}"
    );
    assert!(
        wrapper_events
            .iter()
            .any(|event| event.starts_with("finite-local-macro-invocation:outer:")),
        "invoking only the wrapper must still add finite authority evidence: {wrapper_events:?}"
    );

    let cross_file_root = r#"
        pub mod exec { pub async fn post_osv_batch(_: &[u8]) {} }
        use crate::a::{exported as run};
        async fn relay() { run!(); run!(); }
        mod a;
    "#;
    let cross_file_module = r#"
        macro_rules! inner {
            () => { crate::exec::post_osv_batch(&[]).await };
        }
        pub(crate) use inner;
        macro_rules! outer { () => { $crate::a::inner!() }; }
        pub(crate) use outer as exported;
    "#;
    assert_rustc_accepts_with_module(cross_file_root, "a.rs", cross_file_module);
    let cross_file_events =
        network_capability_events_with_sources(cross_file_root, &[cross_file_module]);
    assert!(
        cross_file_events
            .iter()
            .any(|event| event == "finite-macro-import:crate::a::exported->run"),
        "renamed cross-file wrapper imports must carry finite authority: {cross_file_events:?}"
    );
    assert_eq!(
        cross_file_events
            .iter()
            .filter(|event| event.starts_with("finite-local-macro-invocation:run:"))
            .count(),
        2,
        "duplicate cross-file wrapper invocations must preserve multiplicity: {cross_file_events:?}"
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
         Add a reviewed finite request inside the private network seam."
    );
}

#[derive(Default)]
struct NetworkCapabilityVisitor {
    owner: String,
    events: Vec<(String, String)>,
    scopes: Vec<BTreeMap<String, String>>,
    callable_scopes: Vec<BTreeMap<String, String>>,
    finite_aliases: BTreeSet<String>,
    finite_namespaces: BTreeSet<String>,
    finite_macros: BTreeSet<String>,
    network_aliases: BTreeSet<String>,
    network_glob: bool,
    network_sensitive_macros: BTreeSet<String>,
    ambiguous_macros: BTreeSet<String>,
    network_macro_authority: bool,
    cargo_bindings: BTreeMap<String, String>,
}

#[derive(Clone, Default, Eq, PartialEq)]
struct NetworkAuthorityRegistry {
    finite_aliases: BTreeSet<String>,
    finite_namespaces: BTreeSet<String>,
    finite_macros: BTreeSet<String>,
    aliases: BTreeSet<String>,
    glob: bool,
    sensitive_macros: BTreeSet<String>,
    ambiguous_macros: BTreeSet<String>,
    cargo_bindings: BTreeMap<String, String>,
}

impl NetworkAuthorityRegistry {
    fn visitor(&self) -> NetworkCapabilityVisitor {
        NetworkCapabilityVisitor {
            finite_aliases: self.finite_aliases.clone(),
            finite_namespaces: self.finite_namespaces.clone(),
            finite_macros: self.finite_macros.clone(),
            network_aliases: self.aliases.clone(),
            network_glob: self.glob,
            network_sensitive_macros: self.sensitive_macros.clone(),
            ambiguous_macros: self.ambiguous_macros.clone(),
            cargo_bindings: self.cargo_bindings.clone(),
            ..NetworkCapabilityVisitor::default()
        }
    }

    fn absorb(&mut self, visitor: NetworkCapabilityVisitor) {
        self.finite_aliases.extend(visitor.finite_aliases);
        self.finite_namespaces.extend(visitor.finite_namespaces);
        self.finite_macros.extend(visitor.finite_macros);
        self.aliases.extend(visitor.network_aliases);
        self.glob |= visitor.network_glob;
        self.sensitive_macros
            .extend(visitor.network_sensitive_macros);
        self.ambiguous_macros.extend(visitor.ambiguous_macros);
    }

    fn from_files(files: &[&syn::File]) -> Self {
        Self::from_files_with_cargo_bindings(files, BTreeMap::new())
    }

    fn from_files_with_cargo_bindings(
        files: &[&syn::File],
        cargo_bindings: BTreeMap<String, String>,
    ) -> Self {
        let mut registry = Self {
            cargo_bindings,
            ..Self::default()
        };
        loop {
            let before = registry.clone();
            for file in files {
                let mut visitor = registry.visitor();
                visitor.visit_file(file);
                registry.absorb(visitor);
            }
            if registry == before {
                return registry;
            }
        }
    }
}

impl NetworkCapabilityVisitor {
    fn ident_name(ident: &impl ToString) -> String {
        let spelling = ident.to_string();
        spelling.strip_prefix("r#").unwrap_or(&spelling).to_owned()
    }

    fn path_name(path: &syn::Path) -> String {
        let name = path
            .segments
            .iter()
            .map(|segment| Self::ident_name(&segment.ident))
            .collect::<Vec<_>>()
            .join("::");
        if path.leading_colon.is_some() {
            format!("::{name}")
        } else {
            name
        }
    }

    fn alias(&self, name: &str) -> Option<&str> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(String::as_str))
            .or_else(|| self.cargo_bindings.get(name).map(String::as_str))
    }

    fn callable(&self, name: &str) -> Option<&str> {
        self.callable_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(String::as_str))
    }

    fn expression_path(expression: &syn::Expr) -> Option<&syn::Path> {
        match expression {
            syn::Expr::Path(path) => Some(&path.path),
            syn::Expr::Group(group) => Self::expression_path(&group.expr),
            syn::Expr::Paren(paren) => Self::expression_path(&paren.expr),
            _ => None,
        }
    }

    fn canonical_extern_name(&self, source: &str) -> String {
        let mut segments = source.split("::");
        let first = segments.next().unwrap_or_default();
        let suffix = segments.collect::<Vec<_>>().join("::");
        self.cargo_bindings
            .get(first)
            .map(|package| {
                if suffix.is_empty() {
                    package.clone()
                } else {
                    format!("{package}::{suffix}")
                }
            })
            .unwrap_or_else(|| source.to_owned())
    }

    fn canonical_path(&self, path: &syn::Path) -> String {
        let mut name = Self::path_name(path);
        if let Some(absolute) = name.strip_prefix("::") {
            // A leading `::` starts in the extern prelude in Rust 2018+.
            // Local/module aliases with the same spelling are irrelevant.
            return self.canonical_extern_name(absolute);
        }
        for _ in 0..16 {
            let (first, suffix) = name
                .split_once("::")
                .map(|(first, suffix)| (first.to_owned(), Some(suffix.to_owned())))
                .unwrap_or_else(|| (name.clone(), None));
            let Some(expanded) = self.alias(&first).map(str::to_owned) else {
                break;
            };
            let next = suffix
                .map(|suffix| format!("{expanded}::{suffix}"))
                .unwrap_or(expanded);
            if next == name {
                break;
            }
            name = next;
        }
        name
    }

    fn has_finite_authority(&self, name: &str) -> bool {
        is_finite_seam_name(name)
            || name
                .trim_start_matches("::")
                .split("::")
                .any(|segment| self.finite_aliases.contains(segment))
    }

    fn remember_finite_binding(&mut self, binding: &str, source: &str) {
        self.finite_aliases.insert(binding.to_owned());
        if let Some((namespace, _)) = source.rsplit_once("::") {
            self.finite_namespaces.insert(namespace.to_owned());
        }
    }

    fn use_bindings(
        tree: &syn::UseTree,
        prefix: &mut Vec<String>,
        bindings: &mut Vec<(String, String)>,
        globs: &mut Vec<String>,
    ) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(Self::ident_name(&path.ident));
                Self::use_bindings(&path.tree, prefix, bindings, globs);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                let name = Self::ident_name(&name.ident);
                if name == "self" {
                    if let Some(binding) = prefix.last() {
                        bindings.push((binding.clone(), prefix.join("::")));
                    }
                    return;
                }
                let mut source = prefix.clone();
                source.push(name.clone());
                bindings.push((name, source.join("::")));
            }
            syn::UseTree::Rename(rename) => {
                let name = Self::ident_name(&rename.ident);
                if name == "self" {
                    bindings.push((Self::ident_name(&rename.rename), prefix.join("::")));
                    return;
                }
                let mut source = prefix.clone();
                source.push(name);
                bindings.push((Self::ident_name(&rename.rename), source.join("::")));
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    Self::use_bindings(item, prefix, bindings, globs);
                }
            }
            syn::UseTree::Glob(_) => globs.push(prefix.join("::")),
        }
    }

    fn install_use(&mut self, tree: &syn::UseTree, absolute: bool, record: bool) {
        let mut bindings = Vec::new();
        let mut globs = Vec::new();
        Self::use_bindings(tree, &mut Vec::new(), &mut bindings, &mut globs);
        for (binding, source) in bindings {
            let source = if absolute {
                self.canonical_extern_name(&source)
            } else {
                self.canonical_name(&source)
            };
            let through_alias = source
                .split("::")
                .any(|segment| self.network_aliases.contains(segment));
            let macro_name = source.rsplit("::").next().unwrap_or(&source);
            let finite = self.has_finite_authority(&source);
            let finite_macro = self.finite_macros.contains(macro_name);
            let sensitive_macro = self.network_sensitive_macros.contains(macro_name);
            let ambiguous_macro = self.ambiguous_macros.contains(macro_name);
            if finite {
                self.remember_finite_binding(&binding, &source);
            }
            if finite_macro {
                self.finite_macros.insert(binding.clone());
            }
            if is_network_namespace(&source) || is_network_crate_root(&source) || through_alias {
                self.network_aliases.insert(binding.clone());
            }
            if sensitive_macro {
                self.network_sensitive_macros.insert(binding.clone());
            }
            if ambiguous_macro {
                self.ambiguous_macros.insert(binding.clone());
            }
            if record && finite {
                self.events.push((
                    self.owner.clone(),
                    format!("finite-seam-import:{source}->{binding}"),
                ));
            }
            if record && finite_macro {
                self.events.push((
                    self.owner.clone(),
                    format!("finite-macro-import:{source}->{binding}"),
                ));
            }
            if record && is_network_namespace(&source) {
                self.events.push((
                    self.owner.clone(),
                    format!("network-import:{source}->{binding}"),
                ));
            } else if record && is_network_crate_root(&source) {
                self.events.push((
                    self.owner.clone(),
                    format!("network-root-import:{source}->{binding}"),
                ));
            } else if record && through_alias {
                self.events.push((
                    self.owner.clone(),
                    format!("network-alias-import:{source}->{binding}"),
                ));
            }
            if record && sensitive_macro {
                self.events.push((
                    self.owner.clone(),
                    format!("network-sensitive-macro-import:{source}->{binding}"),
                ));
            } else if record && ambiguous_macro {
                self.events.push((
                    self.owner.clone(),
                    format!("network-ambiguous-macro-import:{source}->{binding}"),
                ));
            }
            self.scopes
                .last_mut()
                .expect("network visitor scope")
                .insert(binding, source);
        }
        for source in globs {
            let source = if absolute {
                self.canonical_extern_name(&source)
            } else {
                self.canonical_name(&source)
            };
            let through_alias = source
                .split("::")
                .any(|segment| self.network_aliases.contains(segment));
            if record && self.finite_namespaces.contains(&source) {
                self.events.push((
                    self.owner.clone(),
                    format!("finite-seam-glob-import:{source}"),
                ));
            }
            if record && is_network_namespace(&source) {
                self.events
                    .push((self.owner.clone(), format!("network-glob-import:{source}")));
            } else if record && is_network_crate_root(&source) {
                self.events.push((
                    self.owner.clone(),
                    format!("network-root-glob-import:{source}"),
                ));
            } else if record && through_alias {
                self.events.push((
                    self.owner.clone(),
                    format!("network-alias-glob-import:{source}"),
                ));
            }
            if is_network_namespace(&source) || is_network_crate_root(&source) || through_alias {
                self.network_glob = true;
            }
        }
    }

    fn install_extern_crate(&mut self, item: &syn::ItemExternCrate, record: bool) {
        let binding = item
            .rename
            .as_ref()
            .map(|(_, rename)| Self::ident_name(rename))
            .unwrap_or_else(|| Self::ident_name(&item.ident));
        // `extern crate` resolves in the extern prelude rather than through a
        // same-spelled local `use`, so its source must not be alias-expanded.
        let source = Self::ident_name(&item.ident);
        if is_network_crate_root(&source) {
            self.network_aliases.insert(binding.clone());
        }
        if record && is_network_crate_root(&source) {
            self.events.push((
                self.owner.clone(),
                format!("network-extern-crate:{source}->{binding}"),
            ));
        }
        self.scopes
            .last_mut()
            .expect("network visitor scope")
            .insert(binding, source);
    }

    fn canonical_name(&self, source: &str) -> String {
        let mut segments = source.split("::");
        let first = segments.next().unwrap_or_default();
        let suffix = segments.collect::<Vec<_>>().join("::");
        self.alias(first)
            .map(|expanded| {
                if suffix.is_empty() {
                    expanded.to_owned()
                } else {
                    format!("{expanded}::{suffix}")
                }
            })
            .unwrap_or_else(|| source.to_owned())
    }

    fn preinstall_item_alias(&mut self, item: &syn::Item) {
        match item {
            syn::Item::ExternCrate(item) => self.install_extern_crate(item, false),
            syn::Item::Use(item) => {
                self.install_use(&item.tree, item.leading_colon.is_some(), false)
            }
            syn::Item::Type(item) => {
                if let syn::Type::Path(path) = item.ty.as_ref() {
                    let target = self.canonical_path(&path.path);
                    if is_network_namespace(&target) {
                        self.network_aliases.insert(Self::ident_name(&item.ident));
                    }
                    self.scopes
                        .last_mut()
                        .expect("network visitor scope")
                        .insert(Self::ident_name(&item.ident), target);
                }
            }
            syn::Item::Macro(item) => {
                let (aliases, glob) = macro_network_bindings(&item.mac.tokens);
                self.network_aliases.extend(aliases);
                self.network_glob |= glob;
                if let Some(ident) = &item.ident {
                    let name = Self::ident_name(ident);
                    if self.tokens_have_finite_authority(&item.mac.tokens) {
                        self.finite_macros.insert(name.clone());
                    }
                    if self.tokens_have_network_authority(&item.mac.tokens) {
                        self.network_sensitive_macros.insert(name.clone());
                    }
                    if self.tokens_have_ambiguous_authority(&item.mac.tokens) {
                        self.ambiguous_macros.insert(name);
                    }
                }
            }
            _ => {}
        }
    }

    fn preinstall_item_aliases<'item>(
        &mut self,
        items: impl IntoIterator<Item = &'item syn::Item>,
    ) {
        let items = items.into_iter().collect::<Vec<_>>();
        // Item imports, extern crates, and type aliases are scope-wide,
        // independent of declaration order. Iterate to resolve simple chains.
        for _ in 0..items.len().max(1) {
            for item in &items {
                self.preinstall_item_alias(item);
            }
        }
    }

    fn record_path(&mut self, kind: &str, path: &syn::Path) {
        let raw = Self::path_name(path);
        let canonical = self.canonical_path(path);
        if is_network_namespace(&canonical) {
            self.events
                .push((self.owner.clone(), format!("network-{kind}:{canonical}")));
        } else if self.has_network_alias_authority(&raw) {
            self.events
                .push((self.owner.clone(), format!("network-alias-{kind}:{raw}")));
        } else if self.network_glob {
            self.events
                .push((self.owner.clone(), format!("network-glob-{kind}:{raw}")));
        } else if self.network_macro_authority {
            self.events.push((
                self.owner.clone(),
                format!("network-macro-context-{kind}:{raw}"),
            ));
        }
    }

    fn has_network_alias_authority(&self, name: &str) -> bool {
        name.split("::")
            .any(|segment| self.network_aliases.contains(segment))
    }

    fn record_macro_tokens(&mut self, tokens: proc_macro2::TokenStream) {
        // A bare outbound-method spelling gets its own per-occurrence event in
        // `record_invocation_token_authorities`; it must not taint every later
        // expression in the file as though the macro had introduced aliases.
        let has_network_authority = self.tokens_have_network_context_authority(&tokens);
        if has_network_authority {
            let digest = hex::encode(Sha256::digest(tokens.to_string().as_bytes()));
            self.events.push((
                self.owner.clone(),
                format!("network-macro-sensitive:{digest}"),
            ));
        }
        let (aliases, glob) = macro_network_bindings(&tokens);
        self.network_aliases.extend(aliases);
        self.network_glob |= glob;
        if macro_tokens_contain_dollar(&tokens) {
            let digest = hex::encode(Sha256::digest(tokens.to_string().as_bytes()));
            self.events.push((
                self.owner.clone(),
                format!("network-macro-ambiguous:dynamic-path:{digest}"),
            ));
        }
        self.record_literal_macro_paths(tokens);
    }

    fn record_invocation_token_authorities(&mut self, invocation: &syn::Macro) {
        let mut flat = Vec::new();
        flatten_macro_tokens(invocation.tokens.clone(), &mut flat);
        let material = format!(
            "{}!({})",
            Self::path_name(&invocation.path),
            invocation.tokens
        );
        let digest = hex::encode(Sha256::digest(material.as_bytes()));
        for token in flat {
            if token == "post_osv_batch" || self.finite_aliases.contains(&token) {
                self.events.push((
                    self.owner.clone(),
                    format!("finite-seam-macro-reference:{digest}"),
                ));
            }
            if is_outbound_instance_method(&token) {
                self.events.push((
                    self.owner.clone(),
                    format!("network-macro-outbound-method:{token}:{digest}"),
                ));
            }
        }
    }

    fn tokens_have_network_context_authority(&self, tokens: &proc_macro2::TokenStream) -> bool {
        if macro_tokens_have_network_authority(tokens)
            || self.network_glob
            || !self.literal_macro_network_paths(tokens.clone()).is_empty()
        {
            return true;
        }
        let mut flat = Vec::new();
        flatten_macro_tokens(tokens.clone(), &mut flat);
        flat.iter()
            .any(|token| self.network_aliases.contains(token))
            || invoked_macro_names(tokens)
                .iter()
                .any(|name| self.network_sensitive_macros.contains(name))
    }

    fn tokens_have_network_authority(&self, tokens: &proc_macro2::TokenStream) -> bool {
        self.tokens_have_network_context_authority(tokens)
            || !outbound_methods_in_tokens(tokens).is_empty()
    }

    fn tokens_have_finite_authority(&self, tokens: &proc_macro2::TokenStream) -> bool {
        let mut flat = Vec::new();
        flatten_macro_tokens(tokens.clone(), &mut flat);
        flat.iter()
            .any(|token| token == "post_osv_batch" || self.finite_aliases.contains(token))
            || invoked_macro_names(tokens)
                .iter()
                .any(|name| self.finite_macros.contains(name))
    }

    fn tokens_have_ambiguous_authority(&self, tokens: &proc_macro2::TokenStream) -> bool {
        macro_tokens_contain_dollar(tokens)
            || invoked_macro_names(tokens)
                .iter()
                .any(|name| self.ambiguous_macros.contains(name))
    }

    fn record_local_macro_invocation(&mut self, invocation: &syn::Macro) -> bool {
        let raw = Self::path_name(&invocation.path);
        let canonical = self.canonical_path(&invocation.path);
        let raw_name = raw.rsplit("::").next().unwrap_or(&raw);
        let canonical_name = canonical.rsplit("::").next().unwrap_or(&canonical);
        let sensitive = self.network_sensitive_macros.contains(raw_name)
            || self.network_sensitive_macros.contains(canonical_name);
        let finite =
            self.finite_macros.contains(raw_name) || self.finite_macros.contains(canonical_name);
        let ambiguous = self.ambiguous_macros.contains(raw_name)
            || self.ambiguous_macros.contains(canonical_name);
        let material = format!("{raw}!({})", invocation.tokens);
        let digest = hex::encode(Sha256::digest(material.as_bytes()));
        if finite {
            self.events.push((
                self.owner.clone(),
                format!("finite-local-macro-invocation:{raw}:{digest}"),
            ));
        }
        let kind = if sensitive {
            Some("sensitive")
        } else if ambiguous {
            Some("ambiguous")
        } else {
            None
        };
        let Some(kind) = kind else {
            return false;
        };
        self.events.push((
            self.owner.clone(),
            format!("network-local-macro-invocation:{kind}:{raw}:{digest}"),
        ));
        kind == "sensitive"
    }

    fn record_named_macro_definition(&mut self, item: &syn::ItemMacro) {
        let Some(ident) = &item.ident else {
            return;
        };
        let name = Self::ident_name(ident);
        let material = format!("macro_rules!{name}{{{}}}", item.mac.tokens);
        let digest = hex::encode(Sha256::digest(material.as_bytes()));
        if self.tokens_have_finite_authority(&item.mac.tokens) {
            self.events.push((
                self.owner.clone(),
                format!("finite-macro-definition:{name}:{digest}"),
            ));
        }
        if self.tokens_have_network_authority(&item.mac.tokens) {
            self.events.push((
                self.owner.clone(),
                format!("network-macro-definition:{name}:{digest}"),
            ));
        }
        for method in outbound_methods_in_tokens(&item.mac.tokens) {
            self.events.push((
                self.owner.clone(),
                format!("network-macro-method-definition:{name}:{method}:{digest}"),
            ));
        }
        if self.tokens_have_ambiguous_authority(&item.mac.tokens) {
            self.events.push((
                self.owner.clone(),
                format!("network-macro-ambiguous-definition:{name}:{digest}"),
            ));
        }
    }

    fn record_literal_macro_paths(&mut self, tokens: proc_macro2::TokenStream) {
        for canonical in self.literal_macro_network_paths(tokens) {
            self.events.push((
                self.owner.clone(),
                format!("network-macro-token:{canonical}"),
            ));
        }
    }

    fn literal_macro_network_paths(&self, tokens: proc_macro2::TokenStream) -> Vec<String> {
        use proc_macro2::TokenTree;

        let trees = tokens.into_iter().collect::<Vec<_>>();
        let mut found = Vec::new();
        for tree in &trees {
            if let TokenTree::Group(group) = tree {
                found.extend(self.literal_macro_network_paths(group.stream()));
            }
        }

        for start in 0..trees.len() {
            let TokenTree::Ident(first) = &trees[start] else {
                continue;
            };
            let mut segments = vec![Self::ident_name(first)];
            let mut cursor = start + 1;
            while cursor + 2 < trees.len() {
                let (TokenTree::Punct(left), TokenTree::Punct(right), TokenTree::Ident(next)) =
                    (&trees[cursor], &trees[cursor + 1], &trees[cursor + 2])
                else {
                    break;
                };
                if left.as_char() != ':' || right.as_char() != ':' {
                    break;
                }
                segments.push(Self::ident_name(next));
                cursor += 3;
            }
            if segments.len() < 2 {
                continue;
            }
            let preceded_by_colons = start >= 2
                && matches!(&trees[start - 2], TokenTree::Punct(mark) if mark.as_char() == ':')
                && matches!(&trees[start - 1], TokenTree::Punct(mark) if mark.as_char() == ':');
            let preceded_by_path_atom = start >= 3
                && matches!(
                    &trees[start - 3],
                    TokenTree::Ident(_) | TokenTree::Group(_) | TokenTree::Literal(_)
                );
            let source = segments.join("::");
            let canonical = if preceded_by_colons && !preceded_by_path_atom {
                self.canonical_extern_name(&source)
            } else {
                self.canonical_name(&source)
            };
            // Keep exact free-copy write authority on the same macro path
            // route as network namespaces, so definitions and wrapper/import
            // fixed points retain it without treating a bare `copy` as proof.
            if is_network_namespace(&canonical) || is_outbound_copy_helper(&canonical) {
                found.push(canonical);
            }
        }
        found
    }

    fn record_expression_path(&mut self, expression: &syn::ExprPath, direct_callee: bool) {
        let name = self.canonical_path(&expression.path);
        if self.has_finite_authority(&name) {
            self.events.push((
                self.owner.clone(),
                format!("finite-authority-reference:{name}"),
            ));
            if !direct_callee {
                self.events.push((
                    self.owner.clone(),
                    format!("finite-function-item-escape:{name}"),
                ));
            }
        } else if !direct_callee
            && (is_network_function_item_path(&name)
                || outbound_ufcs_operation(&name).is_some()
                || is_outbound_copy_helper(&name)
                // Cross-file registry aliases retain authority even when their
                // canonical target cannot be reconstructed locally. Treat such
                // value references conservatively; this does not infer a type.
                // Known namespace constants retain their existing treatment.
                || (!is_network_namespace(&name)
                    && (self.has_network_alias_authority(&name)
                        || self.has_network_alias_authority(&Self::path_name(&expression.path)))))
        {
            self.events.push((
                self.owner.clone(),
                format!("network-function-item-escape:{name}"),
            ));
        }
        self.record_path("expression", &expression.path);
    }

    fn visit_direct_call_callee(&mut self, expression: &syn::Expr) {
        match expression {
            syn::Expr::Path(path) => {
                self.record_expression_path(path, true);
                syn::visit::visit_expr_path(self, path);
            }
            syn::Expr::Group(group) => {
                for attribute in &group.attrs {
                    self.visit_attribute(attribute);
                }
                self.visit_direct_call_callee(&group.expr);
            }
            syn::Expr::Paren(paren) => {
                for attribute in &paren.attrs {
                    self.visit_attribute(attribute);
                }
                self.visit_direct_call_callee(&paren.expr);
            }
            _ => self.visit_expr(expression),
        }
    }
}

impl<'ast> Visit<'ast> for NetworkCapabilityVisitor {
    fn visit_file(&mut self, file: &'ast syn::File) {
        self.scopes.push(BTreeMap::new());
        self.callable_scopes.push(BTreeMap::new());
        self.preinstall_item_aliases(&file.items);
        for item in &file.items {
            self.visit_item(item);
        }
        self.callable_scopes.pop();
        self.scopes.pop();
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let previous = std::mem::replace(&mut self.owner, Self::ident_name(&item.sig.ident));
        syn::visit::visit_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let previous = std::mem::replace(&mut self.owner, Self::ident_name(&item.sig.ident));
        syn::visit::visit_impl_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        let previous = std::mem::replace(&mut self.owner, Self::ident_name(&item.sig.ident));
        syn::visit::visit_trait_item_fn(self, item);
        self.owner = previous;
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if let Some((_, items)) = &item.content {
            self.scopes.push(BTreeMap::new());
            self.callable_scopes.push(BTreeMap::new());
            self.preinstall_item_aliases(items);
            for item in items {
                self.visit_item(item);
            }
            self.callable_scopes.pop();
            self.scopes.pop();
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scopes.push(BTreeMap::new());
        self.callable_scopes.push(BTreeMap::new());
        self.preinstall_item_aliases(block.stmts.iter().filter_map(|statement| match statement {
            syn::Stmt::Item(item) => Some(item),
            _ => None,
        }));
        syn::visit::visit_block(self, block);
        self.callable_scopes.pop();
        self.scopes.pop();
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.install_use(&item.tree, item.leading_colon.is_some(), true);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        self.install_extern_crate(item, true);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if let syn::Type::Path(path) = item.ty.as_ref() {
            let target = self.canonical_path(&path.path);
            if is_network_namespace(&target) {
                self.network_aliases.insert(Self::ident_name(&item.ident));
            }
            if is_network_namespace(&target) {
                self.events.push((
                    self.owner.clone(),
                    format!(
                        "network-type-alias:{target}->{}",
                        Self::ident_name(&item.ident)
                    ),
                ));
            }
            self.scopes
                .last_mut()
                .expect("network visitor scope")
                .insert(Self::ident_name(&item.ident), target);
        }
        syn::visit::visit_item_type(self, item);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        syn::visit::visit_local(self, local);
        let syn::Pat::Ident(binding) = &local.pat else {
            return;
        };
        let Some(initializer) = &local.init else {
            return;
        };
        let Some(path) = Self::expression_path(&initializer.expr) else {
            return;
        };
        let raw = Self::path_name(path);
        let raw_name = raw.rsplit("::").next().unwrap_or(&raw);
        let target = self
            .callable(raw_name)
            .map(str::to_owned)
            .unwrap_or_else(|| self.canonical_path(path));
        if !self.has_finite_authority(&target) && !is_network_namespace(&target) {
            return;
        }
        let name = Self::ident_name(&binding.ident);
        let kind = if self.has_finite_authority(&target) {
            "finite-function-item-binding"
        } else {
            "network-function-item-binding"
        };
        self.events
            .push((self.owner.clone(), format!("{kind}:{name}:{target}")));
        self.callable_scopes
            .last_mut()
            .expect("network visitor callable scope")
            .insert(name, target);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let Some(path) = Self::expression_path(&expression.func) {
            let raw = Self::path_name(path);
            let raw_name = raw.rsplit("::").next().unwrap_or(&raw);
            if let Some(target) = self.callable(raw_name).map(str::to_owned) {
                let kind = if self.has_finite_authority(&target) {
                    "finite-function-item-invocation"
                } else {
                    "network-function-item-invocation"
                };
                self.events
                    .push((self.owner.clone(), format!("{kind}:{raw}:{target}")));
            } else {
                let name = self.canonical_path(path);
                if is_outbound_copy_helper(&name) {
                    self.events
                        .push((self.owner.clone(), format!("network-copy-outbound:{name}")));
                }
                if let Some(method) = outbound_ufcs_operation(&name) {
                    self.events.push((
                        self.owner.clone(),
                        format!("network-ufcs-outbound:{method}:{name}"),
                    ));
                }
                if self.has_finite_authority(&name) {
                    self.events
                        .push((self.owner.clone(), format!("finite-call:{name}")));
                }
            }
        }
        for attribute in &expression.attrs {
            self.visit_attribute(attribute);
        }
        self.visit_direct_call_callee(&expression.func);
        for argument in &expression.args {
            self.visit_expr(argument);
        }
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        let method = Self::ident_name(&expression.method);
        if is_outbound_instance_method(&method) {
            // The receiver's type is deliberately not inferred here. A method
            // with outbound semantics is authority that needs explicit review,
            // and preserving every occurrence keeps duplicate sites visible.
            self.events.push((
                self.owner.clone(),
                format!("network-instance-method:{method}"),
            ));
        }
        syn::visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        self.record_expression_path(expression, false);
        syn::visit::visit_expr_path(self, expression);
    }

    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        self.record_path("type", &ty.path);
        syn::visit::visit_type_path(self, ty);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let local_authority = self.record_local_macro_invocation(invocation);
        let token_authority = self.tokens_have_network_context_authority(&invocation.tokens);
        self.record_invocation_token_authorities(invocation);
        self.record_path("macro", &invocation.path);
        self.record_macro_tokens(invocation.tokens.clone());
        self.network_macro_authority |= local_authority || token_authority;
        syn::visit::visit_macro(self, invocation);
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        if item.ident.is_some() {
            self.record_named_macro_definition(item);
            self.record_path("macro", &item.mac.path);
            self.record_macro_tokens(item.mac.tokens.clone());
        } else {
            self.visit_macro(&item.mac);
        }
    }

    fn visit_item_foreign_mod(&mut self, item: &'ast syn::ItemForeignMod) {
        for foreign in &item.items {
            if let syn::ForeignItem::Fn(function) = foreign
                && matches!(
                    function.sig.ident.to_string().as_str(),
                    "socket" | "connect" | "send" | "sendto"
                )
            {
                self.events.push((
                    self.owner.clone(),
                    format!("network-ffi:{}", function.sig.ident),
                ));
            }
        }
        syn::visit::visit_item_foreign_mod(self, item);
    }
}

fn macro_tokens_contain_dollar(tokens: &proc_macro2::TokenStream) -> bool {
    tokens.clone().into_iter().any(|token| match token {
        proc_macro2::TokenTree::Punct(mark) => mark.as_char() == '$',
        proc_macro2::TokenTree::Group(group) => macro_tokens_contain_dollar(&group.stream()),
        _ => false,
    })
}

fn flatten_macro_tokens(tokens: proc_macro2::TokenStream, out: &mut Vec<String>) {
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Ident(ident) => {
                out.push(NetworkCapabilityVisitor::ident_name(&ident));
            }
            proc_macro2::TokenTree::Punct(punct) => out.push(punct.as_char().to_string()),
            proc_macro2::TokenTree::Group(group) => flatten_macro_tokens(group.stream(), out),
            proc_macro2::TokenTree::Literal(_) => out.push("<literal>".to_owned()),
        }
    }
}

fn invoked_macro_names(tokens: &proc_macro2::TokenStream) -> BTreeSet<String> {
    let mut flat = Vec::new();
    flatten_macro_tokens(tokens.clone(), &mut flat);
    flat.windows(2)
        .filter(|pair| pair[1] == "!")
        .map(|pair| pair[0].clone())
        .collect()
}

fn outbound_methods_in_tokens(tokens: &proc_macro2::TokenStream) -> BTreeSet<String> {
    let mut flat = Vec::new();
    flatten_macro_tokens(tokens.clone(), &mut flat);
    flat.into_iter()
        .filter(|token| is_outbound_instance_method(token))
        .collect()
}

fn macro_tokens_have_network_authority(tokens: &proc_macro2::TokenStream) -> bool {
    let (aliases, glob) = macro_network_bindings(tokens);
    if !aliases.is_empty() || glob {
        return true;
    }
    let mut flat = Vec::new();
    flatten_macro_tokens(tokens.clone(), &mut flat);
    let has_std = flat.iter().any(|token| token == "std");
    let has_tokio = flat.iter().any(|token| token == "tokio");
    let has_net = flat.iter().any(|token| token == "net");
    (has_net && (has_std || has_tokio))
        || flat.iter().any(|token| {
            matches!(
                token.as_str(),
                "socket2" | "reqwest" | "hyper" | "hyper_util" | "ureq" | "isahc" | "surf"
            )
        })
}

fn macro_network_bindings(tokens: &proc_macro2::TokenStream) -> (BTreeSet<String>, bool) {
    let mut flat = Vec::new();
    flatten_macro_tokens(tokens.clone(), &mut flat);
    let mut aliases = BTreeSet::new();
    let mut glob = false;
    for start in 0..flat.len() {
        let body = if flat[start] == "use" {
            start + 1
        } else if flat.get(start).is_some_and(|token| token == "extern")
            && flat.get(start + 1).is_some_and(|token| token == "crate")
        {
            start + 2
        } else {
            continue;
        };
        let end = flat[body..]
            .iter()
            .position(|token| token == ";")
            .map(|offset| body + offset)
            .unwrap_or(flat.len());
        let clause = &flat[body..end];
        let Some(root_index) = clause.iter().position(|token| is_network_crate_root(token)) else {
            continue;
        };
        let root = &clause[root_index];
        let tail = &clause[root_index + 1..];
        let first_meaningful = tail
            .iter()
            .find(|token| !matches!(token.as_str(), ":" | ","));
        let whole_root =
            first_meaningful.is_none_or(|token| matches!(token.as_str(), "as" | "self" | "*"));
        let network_path = tail.iter().any(|token| token == "net");
        let dedicated_network_crate = !matches!(root.as_str(), "std" | "tokio");
        if !whole_root && !network_path && !dedicated_network_crate {
            continue;
        }
        glob |= clause.iter().any(|token| token == "*");
        for pair in clause.windows(2) {
            if pair[0] == "as" && pair[1] != "_" {
                aliases.insert(pair[1].clone());
            }
        }
        for binding in clause.iter().skip(root_index + 1).filter(|token| {
            token
                .chars()
                .next()
                .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
                && !matches!(token.as_str(), "self" | "use" | "extern" | "crate" | "as")
        }) {
            aliases.insert(binding.clone());
        }
        if whole_root && !clause.iter().any(|token| token == "as" || token == "*") {
            aliases.insert(root.clone());
        }
    }
    (aliases, glob)
}

fn network_capability_events(source: &str) -> Vec<String> {
    network_capability_events_with_sources_and_bindings(source, &[], BTreeMap::new())
}

fn network_capability_events_with_sources(source: &str, related: &[&str]) -> Vec<String> {
    network_capability_events_with_sources_and_bindings(source, related, BTreeMap::new())
}

fn network_capability_events_with_cargo_bindings(
    source: &str,
    cargo_bindings: BTreeMap<String, String>,
) -> Vec<String> {
    network_capability_events_with_sources_and_bindings(source, &[], cargo_bindings)
}

fn network_capability_events_with_sources_and_bindings(
    source: &str,
    related: &[&str],
    cargo_bindings: BTreeMap<String, String>,
) -> Vec<String> {
    let file = syn::parse_file(source).expect("parse network capability fixture");
    let related = related
        .iter()
        .map(|source| syn::parse_file(source).expect("parse related network capability fixture"))
        .collect::<Vec<_>>();
    let mut files = vec![&file];
    files.extend(related.iter());
    let registry = NetworkAuthorityRegistry::from_files_with_cargo_bindings(&files, cargo_bindings);
    let mut visitor = registry.visitor();
    visitor.visit_file(&file);
    let mut events = visitor
        .events
        .into_iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    events.sort();
    events
}

fn network_capability_census() -> Vec<(String, String, String)> {
    let test_modules = anvil::source_scan::paths::declared_test_module_files(&repo())
        .expect("classify declared test modules");
    let mut files = Vec::new();
    rust_sources(&repo().join("src"), &mut files);
    let mut parsed = Vec::new();
    for path in files {
        let relative = path.strip_prefix(repo()).unwrap_or(&path);
        if is_test_source(&relative.to_string_lossy())
            || is_declared_test_file(&path, &test_modules)
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read production network source");
        let file = syn::parse_file(&without_test_modules(&source)).expect("parse network source");
        parsed.push((relative.to_string_lossy().replace('\\', "/"), file));
    }
    let files = parsed.iter().map(|(_, file)| file).collect::<Vec<_>>();
    let registry = NetworkAuthorityRegistry::from_files_with_cargo_bindings(
        &files,
        production_cargo_network_bindings(),
    );
    let mut events = Vec::new();
    for (relative, file) in parsed {
        let mut visitor = registry.visitor();
        visitor.visit_file(&file);
        events.extend(
            visitor
                .events
                .into_iter()
                .map(|(owner, event)| (relative.clone(), owner, event)),
        );
    }
    events.sort();
    events
}
