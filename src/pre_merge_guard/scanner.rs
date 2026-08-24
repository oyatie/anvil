use super::GateStatus;
use regex::Regex;

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

pub struct PreMergeScanner;

impl PreMergeScanner {
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
    fn is_credential_shaped(candidate: &str, min_entropy: f64) -> bool {
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
        Self::shannon_entropy(candidate) > min_entropy
    }

    pub fn scan_for_secrets(diff: &str) -> GateStatus {
        for rule in SECRET_RULES {
            let Ok(re) = Regex::new(rule.pattern) else {
                continue;
            };
            for line in diff.lines() {
                if !line.starts_with('+') || line.starts_with("+++") {
                    continue;
                }
                for caps in re.captures_iter(line) {
                    let candidate = caps.get(1).map_or("", |m| m.as_str());
                    if rule.min_entropy > 0.0
                        && !Self::is_credential_shaped(candidate, rule.min_entropy)
                    {
                        continue;
                    }
                    return GateStatus::Failed(format!("Potential credential leak: {}", rule.desc));
                }
            }
        }

        GateStatus::Passed
    }

    pub fn scan_for_breaking_changes(diff: &str, changed_files: &[String]) -> GateStatus {
        let has_migration = changed_files
            .iter()
            .any(|f| f.contains("migration") || f.ends_with(".sql"));

        if has_migration {
            let destructive_patterns = [
                r"(?i)DROP\s+COLUMN",
                r"(?i)DROP\s+TABLE",
                r"(?i)ALTER\s+COLUMN.*NOT\s+NULL",
            ];

            for pattern in destructive_patterns {
                if let Ok(re) = Regex::new(pattern) {
                    for line in diff.lines() {
                        if line.starts_with('+') && !line.starts_with("+++") && re.is_match(line) {
                            return GateStatus::Warning(
                                    "Destructive schema migration detected (DROP/NOT NULL without multi-phase rollout). Verify backwards compatibility across cell nodes.".to_string(),
                                );
                        }
                    }
                }
            }
        }

        GateStatus::Passed
    }

    pub fn scan_for_concurrency_and_flakes(diff: &str) -> GateStatus {
        let flake_patterns = [
            (
                r"(?i)thread::sleep\s*\(\s*Duration::from_millis\s*\(\s*\d+\s*\)\s*\)",
                "Hardcoded real-clock test sleep (risk of test lane flake)",
            ),
            (
                r"(?i)time\.Sleep\s*\(\s*\d+\s*\*\s*time\.Millisecond\s*\)",
                "Hardcoded real-clock test sleep",
            ),
        ];

        for (pattern, desc) in flake_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") && re.is_match(line) {
                        return GateStatus::Warning(format!(
                            "Concurrency/Timing Warning: {}",
                            desc
                        ));
                    }
                }
            }
        }

        GateStatus::Passed
    }
}
