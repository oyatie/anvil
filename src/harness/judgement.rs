//! The judgements the rules make, owned by the harness rather than borrowed
//! from the gates that are being deleted.
//!
//! The migration-boundary ratchet found this the moment the first two
//! changeset-rung rules were written: `harness` is `Migrating` and was reaching
//! into `pre_merge_guard/scanner` and `local_inner_loop/fast_validator`, both
//! `Superseded`. A rule anchored to code scheduled for deletion cannot migrate,
//! and the honest repair is not to reclassify the dependency -- it is to move
//! the durable half here and let the superseded wrappers call inward.
//!
//! Nothing below is new. The secret rules, the entropy floor, the placeholder
//! allowlist and the Conventional Commits grammar are moved verbatim, together
//! with the corrections each of them already carries. The gates keep their
//! public functions as one-line delegations, so every existing caller and test
//! is unaffected and the two regexes have exactly one home.

use crate::pre_merge_guard::report::GateStatus;
use regex::Regex;
use std::sync::LazyLock;

/// One secret rule, in the shape every production scanner converged on: a
/// structural pattern whose capture group 1 is the candidate secret, plus an
/// optional entropy floor applied *after* the structural match.
///
/// The order matters and is the whole correction. gitleaks evaluates
/// regex -> entropy -> allowlist; entropy is never the thing that decides a
/// finding, because it cannot: `task-scheduler-backpressure-limit` scores
/// 3.847, above gitleaks' strictest generic threshold of 3.5, and
/// `if err := readUntilSafeBoundary(reader, n, maxPeekSize, peekBuf); err != nil`
/// scores 4.24. What rejects an identifier is structure -- a provider prefix,
/// an assignment context, an allowlist -- with entropy as the last filter on an
/// already-constrained capture.
struct SecretRule {
    /// Capture group 1 is the candidate secret. Anything outside it is context
    /// used to find the candidate, not part of it.
    pattern: &'static str,
    desc: &'static str,
    /// `0.0` disables every post-filter below. A rule whose prefix is issued by
    /// exactly one provider is already conclusive, so filtering it can only
    /// lose true positives; 92 of gitleaks' 222 default rules set no entropy
    /// for the same reason.
    min_entropy: f64,
}

/// Substrings that mark a value as a stand-in rather than a credential,
/// matched case-insensitively anywhere in the candidate. This is the same
/// mechanism as gitleaks' stopword trie and TruffleHog's
/// `DefaultFalsePositives`, at a hundredth of the size: a short list of the
/// shapes that actually appear in this fleet's diffs.
///
/// Deliberately absent: `secret` and `test`. Both occur inside plausible real
/// passwords, and a stopword is a hard drop -- the false-negative cost is
/// higher than the false-positive cost they would remove.
const PLACEHOLDER_WORDS: &[&str] = &[
    "example",
    "redacted",
    "changeme",
    "placeholder",
    "password",
    "sample",
    "dummy",
    "your",
    "todo",
    "fixme",
    "insert",
];

/// Characters that only appear in a reference to a secret, never in the secret:
/// redaction brackets, shell and template expansion, printf-style formatting.
const PLACEHOLDER_SYNTAX: &[char] = &['<', '>', '{', '}', '$', '%'];

