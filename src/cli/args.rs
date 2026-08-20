use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "pr-watch")]
#[command(
    about = "Oyatie Autonomous Engineering Pipeline: Reviewer, Auto-Fixer, CI Triager, Queue Healer & Domain Guards"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the real-time webhook server and forwarders (default)
    Serve,
    /// Trigger an immediate manual review for a specific PR
    Review {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,

        #[arg(short, long, help = "Force re-review even if SHA was already reviewed")]
        force: bool,
    },
    /// Trigger resolution and auto-fixing for review comments on a PR
    Fix {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Pre-Merge Quality Certification and Domain Gates on a PR (21 gates)
    Certify {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Rust 2024 Edition Quality Guard (rust-skills 380 rules) on a PR
    RustSkillsCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Clean Architecture (Core/Ports/Adapters/Facade) Guard check on a PR
    ArchCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Hyperscaler Monorepo & Hermeticity Guard check on a PR
    MonorepoCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Deprecation & Reorg Drain Ratchet check on a PR
    DebtCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Code Modularization (100-300 lines max) check on a PR
    ModularCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Differential Test Coverage (>=85%) Guard check on a PR
    CoverageCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Ghost DB Migration & Zero-Lock Validator on a PR
    GhostMigrationCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run AST Chaos Mutation Test Adequacy Guard on a PR
    ChaosMutationCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Feature Flag Lifecycle & Dead Code Ratchet on a PR
    FeatureFlagCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Micro-Benchmark & Latency Regression Ratchet on a PR
    BenchCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Cedar IAM Policy Guard check on a PR
    CedarCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Dynamic Regulatory Compliance Guard check on a PR
    ComplianceCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run OpenAPI & Wire Contract Integrity Guard check on a PR
    ApiCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Cell Boundary & Tenant Isolation Guard check on a PR
    CellCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Run Supply Chain & CVE Audit Guard check on a PR
    SupplyChainCheck {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Generate and stamp a cryptographic lane receipt on a PR
    Attest {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Triage a failed CI workflow run on main/dev
    Triage {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Workflow Run ID")]
        run_id: u64,

        #[arg(short, long, help = "Branch name (default: main)")]
        branch: Option<String>,

        #[arg(short, long, help = "Commit SHA")]
        commit_sha: Option<String>,

        #[arg(short, long, help = "Workflow Name")]
        workflow_name: Option<String>,
    },
    /// Enlist a certified PR into the GitHub Merge Queue
    Enlist {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Heal a failed/ejected PR from the merge train
    HealQueue {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Auto-reconcile lockfiles and documentation ledgers on a PR branch
    Reconcile {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,
    },
    /// Install developer inner-loop git hooks (pre-commit & pre-push) in target or current repo
    HookInstall {
        #[arg(
            short,
            long,
            help = "Target repository path (default: current directory)"
        )]
        path: Option<String>,
    },
    /// Run instant sub-100ms local developer inner-loop pre-commit probe
    Probe {
        #[arg(
            short,
            long,
            help = "Optional diff content to validate (default: git diff staged)"
        )]
        diff: Option<String>,
    },
    /// Run Proactive Dependency Upgrade Train on a repository
    TrainRun {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,
    },
    /// Run Flaky-Test Quarantine 100x stress-run rehabilitation
    FlakeRehab {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,
    },
    /// Reap stale preview environments and abandoned git worktrees
    Reap,
    /// Autonomously reconcile trunk failure issues and auto-close recovered alerts
    IssueReconcile {
        #[arg(short, long, help = "Repository (e.g. oyatie/anvil or oyatie/oyatie)")]
        repo: String,
    },
    /// Autonomously heal deterministic CI failures (fmt, OWNERS, SHA pins) on a PR branch
    PrHeal {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Pull Request number")]
        pr: u64,

        #[arg(short, long, help = "PR branch name")]
        branch: String,
    },
    /// Autonomously sweep superseded ADRs, expired sprint plans, and demote unauthorized SSOT claims
    DocSweep {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Perform dry-run without modifying files")]
        dry_run: bool,
    },
    /// Evaluate a component's disposition (Move, Refactor, Rewrite, Retire, Evaluate)
    ComponentEval {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(
            short,
            long,
            help = "Target relative path in repository (e.g. oya/billing/crates/...)"
        )]
        target: String,
    },
    /// Run a 100% full-corpus audit across all files in the repository
    AuditCorpus {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(
            short,
            long,
            default_value_t = 180,
            help = "Dormant file stale threshold in days (default: 180)"
        )]
        stale_days: u64,
    },
    /// Autonomously generate a continuous hygiene maintenance batch PR
    HealCorpus {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(
            short,
            long,
            default_value_t = 10,
            help = "Maximum files to refresh per batch (default: 10)"
        )]
        batch_size: usize,

        #[arg(short, long, help = "Perform dry-run without modifying files")]
        dry_run: bool,
    },
    /// Autonomously audit open issues for resolution on trunk and contradiction against ADRs
    IssueAudit {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,
    },
    /// Consolidate and archive temporary documentation associated with an issue
    DocConsolidate {
        #[arg(short, long, help = "Repository (e.g. oyatie/oyatie)")]
        repo: String,

        #[arg(short, long, help = "Issue number")]
        issue: u64,

        #[arg(short, long, help = "Perform dry-run without modifying files")]
        dry_run: bool,
    },
    /// Execute Zero-Downtime Blue/Green Self-Replacement & Binary Handover
    Swap {
        #[arg(
            short,
            long,
            help = "Path to new green binary (default: target/release/anvil)"
        )]
        binary: Option<std::path::PathBuf>,
    },
    /// Run Full Outage Recovery & PR/Issue Reconciliation Sweep across watched repos
    Recover {
        #[arg(short, long, help = "Repository filter (optional)")]
        repo: Option<String>,
    },
    /// Run GitHub CLI webhook forwarding manually
    Forward,
    /// Verify GitHub CLI authentication and environment readiness
    Check,
}
