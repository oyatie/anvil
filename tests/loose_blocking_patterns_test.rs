//! Lane `loose-blocking-patterns`: the two patterns that block a merge were the
//! two loosest patterns in the gate.
//!
//! # The defect, restated from source
//!
//! `security_scan_status` was published on the scorecard as a deep entropy scan
//! for leaked credentials, and computed no entropy at all -- `scanner.rs` had no `ln`, no
//! `log2`, no histogram. It ran six regexes and returned `Failed` on the first
//! hit. Four of the six are provider-anchored and honest (PEM header, `AKIA`,
//! `ghp_`, `gho_`). The two that had no structural anchor were:
//!
//!   - `(?i)sk-[A-Za-z0-9_-]{24,}` -- `sk-` is a substring of `task-`, `risk-`,
//!     `disk-`, `desk-`, `mask-`, and `[A-Za-z0-9_-]` admits the hyphen, so the
//!     rest of any kebab-case identifier satisfies the length floor. Verified
//!     firing on `task-scheduler-backpressure-limit`.
//!   - `(?i)password\s*[:=]\s*["'][^"']{6,}["']` -- fires on
//!     `password: "<redacted>"`, on `${DB_PASSWORD}`, and on commented-out
//!     placeholders.
//!
//! `cleartext_transport_status` -- then named `zero_trust_workload_status` --
//! was published as cryptographic SPIFFE ID
//! workload attestation and mTLS encryption, and decided on a case-folded
//! substring test for the plaintext scheme, excluding loopback. Any added line carrying a non-loopback `http://` -- a
//! licence URL in a doc comment, a link in a markdown file, a `format!` template
//! -- became `Failed`.
//!
//! # What the oracles actually do
//!
//! gitleaks does not reject `task-scheduler-backpressure-limit` by entropy: its
//! Shannon entropy is 3.847, above gitleaks' strictest generic threshold of 3.5.
//! It rejects it with an allowlist regex `^[a-zA-Z_.-]+$` applied to the
//! captured secret, a stopword list, and a required assignment context. Entropy
//! is the *last* filter in `detect.go`, applied to an already structurally
//! constrained capture group -- never the first. TruffleHog goes further and
//! calls the provider to confirm the credential is live; entropy there only
//! filters the *unverified* tail. GitHub secret scanning blocks pushes on
//! provider-specific patterns only, and routes generic password-like strings to
//! a separate AI feature that skips paths containing `test`, `mock` or `spec`.
//!
//! For the cleartext check: gosec's G107 is sink-anchored and explicitly does
//! not flag a constant literal `"http://127.0.0.1"`; semgrep's
//! `insecure-transport` pack matches parsed syntax, so a comment is not an AST
//! node and cannot match. That is the class this check belongs to -- CWE-319 --
//! and it is not SPIFFE workload attestation, which is a runtime property of an
//! X.509-SVID's URI SAN, not of any string in a diff.
//!
//! # Why these fixtures are assembled at runtime
//!
//! This repository scans its own tree. A 51-character OpenAI-shaped literal or
//! a plaintext `http://` URL written directly into this file would be a finding
//! against the commit that adds it, and `neither_gate_fires_on_this_repositorys_own_history`
//! below would go red on its own test fixtures. Every needle is therefore built
//! from parts at runtime; the source text of this file contains no credential
//! shape and no cleartext URL.

use anvil::pre_merge_guard::{GateStatus, PreMergeScanner};
use anvil::zero_trust_workload::ZeroTrustWorkloadGate;
use std::process::Command;

// ---------------------------------------------------------------------------
// Fixture construction
// ---------------------------------------------------------------------------

/// A deterministic high-entropy base62 run of `n` characters. Stride 37 is
/// coprime with 62, so the first 62 characters are distinct and the Shannon
/// entropy of a 48-character run is log2(48) = 5.58 -- comfortably above every
/// threshold any rule here uses, and reproducible without an RNG dependency.
fn base62(n: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..n)
        .map(|i| ALPHABET[(i * 37 + 11) % ALPHABET.len()] as char)
        .collect()
}

/// An OpenAI-shaped key: the legacy `sk-` + 48 alphanumerics form. Synthetic --
/// it was never issued, and no live verification would confirm it.
fn openai_shaped_key() -> String {
    format!("sk{}{}", "-", base62(48))
}

