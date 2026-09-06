use anvil::predictive_test_selector::{WorkspaceDagSelector, WorkspacePackage};

#[test]
fn test_tia_leaf_package_modification_isolates_scope() {
    let dag = WorkspaceDagSelector::new();
    let packages = vec![
        WorkspacePackage {
            name: "domain_core".to_string(),
            path: "crates/core".to_string(),
            dependencies: vec![],
        },
        WorkspacePackage {
            name: "auth_service".to_string(),
            path: "services/auth".to_string(),
            dependencies: vec!["domain_core".to_string()],
        },
        WorkspacePackage {
            name: "billing_service".to_string(),
            path: "services/billing".to_string(),
            dependencies: vec!["domain_core".to_string()],
        },
        WorkspacePackage {
            name: "frontend_gateway".to_string(),
            path: "gateways/frontend".to_string(),
            dependencies: vec!["auth_service".to_string(), "billing_service".to_string()],
        },
        WorkspacePackage {
            name: "isolated_tool".to_string(),
            path: "tools/isolated".to_string(),
            dependencies: vec![],
        },
    ];

    // Case 1: Modifying billing_service affects only billing_service and frontend_gateway
    let changed_billing = vec!["services/billing/src/invoice.rs".to_string()];
    let affected_billing = dag.select_affected_packages(&changed_billing, &packages);

    assert_eq!(affected_billing.len(), 2);
    assert!(affected_billing.contains(&"billing_service".to_string()));
    assert!(affected_billing.contains(&"frontend_gateway".to_string()));
    assert!(!affected_billing.contains(&"auth_service".to_string()));
    assert!(!affected_billing.contains(&"domain_core".to_string()));
    assert!(!affected_billing.contains(&"isolated_tool".to_string()));

    let pruning_ratio =
        WorkspaceDagSelector::calculate_pruning_ratio(affected_billing.len(), packages.len());
    assert!(
        (pruning_ratio - 0.6).abs() < 1e-6,
        "3 out of 5 packages spared = 60% pruning ratio"
    );
}

#[test]
fn test_tia_root_core_modification_triggers_transitive_dependents() {
    let dag = WorkspaceDagSelector::new();
    let packages = vec![
        WorkspacePackage {
            name: "domain_core".to_string(),
            path: "crates/core".to_string(),
            dependencies: vec![],
        },
        WorkspacePackage {
            name: "auth_service".to_string(),
            path: "services/auth".to_string(),
            dependencies: vec!["domain_core".to_string()],
        },
        WorkspacePackage {
            name: "frontend_gateway".to_string(),
            path: "gateways/frontend".to_string(),
            dependencies: vec!["auth_service".to_string()],
        },
        WorkspacePackage {
            name: "standalone_cli".to_string(),
            path: "tools/cli".to_string(),
            dependencies: vec![],
        },
    ];

    // Case 2: Modifying domain_core affects domain_core, auth_service, and frontend_gateway
    let changed_core = vec!["crates/core/src/types.rs".to_string()];
    let affected_core = dag.select_affected_packages(&changed_core, &packages);

    assert_eq!(affected_core.len(), 3);
    assert!(affected_core.contains(&"domain_core".to_string()));
    assert!(affected_core.contains(&"auth_service".to_string()));
    assert!(affected_core.contains(&"frontend_gateway".to_string()));
    assert!(!affected_core.contains(&"standalone_cli".to_string()));
}
