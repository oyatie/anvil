use anvil::api_contract_guard::ApiContractGuard;
use anvil::attestation_guard::AttestationGuard;
use anvil::cedar_guard::CedarGuard;
use anvil::cell_isolation_guard::CellIsolationGuard;
use anvil::chaos_mutation_guard::ChaosMutationGuard;
use anvil::clean_architecture_guard::CleanArchitectureGuard;
use anvil::compliance_guard::ComplianceGuard;
use anvil::coverage_guard::CoverageGuard;
use anvil::criterion_bench_ratchet::CriterionBenchRatchet;
use anvil::debt_shrink_guard::DebtShrinkGuard;
use anvil::doc_guard::DocGuard;
use anvil::feature_flag_ratchet::FeatureFlagRatchet;
use anvil::ghost_migration_harness::GhostMigrationHarness;
use anvil::git_manager::PrDiffContext;
use anvil::kani_guard::KaniGuard;
use anvil::modularization_guard::ModularizationGuard;
use anvil::monorepo_guard::MonorepoGuard;
use anvil::pre_merge_guard::PreMergeGuard;
use anvil::rust_skills_guard::RustSkillsGuard;
use anvil::slo_canary_guard::SloCanaryGuard;
use anvil::supply_chain_guard::SupplyChainGuard;
use std::path::PathBuf;

#[tokio::test]
async fn test_adversarial_failure_modes_are_real_and_block_certification() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let repo_dir = temp_dir.path();

    let doc_guard = DocGuard::new("high".to_string());
    let cedar_guard = CedarGuard::new("high".to_string());
    let compliance_guard = ComplianceGuard::new();
    let api_guard = ApiContractGuard::new();
    let cell_guard = CellIsolationGuard::new();
    let supply_guard = SupplyChainGuard::new();
    let clean_arch_guard = CleanArchitectureGuard::new();
    let monorepo_guard = MonorepoGuard::new();
    let debt_guard = DebtShrinkGuard::new();
    let modular_guard = ModularizationGuard::new();
    let coverage_guard = CoverageGuard::new();
    let rust_skills_guard = RustSkillsGuard::new(repo_dir);
    let kani_guard = KaniGuard::new();
    let slo_guard = SloCanaryGuard::new();
    let ghost_guard = GhostMigrationHarness::new();
    let chaos_guard = ChaosMutationGuard::new();
    let flag_guard = FeatureFlagRatchet::new();
    let bench_guard = CriterionBenchRatchet::new();
    let attest_guard = AttestationGuard::new();
    let pre_merge = PreMergeGuard::new();

    // 1. Korean PIPA violation in diff
    let bad_pipa_diff = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 999,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: PathBuf::from("."),
        diff_content: "+ let resident_reg_num = \"900101-1234567\";".to_string(),
        changed_files: vec!["src/user.rs".to_string()],
        is_incremental: false,
    };

    let compliance_rep = compliance_guard.evaluate_compliance(&bad_pipa_diff).unwrap();
    assert!(!compliance_rep.is_compliant);

    // 2. Kani safety violation: undocumented unsafe block
    let bad_unsafe_diff = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 998,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: PathBuf::from("."),
        diff_content: "diff --git a/src/core.rs b/src/core.rs\n+ unsafe fn deref(p: *const u8) -> u8 { *p }".to_string(),
        changed_files: vec!["src/core.rs".to_string()],
        is_incremental: false,
    };

    let kani_rep = kani_guard.evaluate_unsafe_invariants(repo_dir, &bad_unsafe_diff).unwrap();
    assert!(!kani_rep.is_verified);
}
