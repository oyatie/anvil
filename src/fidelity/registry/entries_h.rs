//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const CONSTANT_WORK_STATUS: GateFidelity = GateFidelity {
    gate_id: "constant_work_status",
    aspiration: "Work per unit of load does not grow with the size of the system: queues are \
                 bounded, pools are fixed, and a component does work proportional to its \
                 configuration rather than to what arrives.",
    reference: "AWS Builders' Library, constant-work systems; bounded queues and backpressure",
    fidelity: Fidelity::Heuristic,
    gap: "One regex is the entire gate: a line of added Rust matching \
          `mpsc::unbounded_channel` (constant_work_guard/buffer_limits.rs::scan_unbounded_structures). \
          No pool is sized, no capacity limit is read and no backpressure path is followed, so \
          nothing here measures work per unit of load or its constancy. The standard library's own \
          sender, and the unbounded constructors of the common channel crates, are spelled \
          differently and match nothing; an unbounded collection, a retry loop and a fan-out are \
          outside the rule entirely; and the pattern matches its text in a comment or a string as \
          readily as in code. `is_bounded` is the emptiness of `unbounded_findings`, so a change \
          with no Rust in it is a pass (constant_work_guard/mod.rs::evaluate_constant_work), and the \
          verdict blocks.",
    blocked_on: None,
};

pub const EPHEMERAL_SECRET_STATUS: GateFidelity = GateFidelity {
    gate_id: "ephemeral_secret_status",
    aspiration: "CI obtains its credentials by OIDC federation at job start and holds none that \
                 outlives the job, so a leaked credential expires before it can be used.",
    reference: "GitHub Actions OIDC federation with STS AssumeRoleWithWebIdentity; NIST SP 800-207 \
                zero trust",
    fidelity: Fidelity::Heuristic,
    gap: "One regex over the lines of files whose path contains `.github/workflows/`, matching an \
          `AWS_SECRET_ACCESS_KEY` assignment fed from the secrets context \
          (ephemeral_secrets/oidc_validator.rs::validate_workflow_secrets). No token lifetime is \
          read anywhere in this crate, so the fifteen-minute ceiling the label publishes has no \
          implementation and no value is compared against it. Federation is not checked either: \
          `id-token: write` appears only inside the advice string the finding carries, never as \
          something the workflow is examined for. One provider and one spelling are covered -- any \
          other cloud's static key, a key injected at job or step level under another name, and a \
          session token passed through are all clean here. `is_zero_trust` is the emptiness of \
          that list (ephemeral_secrets/mod.rs::evaluate_secret_policies), so a change that touches \
          no workflow at all publishes zero static secrets as a finding of fact, and the verdict \
          blocks.",
    blocked_on: None,
};

pub const GITOPS_PROMO_STATUS: GateFidelity = GateFidelity {
    gate_id: "gitops_promo_status",
    aspiration: "Every image a manifest deploys is named by an immutable digest that resolves in \
                 the registry, so promoting between environments moves a known artefact rather \
                 than whatever a tag points at today.",
    reference: "OCI image specification content-addressable digests; ArgoCD and Flux image \
                promotion; digest pinning over mutable tags",
    fidelity: Fidelity::Heuristic,
    gap: "Verifies no digest. No registry is contacted and no manifest is fetched, so the word \
          verified in the published label means only that sixty-four hex characters follow \
          `@sha256:` in the text; whether that digest exists, or names the image the tag named, is \
          unknown to this gate. `image_line_re` matches any line whose text carries an image key, \
          in any file the path filter admits -- anything under `gitops/`, `iac/`, `helm/` or `k8s/` \
          plus every `.yaml` and `.yml` -- so a template, a comment and a value in an unrelated \
          document are read alike, and no YAML is parsed \
          (gitops_promotion/digest_pinner.rs::scan_unpinned_images). An image string containing \
          `localhost` is exempt wherever it appears. The scan reads `after_change`, the whole hunk \
          rather than the added lines, so an unpinned image the change did not touch is charged to \
          it (gitops_promotion/mod.rs::evaluate_manifest_promotions), and the verdict blocks.",
    blocked_on: None,
};