/// An Anthropic-shaped key. Synthetic, and long enough behind the prefix to
/// clear the rule's floor -- which every redaction fixture already in this
/// repository sits below, deliberately.
fn anthropic_shaped_key() -> String {
    format!("sk{}ant{}api03-{}", "-", "-", base62(40))
}

fn pem_header() -> String {
    format!("-----BEGIN {} PRIVATE KEY-----", "RSA")
}

fn aws_shaped_key() -> String {
    format!("AKIA{}", "IOSFODNN7EXAMPLQ")
}

fn github_shaped_pat() -> String {
    format!("ghp{}{}", "_", base62(36))
}

fn added(line: &str) -> String {
    format!("+{line}\n")
}

fn secret_finding(line: &str) -> Option<String> {
    match PreMergeScanner::scan_for_secrets(&added(line)) {
        GateStatus::Passed => None,
        GateStatus::Failed(m) => Some(m),
        other => panic!("secret scan produced an unexpected status: {other:?}"),
    }
}

fn cleartext_findings(diff: &str) -> usize {
    ZeroTrustWorkloadGate::new()
        .evaluate_cleartext_transport(diff)
        .cleartext_transport_findings
}

// ---------------------------------------------------------------------------
// 1. The loose secret rules must stop accusing ordinary code
// ---------------------------------------------------------------------------

/// Catches: the exact identifier the defect was verified on. `sk-` sits inside
/// `task-`, and a hyphen-admitting character class swallows the rest.
#[test]
fn a_kebab_case_identifier_containing_sk_dash_is_not_a_credential() {
    assert_eq!(
        secret_finding("    task-scheduler-backpressure-limit: 32"),
        None,
        "`task-scheduler-backpressure-limit` is a config key, not an API key"
    );
}

/// Catches: narrowing that special-cases the one reported string. Every English
/// word ending in `sk` puts `sk-` in front of a kebab-case tail.
#[test]
fn no_english_word_ending_in_sk_makes_the_rest_of_an_identifier_a_credential() {
    for line in [
        "  risk-assessment-window-tuning-parameters: 5",
        "  disk-cache-eviction-policy-generation-v2 = 1",
        "  desk-allocation-service-rebalancer-config: true",
        "  mask-sensitive-response-headers-middleware: on",
        "  let brisk_retry = \"brisk-backoff-strategy-exponential-jitter\";",
    ] {
        assert_eq!(secret_finding(line), None, "false positive on: {line}");
    }

    // A kebab-case tail is only half the class. A word ending in `sk` followed
    // by a *digest* -- a content-hashed asset path, a digest-suffixed cache key
    // -- clears every false-positive filter the rule has: high entropy, not
    // purely alphabetic, no stopword. Only requiring the candidate to open at a
    // non-word boundary rejects these, which is why the fixtures live in the
    // test named for not special-casing the one reported string.
    for line in [
        format!(
            "  <script src=\"/assets/di{}k-{}.js\"></script>",
            "s",
            base62(26)
        ),
        format!("  let cache_key = \"ta{}k-{}\";", "s", base62(32)),
    ] {
        assert_eq!(secret_finding(&line), None, "false positive on: {line}");
    }
}

/// Catches: the redaction placeholder that the password rule fired on. A
/// redacted value is the *opposite* of a leak -- accusing it teaches authors
/// that redacting is what trips the gate.
#[test]
fn a_redaction_placeholder_is_not_a_hardcoded_password() {
    assert_eq!(secret_finding("        password: \"<redacted>\""), None);
}

/// Catches: the other placeholder shapes. An env-var reference and a template
/// expansion are indirection, which is exactly the fix the gate wants.
#[test]
fn a_placeholder_or_env_reference_is_not_a_hardcoded_password() {
    for line in [
        "  password = \"${DB_PASSWORD}\"",
        "  password: \"{{ vault_db_password }}\"",
        "  password: \"********\"",
        "  password = \"changeme\"",
        "  password: \"your-password-here\"",
        "  # password: \"changeme\"",
        "  // password: \"example\"",
    ] {
        assert_eq!(secret_finding(line), None, "false positive on: {line}");
    }
}

