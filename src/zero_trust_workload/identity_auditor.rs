//! A cleartext-transport lint over the diff (CWE-319), and nothing more.
//!
//! # What this is not
//!
//! It was published as "Cryptographic SPIFFE ID workload attestation & mTLS
//! encryption" while being `line.contains("http://")`. No source-text predicate
//! can support that claim, and narrowing this one does not bring it any closer:
//!
//!   - a SPIFFE ID is a URI in an X.509-SVID's single URI SAN, issued by a
//!     SPIRE server after an agent interrogated the kernel for the caller's PID
//!     and ran attestor plugins against it. It exists at runtime, delivered
//!     over the Workload API on a unix socket, and appears in no diff.
//!   - mTLS enforcement is a property of a deployment -- an Istio
//!     `PeerAuthentication` in `STRICT` mode, an Envoy listener with an
//!     SDS-backed TLS transport socket -- verified by querying a cluster.
//!
//! Neither direction of inference holds: a repository with no cleartext URL can
//! be running a mesh in PERMISSIVE mode with no workload identity at all, and a
//! repository full of cleartext doc links can be running STRICT mTLS over
//! SPIRE-issued SVIDs. The gate now says what it does.
//!
//! # Where it sits relative to real lints of this class
//!
//! gosec `G107`, semgrep's `insecure-transport` pack and Android's
//! `DefaultCleartextTraffic` are the honest comparison. All three are
//! sink-anchored over a parsed syntax tree: the finding is "this URL reaches an
//! HTTP client", not "this string exists", which is why a comment cannot match
//! -- a comment is not an AST node. gosec is explicit that a constant literal
//! URL produces no G107 warning at all.
//!
//! This is a line scanner with no parser, so it approximates the property from
//! the outside: it drops the line's comment tail, skips prose and test files,
//! and requires the URL to name a concrete non-loopback host. That removes the
//! false-positive classes that made the gate unshippable. It does not make the
//! check sink-anchored, and a cleartext URL built from parts across lines, or
//! read from configuration this diff does not touch, is invisible to it.

/// File extensions whose contents are prose. A link in a document is a link.
const PROSE_EXTENSIONS: &[&str] = &["md", "markdown", "txt", "rst", "adoc"];

/// Path fragments that mark a file as fixtures rather than deployed code. A
/// cleartext URL in a test is a test input, not a client the fleet will run.
/// GitHub's own generic-secret scanning documents the same exclusion, skipping
/// paths containing `test`, `mock` or `spec`.
const FIXTURE_PATH_MARKERS: &[&str] = &[
    "tests/",
    "/test/",
    "_test.",
    "testdata/",
    "fixtures/",
    "mock",
];

/// Hosts that are the machine running the process. Excluded before this change
/// too, and still excluded: a loopback endpoint is not an inter-service hop.
const LOOPBACK_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0"];

const CLEARTEXT_SCHEME: &str = "http://";

#[derive(Clone, Debug, Default)]
pub struct IdentityAuditor;

impl IdentityAuditor {
    pub fn new() -> Self {
        Self
    }

    /// One finding per added line that introduces a cleartext endpoint or an
    /// explicit insecure-transport opt-in, in a file that is neither prose nor
    /// fixtures.
    ///
    /// A diff that has produced no `+++ b/` header yet is scanned: an unknown
    /// path fails closed, so a caller passing bare lines gets the check rather
    /// than silence.
    pub fn audit_cleartext_transport(&self, diff_content: &str) -> Vec<String> {
        let mut violations = Vec::new();
        let mut in_scope = true;

        for line in diff_content.lines() {
            if let Some(path) = line.strip_prefix("+++ b/") {
                in_scope = !path_is_out_of_scope(path);
                continue;
            }
            if !in_scope || !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            let code = code_before_comment(&line[1..]);
            if let Some(reason) = insecure_transport_in(code) {
                violations.push(format!("{}: {}", reason, code.trim()));
            }
        }

        violations
    }
}

/// True for prose and fixture paths, which carry URLs that are never dialled.
fn path_is_out_of_scope(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    PROSE_EXTENSIONS.contains(&ext) || FIXTURE_PATH_MARKERS.iter().any(|m| lower.contains(m))
}

