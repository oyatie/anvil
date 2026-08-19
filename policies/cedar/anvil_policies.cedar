// Anvil Cedar Access Policies
permit(
    principal in Role::"SystemAdmin",
    action in [Action::"TriggerReview", Action::"TriggerCertify", Action::"TriggerFix"],
    resource in Resource::"AnvilService"
);

permit(
    principal in Service::"GitHubWebhook",
    action == Action::"IngestWebhook",
    resource == Resource::"WebhookEndpoint"
);
