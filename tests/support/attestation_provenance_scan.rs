//! Pure matching shared by repository scans and their inert regression cases.
use anvil::source_scan::without_commentary;

// Only the shared builder and lane staging may spell their own whole-tree
// arguments. Moving this list does not grant a third owner an exemption.
const MAY_SPELL_THEIR_OWN_STAGING: &[&str] = &[
    "src/git_manager/mod.rs",
    "src/change_delivery/adapters/git_vcs.rs",
];

pub fn fabricated_claims(source: &str) -> Vec<&'static str> {
    let source = without_commentary(source);
    ["is_attested", "Cryptographic lane receipt"]
        .into_iter()
        .filter(|needle| source.contains(needle))
        .collect()
}

pub fn staging_violation(owner: &str, source: &str) -> Option<&'static str> {
    let source = without_commentary(source);
    if !source.contains("\"-A\"") {
        None
    } else if !MAY_SPELL_THEIR_OWN_STAGING.contains(&owner) {
        Some("stages a whole tree without going through git_manager::stage_excluding_receipts")
    } else if !source.contains(":(exclude)") {
        Some("spells its own `git add -A` with no exclusion")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{fabricated_claims, staging_violation};

    #[test]
    fn a_literal_whole_tree_argument_is_detected_outside_the_two_owners() {
        assert!(staging_violation("src/other.rs", "cmd.args([\"add\", \"-A\"]);").is_some());
    }

    #[test]
    fn each_allowed_owner_still_requires_the_exclusion() {
        for owner in [
            "src/git_manager/mod.rs",
            "src/change_delivery/adapters/git_vcs.rs",
        ] {
            assert_eq!(
                staging_violation(owner, "cmd.args([\"add\", \"-A\"]);"),
                Some("spells its own `git add -A` with no exclusion")
            );
            assert_eq!(
                staging_violation(
                    owner,
                    "cmd.args([\"add\", \"-A\", \":(exclude).anvil/receipts\"]);"
                ),
                None
            );
        }
    }

    #[test]
    fn the_fabricated_summary_literal_and_field_are_both_detected() {
        assert_eq!(
            fabricated_claims("let summary = \"Cryptographic lane receipt\";"),
            ["Cryptographic lane receipt"]
        );
        assert_eq!(
            fabricated_claims("report.is_attested = true;"),
            ["is_attested"]
        );
    }

    #[test]
    fn commentary_and_unrelated_arguments_are_not_findings() {
        let comments =
            "// cmd.args([\"add\", \"-A\"]);\n/* is_attested; \"Cryptographic lane receipt\" */";
        assert_eq!(staging_violation("src/other.rs", comments), None);
        assert!(fabricated_claims(comments).is_empty());
        assert_eq!(
            staging_violation("src/other.rs", "cmd.args([\"add\", \"file.rs\"]);"),
            None
        );
        assert!(fabricated_claims("let summary = \"Lane receipt recorded\";").is_empty());
    }
}