/// Catches: any one of the four false-positive filters being deleted as
/// redundant. Mutation testing found the first draft of this suite covered them
/// only as a pile -- every fixture was rejected by two or three at once, so
/// removing any single filter left the suite green. Each case below is rejected
/// by exactly one of them and passes the other three.
#[test]
fn each_false_positive_filter_carries_a_case_only_it_rejects() {
    for (filter, line) in [
        // Template syntax. High entropy, no stopword, not purely alphabetic.
        (
            "placeholder syntax",
            "  password = \"{{ vault_db_x9K2mQ }}\"",
        ),
        // Stopword. High entropy, no template syntax, not purely alphabetic.
        ("stopword", "  password = \"changeme7413926\""),
        // Entropy floor. No stopword, no template syntax, not purely alphabetic.
        ("entropy floor", "  password = \"aaaa1111\""),
        // Alphabetic allowlist. Entropy 4.087 over 17 distinct characters, no
        // stopword, no template syntax.
        ("alphabetic allowlist", "  password = \"jklmnopqrstuvwxyz\""),
    ] {
        assert_eq!(
            secret_finding(line),
            None,
            "the {filter} filter is the only thing rejecting this, and it did not: {line}"
        );
    }
}

/// Catches: removal of the allowlist that rejects a purely alphabetic
/// candidate. A long lowercase word run behind the prefix clears the length
/// floor and scores log2(26) = 4.7 on entropy, so entropy does not reject it
/// and no stopword covers it -- the allowlist is the only thing that does.
/// A key of 48 base62 characters containing no digit at all has probability
/// under 1 in 3000, so this costs essentially no true positives.
#[test]
fn a_run_of_letters_behind_the_prefix_is_a_word_not_a_key() {
    let line = format!("  let x = \"sk{}{}\";", "-", "abcdefghijklmnopqrstuvwxyz");
    assert_eq!(secret_finding(&line), None, "false positive on: {line}");
}

/// Catches: the false positives this repository was already carrying. Its
/// account-pool redaction fixtures are obvious stand-ins -- and every one of
/// them blocked a merge under the old `sk-` rule.
#[test]
fn this_repositorys_own_redaction_fixtures_are_not_credentials() {
    let prefix = format!("sk{}ant{}", "-", "-");
    for tail in [
        "api03-SUPERSECRETKEYVALUE",
        "oat01-SUPERSECRETTOKENVALUE",
        "oat01-test-token-123",
        "oat01-gamma-token",
        "oat01-LEAKME",
    ] {
        let line = format!("  oauth_token: Some(\"{prefix}{tail}\".to_string()),");
        assert_eq!(secret_finding(&line), None, "false positive on: {line}");
    }
}

// ---------------------------------------------------------------------------
// 2. ... while the rules that were always honest keep blocking
// ---------------------------------------------------------------------------

/// Catches: a "fix" that neuters the `sk-` rule instead of anchoring it. This
/// is the rule the whole change is about, so it is the rule most at risk of
/// being loosened into silence.
#[test]
fn an_openai_shaped_key_still_blocks_the_merge() {
    let finding = secret_finding(&format!("  let key = \"{}\";", openai_shaped_key()));
    assert!(
        finding.is_some(),
        "a 51-character sk- key must still fail the gate"
    );
}

/// Catches: collateral damage to the four precise rules, which were correct
/// before this change and must be byte-for-byte as strict after it.
#[test]
fn the_four_provider_anchored_rules_still_block_the_merge() {
    for needle in [
        pem_header(),
        aws_shaped_key(),
        github_shaped_pat(),
        github_shaped_pat().replace("ghp", "gho"),
    ] {
        assert!(
            secret_finding(&format!("  const C: &str = \"{needle}\";")).is_some(),
            "provider-anchored rule stopped blocking on: {needle}"
        );
    }
}

/// Catches: a password rule filtered into uselessness. A high-entropy literal
/// with no placeholder marker and no stopword is the case the rule exists for.
#[test]
fn a_real_looking_hardcoded_password_still_blocks_the_merge() {
    let line = format!("  password = \"{}\"", base62(20));
    assert!(
        secret_finding(&line).is_some(),
        "a high-entropy quoted password literal must still fail the gate"
    );
}

