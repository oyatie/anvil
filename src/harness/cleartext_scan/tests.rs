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

#[test]
fn reserved_invalid_dns_hosts_are_not_concrete_endpoints() {
    for authority in [
        "invalid",
        "proxy.invalid",
        "Proxy.INVALID.",
        "a-b.invalid:8080/path",
    ] {
        let diff = format!("+ endpoint = \"{}{authority}\"", scheme());
        assert!(
            IdentityAuditor::new()
                .audit_cleartext_transport(&diff)
                .is_empty(),
            "{authority}"
        );
    }
}

#[test]
fn reserved_invalid_exception_requires_a_complete_dns_authority() {
    for authority in [
        "invalid.example",
        "notinvalid",
        "invalid..",
        "bad-.invalid",
        "name.invalid:service",
        "name.invalid_suffix",
        "name.invalid:99999",
        "example.test",
        "example.internal",
        "example.com",
    ] {
        let diff = format!("+ endpoint = \"{}{authority}\"", scheme());
        assert_eq!(
            IdentityAuditor::new()
                .audit_cleartext_transport(&diff)
                .len(),
            1,
            "{authority}"
        );
    }
    let diff = format!(
        "+ endpoint = \"{}proxy.invalid\"; allow_insecure(true)",
        scheme()
    );
    assert_eq!(
        IdentityAuditor::new()
            .audit_cleartext_transport(&diff)
            .len(),
        1
    );
}

#[test]
fn reserved_invalid_exception_supports_only_complete_plain_literal_values() {
    let url = format!("{}proxy.invalid", scheme());
    for template in [
        "\"URL\"",
        "endpoint = \"URL\"",
        "let endpoint = \"URL\";",
        "let café = \"URL\";",
        "(\"HTTPS_PROXY\".to_owned(), \"URL\".to_owned()),",
        "Some(\"URL\")",
        "endpoint = \"URL\"; // ordinary trailing comment",
    ] {
        let diff = format!("+ {}", template.replace("URL", &url));
        assert!(
            IdentityAuditor::new()
                .audit_cleartext_transport(&diff)
                .is_empty(),
            "complete supported value: {template}"
        );
    }
}

#[test]
fn incomplete_or_unsupported_value_boundaries_keep_the_finding() {
    let url = format!("{}proxy.invalid", scheme());
    for template in [
        "endpoint = \"URL",
        "endpoint = \"URL note\"",
        "endpoint = \"URL' note\"",
        "endpoint = 'URL'",
        "endpoint = r\"URL\"",
        "endpoint = r#\"URL\"#",
        "endpoint = \"URL\" + suffix",
        "endpoint = \"URL\" \"suffix\"",
        "endpoint = \"URL # note\"",
        "endpoint = \"URL /* note",
        "endpoint = \"URL//note",
        "endpoint = wrapper(\"URL\")",
    ] {
        let diff = format!("+ {}", template.replace("URL", &url));
        assert_eq!(
            IdentityAuditor::new()
                .audit_cleartext_transport(&diff)
                .len(),
            1,
            "unsupported or incomplete value: {template}"
        );
    }
}

#[test]
fn reserved_value_proof_is_bound_to_the_scanned_literal_offset() {
    let url = format!("{}proxy.invalid", scheme());
    for first in [
        format!("{}service.internal", scheme()),
        format!("{url} ordinary text"),
        format!("ordinary text {url}"),
    ] {
        let diff = format!("+ (\"{first}\", \"{url}\")");
        assert_eq!(
            IdentityAuditor::new()
                .audit_cleartext_transport(&diff)
                .len(),
            1,
            "another complete literal cannot authorize this occurrence"
        );
    }
}

#[test]
fn complete_standalone_literal_opt_in_text_is_data_not_a_call() {
    for line in [
        "+ \"allow_insecure(true)\"",
        "+ \"allow_insecure(true)\",",
        "+ \"fixture \\\"quoted\\\" insecure_client(true)\",",
    ] {
        assert!(
            IdentityAuditor::new()
                .audit_cleartext_transport(line)
                .is_empty()
        );
    }
}

#[test]
fn actual_calls_and_unsupported_literal_forms_keep_the_explicit_opt_in_reason() {
    for line in [
        "+ allow_insecure(true);",
        "+ insecure_client(true);",
        "+ \"allow_insecure(true)",
        "+ endpoint = \"allow_insecure(true)\";",
        "+ wrapper(\"allow_insecure(true)\")",
        "+ \"allow_insecure(true)\", extra",
    ] {
        let findings = IdentityAuditor::new().audit_cleartext_transport(line);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].starts_with("Explicit insecure-transport opt-in:"));
    }
    let endpoint = format!("+ \"{}service.internal\"", scheme());
    assert!(
        IdentityAuditor::new().audit_cleartext_transport(&endpoint)[0]
            .starts_with("Cleartext http endpoint")
    );
}