/// Seven rules: the four provider-anchored ones unchanged from the audit that
/// found this defect, one added, and the two loose ones narrowed. Only the last
/// three carry an entropy floor, because only those lack an anchor conclusive
/// on its own.
const SECRET_RULES: &[SecretRule] = &[
    SecretRule {
        pattern: r"(?i)(-----BEGIN[ A-Z0-9_-]*PRIVATE KEY-----)",
        desc: "Exposed Private Key block",
        min_entropy: 0.0,
    },
    SecretRule {
        pattern: r"(?i)(AKIA[0-9A-Z]{16})",
        desc: "AWS Access Key ID",
        min_entropy: 0.0,
    },
    SecretRule {
        pattern: r"(?i)(ghp_[A-Za-z0-9_]{36})",
        desc: "GitHub Personal Access Token",
        min_entropy: 0.0,
    },
    SecretRule {
        pattern: r"(?i)(gho_[A-Za-z0-9_]{36})",
        desc: "GitHub OAuth Token",
        min_entropy: 0.0,
    },
    // Was `(?i)sk-[A-Za-z0-9_-]{24,}`. Three independent narrowings, each of
    // which alone kills the `task-`/`risk-`/`disk-`/`desk-`/`mask-` class:
    //
    //  1. case-sensitive. The issued prefix is lowercase `sk-`; `(?i)` made
    //     every word ending in `SK` or `Sk` a match site too.
    //  2. the free-form branch admits no hyphen and no underscore, so an
    //     identifier's next hyphen ends the candidate long before 24
    //     characters. `task-scheduler-...` offers `scheduler`, nine.
    //  3. the prefixed branch requires one of OpenAI's three literal segment
    //     keywords before the hyphen-bearing body.
    //  4. the candidate must open at a non-word boundary. Without it a word
    //     ending in `sk` followed by a hex or base62 run -- a content-hashed
    //     asset path, a digest-suffixed cache key -- clears every filter:
    //     high entropy, not purely alphabetic, no stopword. gitleaks anchors
    //     the same way for the same reason.
    //
    // gitleaks additionally anchors on `T3BlbkFJ`, the base64 of `OpenAI`
    // embedded in every issued key. That is strictly better and is what the
    // registry records as the remaining gap: it is not adopted here because
    // this rule is the fleet's only `sk-` rule and other vendors issue `sk-`
    // keys without that marker.
    SecretRule {
        pattern: r"(?:^|[^A-Za-z0-9_])(sk-(?:proj|svcacct|admin)-[A-Za-z0-9_-]{20,}|sk-[A-Za-z0-9]{24,})",
        desc: "API Secret Key",
        min_entropy: 3.5,
    },
    // The one vendor prefix this repository demonstrably handles -- `types.rs`
    // in the account pool stores exactly these two token kinds -- and the only
    // rule here that is new rather than narrowed. The old `sk-` rule covered
    // this shape by accident, so removing its hyphen class would have dropped
    // the coverage silently; the length floor is set above the longest
    // redaction fixture already in this tree, all of which are under 24
    // characters behind the prefix.
    SecretRule {
        pattern: r"(sk-ant-(?:api|oat|admin)\d{2}-[A-Za-z0-9_-]{32,})",
        desc: "Anthropic API or OAuth Key",
        min_entropy: 3.5,
    },
    // Was `["'][^"']{6,}["']` with no filter at all, so `password: "<redacted>"`
    // -- a value that has already been scrubbed -- blocked the merge. Capture
    // group 1 is now the value alone rather than the whole assignment, which is
    // what makes the filters below meaningful.
    SecretRule {
        pattern: r#"(?i)password\s*[:=]\s*["']([^"']{8,})["']"#,
        desc: "Hardcoded plaintext password",
        min_entropy: 3.0,
    },
];

/// Shannon entropy of `s` in bits per character, over the characters `s`
/// actually contains: `H = -sum p(c) * log2 p(c)`.
///
/// This is the computation the gate's own scorecard line ("Deep entropy
/// scan for leaked credentials") claimed and did not perform -- before this
/// change the file contained no logarithm of any kind. It is the same
/// formula gitleaks and TruffleHog use, and it deliberately measures the
/// observed alphabet rather than a fixed one, so a 48-character base62 run
/// scores near log2(48) while a run of one repeated character scores 0.
pub fn shannon_entropy(s: &str) -> f64 {
    let mut counts: Vec<(char, usize)> = Vec::new();
    let mut len = 0usize;
    for c in s.chars() {
        len += 1;
        match counts.iter_mut().find(|(k, _)| *k == c) {
            Some((_, n)) => *n += 1,
            None => counts.push((c, 1)),
        }
    }
    if len == 0 {
        return 0.0;
    }
    let total = len as f64;
    counts
        .iter()
        .map(|(_, n)| {
            let p = *n as f64 / total;
            -p * p.log2()
        })
        .sum()
}

/// Whether a structurally matched candidate survives the false-positive
/// filters. Applied only to rules that set a `min_entropy`.
pub fn is_credential_shaped(candidate: &str, min_entropy: f64) -> bool {
    // An empty candidate needs no guard of its own: `all` is vacuously true
    // on it, so the allowlist below rejects it.
    //
    // gitleaks' generic allowlist, verbatim in effect: a value made only of
    // letters and identifier punctuation is an identifier. This -- not
    // entropy -- is what rejects `task-scheduler-backpressure-limit`.
    if candidate
        .chars()
        .all(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '.' | '-'))
    {
        return false;
    }
    if candidate.chars().any(|c| PLACEHOLDER_SYNTAX.contains(&c)) {
        return false;
    }
    let lower = candidate.to_ascii_lowercase();
    if PLACEHOLDER_WORDS.iter().any(|w| lower.contains(w)) {
        return false;
    }
    shannon_entropy(candidate) > min_entropy
}

