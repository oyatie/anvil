//! What a model turn is allowed to carry in from the daemon.
//!
//! Its own module because it is policy, not mechanism: `agent.rs` decides how a
//! turn is spawned, and this decides what it holds while it runs. Both halves
//! of the line below are asserted, so neither can be quietly widened nor
//! quietly starved.

/// Environment variables a model turn is given.
///
/// The list is what a provider CLI needs to start, find its configuration, and
/// reach the network -- and nothing else. It is deliberately short: adding a
/// name here hands it to a process acting on attacker-influenced instructions,
/// so each addition is a decision rather than an accident of what the daemon
/// happened to be started with.
///
/// The line it draws is between the credential the turn *is* and the authority
/// Anvil *has*. A model turn must be able to authenticate to its own provider,
/// so the provider's key or token is on the list; holding it is not an
/// escalation, because the turn is already that session. `GITHUB_TOKEN`,
/// `GITHUB_WEBHOOK_SECRET` and `SSH_AUTH_SOCK` are different in kind: they are
/// how Anvil speaks as itself to the forge and how it signs and pushes. A turn
/// that reads an attacker's diff and holds those can open, approve and merge on
/// Anvil's behalf, which is the escalation this list exists to refuse.
pub const INHERITED: &[&str] = &[
    // Enough of a process to run at all.
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "TERM",
    "TZ",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    // The provider CLIs' own configuration and credentials. Set per lease by
    // the account pool where a lease succeeds, and inherited here so that a
    // turn whose lease failed still authenticates as the daemon rather than
    // failing over for a reason that looks like the model being down.
    "ANTIGRAVITY_CONFIG_DIR",
    "ANTIGRAVITY_AUTH_TOKEN",
    "GEMINI_CLI_CONFIG_DIR",
    "GEMINI_API_KEY",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "CODEX_HOME",
    "CODEX_AUTH_TOKEN",
    "OPENAI_AUTH_TOKEN",
    "OPENAI_API_KEY",
    "CURSOR_CONFIG_DIR",
    "CURSOR_AUTH_TOKEN",
    "GROK_CONFIG_DIR",
    "GROK_AUTH_TOKEN",
    "XAI_API_KEY",
    // Without these, every provider call fails behind a corporate proxy, and
    // the failure looks like the model being unreachable.
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The list is an allowlist, so what is not on it is the assertion.
    #[test]
    fn the_inherited_list_names_no_forge_or_signing_credential() {
        for forbidden in [
            "GITHUB_WEBHOOK_SECRET",
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "GITHUB_APP_PRIVATE_KEY",
            "SSH_AUTH_SOCK",
            "AWS_SECRET_ACCESS_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
        ] {
            assert!(
                !INHERITED.contains(&forbidden),
                "{forbidden} is authority Anvil holds over the forge or over \
                 signing, not a credential the model turn is. A turn acting on \
                 an attacker's diff must not be able to open, approve or merge \
                 on Anvil's behalf."
            );
        }
    }

    /// And the other half: a turn that cannot authenticate to its own provider
    /// fails over for a reason that looks like the model being down.
    #[test]
    fn the_inherited_list_carries_each_providers_own_credential() {
        for needed in [
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "GEMINI_API_KEY",
            "ANTIGRAVITY_AUTH_TOKEN",
            "OPENAI_API_KEY",
            "CURSOR_AUTH_TOKEN",
            "XAI_API_KEY",
        ] {
            assert!(
                INHERITED.contains(&needed),
                "{needed} is how a model turn authenticates as itself; without \
                 it the turn fails and the failure reads as an outage"
            );
        }
    }
}
