use anvil::metrics::PrometheusRegistry;

#[test]
fn test_prometheus_registry_counters_and_gauges() {
    let registry = PrometheusRegistry::new();

    // 1. Simulate review activity and gate evaluations
    registry.record_review();
    registry.record_review();
    registry.record_gate_evaluation(true);
    registry.record_gate_evaluation(true);
    registry.record_gate_evaluation(false);

    // 2. Set quality gauges
    registry.set_review_precision(0.915);
    registry.set_tia_pruning_ratio(0.785);

    // 3. Export text
    let metrics_text = registry.export_prometheus_text();

    assert!(metrics_text.contains("anvil_pr_reviews_total 2"));
    assert!(metrics_text.contains("anvil_gates_evaluated_total 3"));
    assert!(metrics_text.contains("anvil_gates_passed_total 2"));
    assert!(metrics_text.contains("anvil_gates_failed_total 1"));
    assert!(metrics_text.contains("anvil_review_precision_ratio 0.9150"));
    assert!(metrics_text.contains("anvil_tia_pruning_ratio 0.7850"));
    assert!(metrics_text.contains("anvil_flake_rate_ratio 0.002000"));
    assert!(metrics_text.contains("anvil_ci_wallclock_p95_seconds 45.00"));
    assert!(metrics_text.contains("anvil_merge_queue_dwell_p95_seconds 120.00"));
}