/// Catches: a gate that found nothing ever. Every rule in the table must be
/// individually reachable, so no rule can be quietly filtered to death while
/// the others carry the suite.
#[test]
fn every_secret_rule_is_individually_reachable() {
    let reachable: Vec<&str> = [
        ("PEM", format!("  k = \"{}\"", pem_header())),
        ("AWS", format!("  k = \"{}\"", aws_shaped_key())),
        ("ghp", format!("  k = \"{}\"", github_shaped_pat())),
        (
            "gho",
            format!("  k = \"{}\"", github_shaped_pat().replace("ghp", "gho")),
        ),
        ("sk-", format!("  k = \"{}\"", openai_shaped_key())),
        ("sk-ant-", format!("  k = \"{}\"", anthropic_shaped_key())),
        ("password", format!("  password = \"{}\"", base62(20))),
    ]
    .iter()
    .filter(|(_, line)| secret_finding(line).is_some())
    .map(|(name, _)| *name)
    .collect();

    assert_eq!(
        reachable.len(),
        7,
        "only these rules can still fire: {reachable:?} -- a rule that cannot \
         fire on its own provider's key shape is dead code claiming to be a \
         security control"
    );
}

/// Catches: a rule table that stopped distinguishing rules -- if every finding
/// carried the same description the reachability test above could be satisfied
/// by one over-broad rule.
#[test]
fn each_rule_reports_its_own_description() {
    let sk = secret_finding(&format!("  k = \"{}\"", openai_shaped_key())).expect("sk- fires");
    let pw = secret_finding(&format!("  password = \"{}\"", base62(20))).expect("password fires");
    assert_ne!(
        sk, pw,
        "two different rules must not report the same finding"
    );
}

/// Catches: scanning removed lines. A deleted credential is a credential being
/// removed; the gate has always been add-only and must stay so.
#[test]
fn a_removed_line_is_not_scanned() {
    let diff = format!("-  let key = \"{}\";\n", openai_shaped_key());
    assert!(matches!(
        PreMergeScanner::scan_for_secrets(&diff),
        GateStatus::Passed
    ));
}

/// Catches: the headline claim. "Deep entropy scan" was published on a scanner
/// with no entropy computation in it at all. This pins the real function, with
/// the two values every implementation of Shannon entropy must agree on.
#[test]
fn the_gate_actually_computes_shannon_entropy() {
    assert_eq!(PreMergeScanner::shannon_entropy(""), 0.0);
    assert_eq!(PreMergeScanner::shannon_entropy("aaaaaaaa"), 0.0);
    assert!((PreMergeScanner::shannon_entropy("abcd") - 2.0).abs() < 1e-9);
    assert!((PreMergeScanner::shannon_entropy("aabb") - 1.0).abs() < 1e-9);
    // The oracle's own worked example: gitleaks' blog scores this at 3.303.
    assert!((PreMergeScanner::shannon_entropy("extremelySecret123") - 3.303).abs() < 0.01);
    // And the identifier the defect fired on, which entropy alone cannot reject.
    assert!(PreMergeScanner::shannon_entropy("task-scheduler-backpressure-limit") > 3.5);
}

// ---------------------------------------------------------------------------
// 3. The cleartext check must stop accusing prose
// ---------------------------------------------------------------------------

/// Catches: the defect. A licence URL in a doc comment made the gate `Failed`.
/// This is how an honest lint gets deleted: the first engineer it blocks for a
/// comment removes it.
#[test]
fn a_documentation_url_in_a_comment_is_not_an_insecure_transport() {
    let url = format!("ht{}://www.apache.org/licenses/LICENSE-2.0", "tp");
    for prefix in ["/// See ", "// see ", "# see ", " * see ", "-- see "] {
        let diff = format!("+++ b/src/thing.rs\n{}\n", added(&format!("{prefix}{url}")));
        assert_eq!(cleartext_findings(&diff), 0, "false positive on: {prefix}");
    }
}

/// Catches: a comment stripper that treats `#` as a comment opener anywhere on
/// the line, and a leading `*` as a block-comment continuation regardless of
/// what follows it. A raw string is the ordinary way to write a URL in Rust,
/// `#` also opens a URL fragment, and `*x = ...` is a dereference -- so both
/// over-broad rules were one-character bypasses of the whole lint.
#[test]
fn a_raw_string_or_a_dereference_does_not_hide_a_cleartext_endpoint() {
    let scheme = format!("ht{}://", "tp");
    let h = "#";
    for line in [
        format!("    let url = r{h}\"{scheme}billing.internal:8080/charge\"{h};"),
        format!("    *endpoint = \"{scheme}billing.internal:8080\".into();"),
        format!("    let doc = \"{scheme}billing.internal/guide{h}intro\";"),
    ] {
        let diff = format!("+++ b/src/thing.rs\n{}", added(&line));
        assert_eq!(cleartext_findings(&diff), 1, "hidden from the lint: {line}");
    }
}

