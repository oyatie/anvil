//! The ephemeral-sandbox gate must not publish a latency it never measured.

use anvil::ephemeral_sandbox::EphemeralSandboxManager;
use anvil::pre_merge_guard::GateStatus;

/// `allocate_ephemeral_sandbox` allocated nothing. It returned a struct
/// literal -- `is_isolated: true`, `spinup_latency_ms: 185` -- so
/// `is_hermetic` was a constant, and every pull request published
///
///   "PASSED (Ephemeral sandbox allocated in 185ms; zero host state leaks
///    or port collisions)"
///
/// with `average_spinup_ms: 185`. A hardcoded number reported as a
/// measurement, on a gate whose name claims a sandbox was spun up.
#[test]
fn an_unrun_sandbox_is_not_measured_and_publishes_no_latency() {
    let report = EphemeralSandboxManager::new().evaluate_without_sandbox_runtime();

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("sandbox_status"),
        "no sandbox runtime exists, so the gate must report NotMeasured"
    );
    assert!(
        !report.is_hermetic,
        "nothing was isolated because nothing ran"
    );
    assert_eq!(
        report.sandboxes_allocated, 0,
        "no sandbox was allocated, so none may be counted"
    );
    assert_eq!(
        report.average_spinup_ms, 0,
        "no sandbox was started, so no spin-up time may be published"
    );
    assert!(
        !report.summary.contains("185"),
        "the summary must not quote a fabricated latency: {}",
        report.summary
    );
    assert!(
        !matches!(report.status, GateStatus::Passed),
        "an unmeasured sandbox is not a passing one"
    );
}