/// The line with its comment tail removed.
///
/// Every comment opener here is also ordinary code somewhere, so each one is
/// recognised only in the position a comment actually starts in:
///
///   - `//` is the middle of every URL, so one preceded by `:` is left alone --
///     otherwise this function would truncate exactly the thing it exists to
///     expose.
///   - `#` opens a comment in YAML, TOML and shell, but it also delimits a Rust
///     raw string (`r#"..."#`) and a URL fragment (`/doc#frag`). Treating it as
///     an opener anywhere made `let url = r#"http://host"#;` invisible to the
///     lint -- a one-character bypass -- so it must start a token.
///   - a leading `*` continues a block comment (`* text`, `*/`), but it is also
///     a dereference: blanking the line on `*` alone hid `*endpoint = "..."`.
fn code_before_comment(line: &str) -> &str {
    let b = line.as_bytes();
    // A block-comment continuation and a SQL comment open the whole line. A
    // lone `-` must not: a YAML sequence item (`- url: ...`) starts with one.
    let trimmed = line.trim_start();
    let block_continuation = trimmed.strip_prefix('*').is_some_and(|rest| {
        rest.is_empty() || rest.starts_with('/') || rest.starts_with(char::is_whitespace)
    });
    if block_continuation || trimmed.starts_with("--") {
        return "";
    }
    for i in 0..b.len() {
        let is_comment_open = match b[i] {
            b'#' => i == 0 || b[i - 1].is_ascii_whitespace(),
            b'/' if i + 1 < b.len() => {
                (b[i + 1] == b'/' && (i == 0 || b[i - 1] != b':')) || b[i + 1] == b'*'
            }
            _ => false,
        };
        if is_comment_open {
            return &line[..i];
        }
    }
    line
}

/// Why this line is a finding, or `None`.
fn insecure_transport_in(code: &str) -> Option<&'static str> {
    let mut rest = code;
    while let Some(i) = rest.find(CLEARTEXT_SCHEME) {
        let after = &rest[i + CLEARTEXT_SCHEME.len()..];
        let host: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .collect();
        // A host that does not start with an alphanumeric is not a host: the
        // URL's authority is a `{}` format placeholder, a `$VAR`, or the string
        // ends at the scheme. Nothing in the diff says where it points.
        let concrete = host
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        if concrete && !LOOPBACK_HOSTS.contains(&host.as_str()) {
            return Some("Cleartext http endpoint (CWE-319)");
        }
        rest = after;
    }

    // An explicit opt-out of transport security, named as a call. Requiring the
    // open paren is what keeps this file -- which names both identifiers as
    // string data below -- from being a finding against itself.
    // Every occurrence, not only the first: a line that names the identifier
    // before calling it (`let f = allow_insecure; f(true)` and the like) would
    // otherwise clear the check on its first, uncalled mention.
    for opt_in in ["allow_insecure", "insecure_client"] {
        if code
            .match_indices(opt_in)
            .any(|(i, _)| code[i + opt_in.len()..].starts_with('('))
        {
            return Some("Explicit insecure-transport opt-in");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assembled at runtime so this file is not a finding against itself; see
    /// the same note in `tests/loose_blocking_patterns_test.rs`.
    fn scheme() -> String {
        format!("ht{}://", "tp")
    }

    #[test]
    fn test_detects_insecure_remote_http() {
        let auditor = IdentityAuditor::new();
        let diff = format!(
            "+ let client = HttpClient::connect(\"{}billing.internal:8080\");",
            scheme()
        );
        assert_eq!(auditor.audit_cleartext_transport(&diff).len(), 1);
    }

    #[test]
    fn test_passes_spiffe_mtls_transport() {
        let auditor = IdentityAuditor::new();
        let diff = "+ let client = SpiffeTlsClient::connect(\"https://billing.internal:8443\");";
        assert!(auditor.audit_cleartext_transport(diff).is_empty());
    }

    #[test]
    fn test_ignores_a_url_in_a_comment() {
        let auditor = IdentityAuditor::new();
        let diff = format!("+// see {}docs.internal/runbook", scheme());
        assert!(auditor.audit_cleartext_transport(&diff).is_empty());
    }
}