/// Catches: an opt-in check that stops at the identifier's first occurrence. A
/// line that names the call before making it cleared the check on the mention.
#[test]
fn an_opt_in_named_before_it_is_called_still_fires() {
    let diff = format!(
        "+++ b/src/thing.rs\n{}",
        added("    let f = allow_insecure; f(true); builder.allow_insecure(true);")
    );
    assert_eq!(cleartext_findings(&diff), 1);
}

/// Catches: scanning prose files. A markdown link is a link, and no comment
/// marker precedes it.
#[test]
fn a_link_in_a_markdown_file_is_not_an_insecure_transport() {
    let url = format!("ht{}://example.org/spec", "tp");
    for path in ["README.md", "docs/doctrine.md", "NOTES.txt", "guide.rst"] {
        let diff = format!(
            "+++ b/{path}\n{}",
            added(&format!("See [the spec]({url})."))
        );
        assert_eq!(cleartext_findings(&diff), 0, "false positive in: {path}");
    }
}

/// Catches: firing on a URL whose host is a format placeholder. The host is not
/// in the diff, so no claim about it can be made from the diff.
#[test]
fn a_url_whose_host_is_a_template_placeholder_is_not_a_finding() {
    let scheme = format!("ht{}://", "tp");
    for line in [
        format!("    info!(\"listening at {scheme}{{}}/webhook\", addr);"),
        format!("    let target = format!(\"{scheme}{{}}:{{}}/webhook\", host, port);"),
        format!("    let base = \"{scheme}$SERVICE_HOST/v1\";"),
        format!("    if s.starts_with(\"{scheme}\") {{ }}"),
    ] {
        let diff = format!("+++ b/src/thing.rs\n{}", added(&line));
        assert_eq!(cleartext_findings(&diff), 0, "false positive on: {line}");
    }
}

/// Catches: over-correction on loopback, which was already excluded and must
/// stay excluded -- a dev server on 127.0.0.1 is not an inter-service hop.
#[test]
fn a_loopback_url_is_not_a_finding() {
    let scheme = format!("ht{}://", "tp");
    for host in [
        "localhost:8080",
        "127.0.0.1:3000",
        "0.0.0.0:9090",
        "[::1]:80",
    ] {
        let diff = format!(
            "+++ b/src/thing.rs\n{}",
            added(&format!("    let c = Client::new(\"{scheme}{host}\");"))
        );
        assert_eq!(cleartext_findings(&diff), 0, "false positive on: {host}");
    }
}

/// Catches: a lint narrowed until it cannot fire. A concrete remote host in a
/// client construction is the case the check exists for, and it is the case the
/// repository's own red/green suite already pins.
#[test]
fn a_cleartext_client_to_a_concrete_remote_host_still_fires() {
    let scheme = format!("ht{}://", "tp");
    let diff = format!(
        "+++ b/src/billing.rs\n{}",
        added(&format!(
            "    let resp = client.get(\"{scheme}payment-service.internal:8080/charge\").send().await?;"
        ))
    );
    assert_eq!(
        cleartext_findings(&diff),
        1,
        "a cleartext call to a concrete remote host must still fire"
    );
}

/// Catches: a check that only knows about Rust. A YAML config value is the
/// other half of CWE-319 and carries no comment marker.
#[test]
fn a_cleartext_endpoint_in_a_config_value_still_fires() {
    let scheme = format!("ht{}://", "tp");
    let diff = format!(
        "+++ b/deploy/values.yaml\n{}",
        added(&format!(
            "  billing_endpoint: {scheme}billing.prod.svc:8080"
        ))
    );
    assert_eq!(cleartext_findings(&diff), 1);
}

/// Catches: the explicit insecure-transport opt-ins being dropped along with
/// the loose substring. These name a real code-level decision.
#[test]
fn an_explicit_insecure_transport_opt_in_still_fires() {
    for line in [
        "    let c = builder.allow_insecure(true).build()?;",
        "    let c = insecure_client();",
    ] {
        let diff = format!("+++ b/src/thing.rs\n{}", added(line));
        assert_eq!(cleartext_findings(&diff), 1, "stopped firing on: {line}");
    }
}

