//! The one finite off-forge HTTP request Anvil can spawn.
//!
//! The fourth outbound seam, alongside `exec::agent`, [`crate::exec::build_env`]
//! and `exec::gh`. Those three bound a model turn, a build and a forge call;
//! this one bounds a transport -- `curl` reaching a public HTTP endpoint.
//!
//! # Why a seam
//!
//! `supply_chain_guard::osv_stream` POSTs to the OSV advisory database once per
//! certification. A bare `Command::new` hands that child the daemon's whole
//! environment: `GITHUB_WEBHOOK_SECRET`, `GH_TOKEN` and every model provider
//! key. That is the default rather than an oversight, so the next network call
//! written in that module carries them too.
//!
//! # Severity, stated honestly
//!
//! Lower than [`super::build_env`]. That seam exists because
//! `run_cargo_test_gate` runs a CONTRIBUTOR'S `#[test]` code, and a test can
//! read an environment variable -- so anyone who could open a pull request
//! could read the webhook secret. Nothing comparable holds here: the argv is
//! fixed by the caller and `curl` cannot be made to print its environment by a
//! payload. This is a consistency and blast-radius fix. Calling it an exposure
//! would be the overclaim this repository exists to refuse.
//!
//! # Why this is not `exec::gh`
//!
//! `gh` is Anvil talking to the forge as itself, so a forge credential is
//! exactly what belongs at that seam. A transport to a public advisory
//! database authenticates nobody, so `GH_TOKEN` is on the test's `NEVER_HANDED_OVER` list
//! here. The two lists disagree on purpose, and the test beside this pins the
//! disagreement in both directions.

use tokio::process::Command;

const CURL: &str = "curl";
const CURL_MAX_TIME: &str = "15";
const OSV_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);

/// What an outbound network tool is given.
///
/// Shorter than the other three lists because the subject is smaller: resolve a
/// name, open a TLS connection, write a body, read the answer. As with every
/// other seam this bounds what the daemon HANDS OVER; it is not a sandbox, and
/// that smaller claim is the only one the list supports.
const NET_INHERITED: &[&str] = &[
    // Without this the tool is not found at all and every request fails as a
    // spawn error rather than as the network result it never got to make.
    "PATH",
    // Where a transport spools a body too large to hold in memory.
    "TMPDIR",
    // Corporate egress. Without these the request never leaves the box, and the
    // failure reads as an outage at the far end rather than a proxy here.
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    // Both spellings, because libcurl honours `http_proxy` in lowercase ONLY --
    // it ignores the uppercase form so a CGI `Proxy:` header cannot set it.
    "https_proxy",
    "http_proxy",
    "no_proxy",
    // A corporate TLS interception root. curl and Go both read these, and
    // without them the failure surfaces as a certificate error rather than the
    // configuration error it is.
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Names that must never reach an outbound network tool, whatever else changes.
///
/// Asserted rather than assumed, for the reason `build_env` gives: the list
/// above is a list, and a name appended to it by hand is refused by the test
/// beside this one. `GH_TOKEN` and `GITHUB_TOKEN` are here and NOT on
/// [`super::gh::GH_INHERITED`]'s exclusions -- a forge credential belongs at
/// exactly one seam, and this is not it.
#[cfg(test)]
const NEVER_HANDED_OVER: &[&str] = &[
    "GITHUB_WEBHOOK_SECRET",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GITHUB_APP_PRIVATE_KEY",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
];

/// Constructs a fixed POST to OSV's batch endpoint.
///
/// No command, URL, method, header, argv, or budget crosses this module
/// boundary. Keeping those choices here prevents the generic network seam
/// from becoming a raw direct-model HTTP transport.
pub(super) async fn post_osv_batch(
    packages: &[crate::supply_chain_guard::LockedPackage],
) -> anyhow::Result<std::process::Output> {
    let cmd = command(packages);
    let command = super::NonModelCommand::checked_for(cmd, &[CURL])?;
    super::transport::run_for(command, OSV_BUDGET, "curl OSV querybatch", None).await
}

fn command(packages: &[crate::supply_chain_guard::LockedPackage]) -> Command {
    let payload =
        crate::supply_chain_guard::osv_stream::OsvAdvisoryStream::build_batch_payload(packages);
    let mut cmd = Command::new(CURL);
    apply(&mut cmd);
    cmd.args([
        // Must be argv[0]. This disables ~/.curlrc before curl reads it, so a
        // daemon HOME cannot replace the sealed URL or add a second upload.
        "-q",
        "-s",
        "-X",
        "POST",
        crate::supply_chain_guard::osv_stream::OSV_BATCH_URL,
        "-H",
        "Content-Type: application/json",
        "--max-time",
        CURL_MAX_TIME,
        "-w",
        "\n%{http_code}",
        "-d",
        &payload,
    ]);
    cmd
}

/// Applies the environment bound to a command the caller already holds.
fn apply(cmd: &mut Command) {
    apply_from(cmd, std::env::vars());
}

pub(super) fn apply_from<I>(cmd: &mut Command, environment: I)
where
    I: IntoIterator<Item = (String, String)>,
{
    // The canonical executable rebinder constructs a fresh Command. The
    // private marker is how it knows this environment was intentionally
    // cleared; a bare `env_clear` here would be lost during rebinding and the
    // canonical curl would inherit the daemon environment again.
    super::clear_environment(cmd);
    for (name, value) in environment {
        if NET_INHERITED.contains(&name.as_str()) {
            cmd.env(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn osv_command_has_a_fixed_destination_and_ignores_home_curl_config() {
        let packages = [crate::supply_chain_guard::LockedPackage {
            name: "tokio".to_owned(),
            version: "1.0.0".to_owned(),
        }];
        let command = command(&packages);
        let command = command.as_std();
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args.first().map(String::as_str), Some("-q"));
        assert_eq!(
            args.iter()
                .filter(|arg| arg.as_str() == crate::supply_chain_guard::osv_stream::OSV_BATCH_URL)
                .count(),
            1
        );
        assert_eq!(
            args.last().map(String::as_str),
            Some(
                r#"{"queries":[{"package":{"name":"tokio","ecosystem":"crates.io"},"version":"1.0.0"}]}"#
            )
        );

        let mut probe = Command::new("curl");
        apply_from(
            &mut probe,
            [
                ("HOME".to_owned(), "/tmp/hostile-curlrc-home".to_owned()),
                ("PATH".to_owned(), "/usr/bin".to_owned()),
                ("HTTPS_PROXY".to_owned(), "http://proxy.invalid".to_owned()),
                ("OPENAI_API_KEY".to_owned(), "secret".to_owned()),
            ],
        );
        let environment = probe
            .as_std()
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(environment.get("HOME"), None);
        assert_eq!(environment["PATH"].as_deref(), Some("/usr/bin"));
        assert_eq!(
            environment["HTTPS_PROXY"].as_deref(),
            Some("http://proxy.invalid")
        );
        assert_eq!(environment.get("OPENAI_API_KEY"), None);
        for forbidden in NEVER_HANDED_OVER {
            assert_eq!(environment.get(*forbidden), None);
        }
    }
}