/// The rule patterns, compiled once.
///
/// They used to be compiled inside the loop, once per rule per CALL. That is
/// the same defect `compliance_guard::engine` already recorded against itself,
/// and it is what made a whole-tree scan of 417 files take nine minutes rather
/// than seconds -- which in turn is why the self-scan could only afford to look
/// at the last twenty commits. A cost that forces a check to sample instead of
/// covering is not a performance problem alone; it decides what the check can see.
static COMPILED: LazyLock<Vec<(&'static SecretRule, Regex)>> = LazyLock::new(|| {
    SECRET_RULES
        .iter()
        .filter_map(|rule| Regex::new(rule.pattern).ok().map(|re| (rule, re)))
        .collect()
});

pub fn scan_for_secrets(diff: &str) -> GateStatus {
    for (rule, re) in COMPILED.iter() {
        for line in diff.lines() {
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            for caps in re.captures_iter(line) {
                let candidate = caps.get(1).map_or("", |m| m.as_str());
                if rule.min_entropy > 0.0 && !is_credential_shaped(candidate, rule.min_entropy) {
                    continue;
                }
                return GateStatus::Failed(format!("Potential credential leak: {}", rule.desc));
            }
        }
    }

    GateStatus::Passed
}

/// The Conventional Commits 1.0.0 header, with commitlint's
/// `@commitlint/config-conventional` type list.
///
/// The specification requires `<type>[(scope)][!]: <description>`: the colon and
/// the space after it are mandatory and the description may not be empty, so
/// `feat` and `feat:` are both invalid. The type list is commitlint's default
/// `type-enum`, which is convention rather than specification — the base spec
/// only gives `feat` and `fix` a defined meaning and permits others — and it is
/// matched case-sensitively, which is commitlint's `type-case: lower-case`.
///
/// One deliberate relaxation: more than one space after the colon is accepted.
/// The specification says one, and rejecting the second would be a red an author
/// cannot learn anything from.
///
/// One addition: `promote`. `type-enum` is configuration precisely because it is
/// per-project, and the base specification permits types beyond `feat` and
/// `fix`. This repository's promotion ladder writes `promote(dev): ...` and
/// `promote(staging): ...`; hardcoding commitlint's default and calling it the
/// grammar made the check red on the convention the project actually follows --
/// which is the same shape of invented vocabulary this module exists to delete.
const CONVENTIONAL_HEADER: &str =
    r"^(build|chore|ci|docs|feat|fix|perf|promote|refactor|revert|style|test)(\([^()]+\))?!?: +\S";

/// Subjects git writes rather than the author, taken from commitlint's own
/// `defaultIgnores`. Judging these is a false red nobody can fix: the text is
/// generated, and rewriting it means rewriting history.
const GENERATED_SUBJECT_PREFIXES: &[&str] = &[
    "Merge branch",
    "Merge pull request",
    "Merge remote-tracking branch",
    "Merge tag",
    "fixup!",
    "squash!",
    "amend!",
    "Revert \"",
];

/// A judged commit subject. Carries the message in both directions because the
/// gate reports the passing case too, and a verdict that can only explain
/// itself when it fails teaches an author nothing.
pub struct HeaderVerdict {
    pub valid: bool,
    pub message: String,
}

/// The Conventional Commits grammar, compiled once.
pub struct ConventionalHeader {
    re: Regex,
}

impl Default for ConventionalHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConventionalHeader {
    pub fn new() -> Self {
        Self {
            re: Regex::new(CONVENTIONAL_HEADER)
                .expect("the conventional-commit header pattern is a compile-time constant"),
        }
    }

    /// Judges one commit subject.
    ///
    /// `None` means the subject is one git generated and there is nothing to
    /// judge -- which is not the same as a subject that passed, and no caller
    /// may treat it as one. That distinction is the whole reason this returns
    /// an `Option` rather than a bool.
    pub fn judge(&self, commit_msg: &str) -> Option<HeaderVerdict> {
        let subject = commit_msg.lines().next().unwrap_or("").trim_end();
        if GENERATED_SUBJECT_PREFIXES
            .iter()
            .any(|p| subject.starts_with(p))
        {
            return None;
        }
        let valid = self.re.is_match(subject);
        Some(HeaderVerdict {
            valid,
            message: if valid {
                format!("`{subject}` is a valid conventional commit header.")
            } else {
                format!(
                    "`{subject}` is not a conventional commit header: Conventional Commits \
                     1.0.0 requires <type>[(scope)][!]: <description>, with the colon, the \
                     space and a non-empty description all present, and a type from \
                     build|chore|ci|docs|feat|fix|perf|promote|refactor|revert|style|test."
                )
            },
        })
    }
}