pub const IDEMPOTENCY_STATUS: GateFidelity = GateFidelity {
    gate_id: "idempotency_status",
    aspiration: "Every mutating endpoint the change adds takes an idempotency key, records the \
                 result against it and replays rather than re-executes on a retry; state leaves \
                 the transaction through an outbox rather than a second write.",
    reference: "Stripe idempotent requests; the transactional outbox pattern",
    fidelity: Fidelity::Heuristic,
    gap: "The outbox half does not exist. Nothing in the engine named for it looks for a \
          transaction, an outbox table, a relay or a publish path \
          (idempotency_guard/outbox_rules.rs::scan_mutating_endpoints); the name is the whole of \
          that claim. What runs is `post_route_re`, one regex matching a single router spelling -- \
          a quoted path followed by a mutating method call -- on added Rust lines, so an endpoint \
          declared by any other framework, by attribute macro, or by a builder is not an endpoint \
          this gate can see. The excuse is `idempotency_header_re` matching anywhere in the \
          surrounding hunk, comments included: presence of the words is accepted as handling, \
          while no store, no replay, no key scope and no conflict path is examined. `is_idempotent` \
          is the emptiness of the findings (idempotency_guard/mod.rs::evaluate_idempotency), and a \
          finding is published as `GateStatus::Warning`, so the gate in the label's name withholds \
          nothing (pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates).",
    blocked_on: None,
};

pub const MIGRATION_BOUNDARY_STATUS: GateFidelity = GateFidelity {
    gate_id: "migration_boundary_status",
    aspiration: "No component recorded as Migrating depends on one recorded as Superseded, so the \
                 partition the ledger describes can actually be executed.",
    reference: "the repository's own migration ledger and its four verdicts; strangler-fig \
                partitioning",
    fidelity: Fidelity::Heuristic,
    gap: "The rule is real and the third state is right -- an unreadable tree reports no \
          measurement rather than a pass -- but the graph it runs on is text. Edges come from \
          scanning for the literal `crate::` and taking the one or two lowercase segments that \
          follow (migration/mod.rs::live_tree_violations), so a dependency spelled through a \
          re-export, an alias, a macro, or a path whose next segment is not lowercase is not an \
          edge it sees, and comments are dropped only where a trimmed line begins with a comment \
          opener, leaving trailing and block comments in the text it scans. `verdict_for` answers \
          None for a path no ledger row covers, and `check_edge` then returns None as well \
          (migration/boundary.rs::verdict_for and migration/boundary.rs::check_edge), so an \
          unclassified module is silently exempt rather than failing -- the opposite of what an \
          unclassified subject should do. The subject is also whatever tree it is handed: the \
          evaluator passes `repo_working_dir`, the repository under review, while the ledger names \
          this crate's own components, so against any other repository every module is \
          unclassified, no edge is judged, and the gate publishes a pass \
          (pre_merge_guard/evaluator.rs::evaluate_pre_merge_gates).",
    blocked_on: None,
};

pub const MODULARIZATION_STATUS: GateFidelity = GateFidelity {
    gate_id: "modularization_status",
    aspiration: "No file in the tree is a monolith: each source file stays inside a stated line \
                 budget and the directory structure stays inside a stated depth.",
    reference: "the 300-line file budget this repository's own decision record fixes; Google's \
                readability guidance on file size",
    fidelity: Fidelity::Heuristic,
    gap: "Reads no file, so it does not measure the thing it reports. The counter it compares \
          against `MAX_RECOMMENDED_LINES` is incremented once per ADDED line in the diff \
          (modularization_guard.rs::evaluate_modularization), so what is judged is how much this \
          change wrote into a file, not how long the file is: a five-thousand-line file passes \
          where the change adds ten lines, and a new file of three hundred and one lines is \
          indistinguishable from that many lines added to an existing one. The lower bound the \
          label publishes is enforced nowhere. The pass then states the property that was not \
          measured -- `files are strictly bounded within 100-300 lines` -- from that count of \
          added lines. The second rule is arithmetic on path segments against per-category \
          maxima, with the category itself decided by path prefix, and it is what fills \
          `oversized_files` for a diff carrying no file header at all -- no size is judged there. \
          The measurement the name claims does exist in \
          this tree, in another gate, where the file is read from disk and only a change that grew \
          it past `MAX_WHOLE_FILE_LINES` is charged \
          (monorepo_guard/whole_file_expansion.rs::evaluate_whole_file).",
    blocked_on: None,
};