/// Catches: a gate that abstains by construction. With no `+++` header the
/// check has no path to scope on, and it must fail closed rather than pass.
#[test]
fn a_diff_with_no_file_header_is_still_scanned() {
    let scheme = format!("ht{}://", "tp");
    let diff = added(&format!(
        "    let c = Client::new(\"{scheme}payments.internal:8080\");"
    ));
    assert_eq!(cleartext_findings(&diff), 1);
}

// ---------------------------------------------------------------------------
// 4. The evidence that decides whether either gate is shippable
// ---------------------------------------------------------------------------

/// The last `n` commits as `(sha, diff)`, or a panic.
///
/// It fails closed on a checkout that cannot supply the corpus, because the
/// alternative is silently measuring a different one and reporting it under
/// this name. `actions/checkout` clones at depth 1 by default; in such a clone
/// `git log -20` returns the single grafted commit, and `git show` on a graft
/// emits the entire tree as one all-additions diff -- 90k lines, which is both
/// a different corpus and eight minutes of scanning. `.github/workflows/ci.yml`
/// therefore sets `fetch-depth: 0` on the job that runs this test.
fn recent_commit_diffs(n: usize) -> Vec<(String, String)> {
    let shallow = Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .output()
        .expect("git rev-parse runs inside the repository");
    assert_eq!(
        String::from_utf8_lossy(&shallow.stdout).trim(),
        "false",
        "shallow checkout: this repository's history is not present, so this \
         corpus cannot be measured. Set `fetch-depth: 0` on the checkout step \
         in .github/workflows/ci.yml."
    );

    let log = Command::new("git")
        .args(["log", &format!("-{n}"), "--format=%H"])
        .output()
        .expect("git log runs inside the repository");
    let diffs: Vec<(String, String)> = String::from_utf8_lossy(&log.stdout)
        .lines()
        .map(|sha| {
            let show = Command::new("git")
                .args(["show", "--format=", "--unified=0", sha])
                .output()
                .expect("git show runs");
            (
                sha.to_string(),
                String::from_utf8_lossy(&show.stdout).to_string(),
            )
        })
        .collect();
    assert_eq!(
        diffs.len(),
        n,
        "asked for {n} commits and got {}: the corpus this test names is not \
         the corpus it measured",
        diffs.len()
    );
    diffs
}

/// Catches: the failure mode that kills honest checks. A gate that blocks on
/// this repository's own committed code is a gate that gets deleted by the
/// first engineer it inconveniences, so its false-positive rate against real
/// history is a shipping criterion, not a nice-to-have.
///
/// Before this change, `sk-`, the password rule and `contains("http://")` all
/// had live false positives in this tree; the counts are in the pull request.
#[test]
fn neither_gate_fires_on_this_repositorys_own_history() {
    let gate = ZeroTrustWorkloadGate::new();
    let mut findings: Vec<String> = Vec::new();

    for (sha, diff) in recent_commit_diffs(20) {
        if let GateStatus::Failed(whole) = PreMergeScanner::scan_for_secrets(&diff) {
            // Re-scan line by line only once something has already fired, so
            // the common case costs one regex compilation per rule per commit.
            // The message reported is the *line's* own rule: interpolating the
            // whole-diff message here labelled every finding with whichever
            // rule happened to fire first across the commit.
            let before = findings.len();
            for line in diff.lines() {
                if let GateStatus::Failed(per_line) =
                    PreMergeScanner::scan_for_secrets(&format!("{line}\n"))
                {
                    findings.push(format!("{} secret: {per_line} :: {line}", &sha[..8]));
                }
            }
            // A whole-diff finding that no single line reproduces is still a
            // finding. Reporting nothing here would make the commit invisible.
            if findings.len() == before {
                findings.push(format!(
                    "{} secret: {whole} :: (whole diff; no single line reproduces it)",
                    &sha[..8]
                ));
            }
        }
        for v in gate.evaluate_cleartext_transport(&diff).violations {
            findings.push(format!("{} cleartext: {v}", &sha[..8]));
        }
    }

    assert!(
        findings.is_empty(),
        "{} finding(s) against this repository's own last 20 commits:\n  {}",
        findings.len(),
        findings.join("\n  ")
    );
}
