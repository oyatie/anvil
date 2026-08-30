//! A capability rung holds what was gathered, never a licence to go and get it.
//!
//! `Corpus` carried `build_graph: bool`, `toolchain: bool` and `network: bool`.
//! Each told a rule it *may* do something, which left the rule doing it inside
//! `Rule::examine` -- synchronous, returning findings, with nowhere to say the
//! attempt failed. An absent toolchain therefore read as a clean run, which is
//! the one confusion `Evaluated` exists to make unspellable.
//!
//! The fix is a shape, so this checks the shape. A `bool` on the corpus is the
//! defect whatever it is named.

use anvil::harness::Requires;
use anvil::harness::corpus::Corpus;
use anvil::harness::evidence::{BuildGraph, Fetched, ToolRun};

fn one_subject() -> Corpus {
    Corpus::of_paths(&["src/lib.rs"])
}

/// The scan that makes the next one unwritable.
///
/// Keyed to the declaration, not to a filename: the struct is what must hold no
/// permission flag, and moving it to another file must not silently pass.
#[test]
fn the_corpus_declares_no_boolean_field() {
    let src = anvil::source_scan::paths::module_source(
        "src/harness/corpus",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let body = anvil::source_scan::without_commentary(&src);
    let decl = body
        .split_once("pub struct Corpus {")
        .expect("Corpus must be declared here; if it moved, this test follows it")
        .1
        .split_once("\n}")
        .expect("the declaration must close")
        .0;

    let flags: Vec<&str> = decl
        .lines()
        .map(str::trim)
        .filter(|l| l.ends_with(": bool,"))
        .collect();
    assert!(
        flags.is_empty(),
        "the corpus declares {} boolean field(s): {flags:?}\n\
         A `bool` here grants a rule permission to go and get something, which \
         puts the I/O inside `examine` and leaves the failure with nowhere to \
         be reported. Carry what was gathered -- see `harness::evidence`.",
        flags.len()
    );
}

/// Absent evidence withholds. The whole point, at each of the three rungs.
#[test]
fn a_rung_with_nothing_gathered_is_not_satisfied() {
    let bare = one_subject();
    for needs in [Requires::BuildGraph, Requires::Toolchain, Requires::Network] {
        assert!(
            !bare.satisfies(needs),
            "{needs:?} was satisfied by a corpus holding nothing for it, so a \
             rule at that rung would run against absent data and its empty \
             finding list would read as clean"
        );
    }
}

#[test]
fn a_rung_is_satisfied_by_the_evidence_it_names_and_by_no_other() {
    let graph = one_subject().with_build_graph(BuildGraph::default());
    assert!(graph.satisfies(Requires::BuildGraph));
    assert!(
        !graph.satisfies(Requires::Toolchain),
        "a graph is not a tool run"
    );
    assert!(
        !graph.satisfies(Requires::Network),
        "a graph is not a fetch"
    );

    let tools = one_subject().with_tool_run(ToolRun {
        tool: "cargo".to_string(),
        exit_ok: true,
        stdout: String::new(),
        stderr: String::new(),
    });
    assert!(tools.satisfies(Requires::Toolchain));
    assert!(!tools.satisfies(Requires::BuildGraph));

    let fetched = one_subject().with_fetched(Fetched {
        source: "https://api.osv.dev/v1/querybatch".to_string(),
        body: "{}".to_string(),
    });
    assert!(fetched.satisfies(Requires::Network));
    assert!(!fetched.satisfies(Requires::Toolchain));
}

/// A failed invocation is still evidence, and must not read as absence.
///
/// The distinction the boolean could not draw at all: `exit_ok: false` is a
/// tool that ran and refused, which is a measurement. Withholding it would tell
/// the rule the toolchain was unavailable, which is a different fact.
#[test]
fn a_toolchain_that_ran_and_failed_is_evidence_not_absence() {
    let refused = one_subject().with_tool_run(ToolRun {
        tool: "clippy".to_string(),
        exit_ok: false,
        stdout: String::new(),
        stderr: "error: this expression creates a reference".to_string(),
    });
    assert!(
        refused.satisfies(Requires::Toolchain),
        "a tool that ran and refused is a measurement; withholding it would \
         report the toolchain as unavailable, which is a different fact"
    );
}
