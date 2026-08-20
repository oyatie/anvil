/// Reactive Client JavaScript for SSE Streaming and Fleet Cockpit DOM Hydration
pub fn get_client_scripts() -> &'static str {
    r#"
        async function fetchDashboardState() {
            try {
                const res = await fetch('/api/dashboard/state');
                if (!res.ok) return;
                const data = await res.json();
                
                // Update DORA KPIs
                if (data.dora_metrics) {
                    const leadEl = document.querySelector('#lead-time-val');
                    if (leadEl) leadEl.textContent = data.dora_metrics.lead_time_hours.toFixed(1) + 'h';
                    const deployEl = document.querySelector('#deploy-cadence-val');
                    if (deployEl) deployEl.textContent = data.dora_metrics.deployment_frequency_per_day.toFixed(1) + '/d';
                    const mttrEl = document.querySelector('#mttr-val');
                    if (mttrEl) mttrEl.textContent = data.dora_metrics.mttr_minutes.toFixed(0) + 'm';
                    const cfrEl = document.querySelector('#cfr-val');
                    if (cfrEl) cfrEl.textContent = data.dora_metrics.change_failure_rate_pct.toFixed(1) + '%';
                }
            } catch (err) {}
        }

        function initFleetSSE() {
            const eventSource = new EventSource('/events');
            eventSource.addEventListener('fleet_update', function(e) {
                try {
                    const event = JSON.parse(e.data);
                    const tableBody = document.querySelector('#activity-tbody');
                    if (tableBody) {
                        const row = document.createElement('tr');
                        row.innerHTML = `<td>${event.timestamp_utc}</td><td><code>${event.repo}</code></td><td><strong>${event.entity_id}</strong></td><td>${event.title}</td><td><span class="badge badge-healthy">${event.status}</span></td>`;
                        tableBody.insertBefore(row, tableBody.firstChild);
                    }
                    fetchDashboardState();
                } catch(err) {}
            });
            eventSource.onerror = function() { setTimeout(initFleetSSE, 5000); };
        }

        setInterval(fetchDashboardState, 3000);
        initFleetSSE();

        function openAddAccountModal() {
            document.querySelector('#add-account-dialog').showModal();
        }

        function closeAddAccountModal() {
            document.querySelector('#add-account-dialog').close();
        }

        async function submitAddAccount(event) {
            event.preventDefault();
            const accountId = document.querySelector('#acc-id').value.trim();
            const provider = document.querySelector('#acc-provider').value;
            const authType = document.querySelector('#acc-authtype').value;
            const oauthToken = document.querySelector('#acc-oauth').value.trim();
            const configDir = document.querySelector('#acc-config-dir').value.trim();
            const max5hr = parseInt(document.querySelector('#acc-5hr').value, 10);
            const weeklyBudget = parseFloat(document.querySelector('#acc-budget').value);

            try {
                const res = await fetch('/api/accounts/pool', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        account_id: accountId,
                        provider: provider,
                        auth_type: authType,
                        oauth_token: oauthToken || null,
                        config_dir: configDir || null,
                        auth_profile_or_key: oauthToken || null,
                        max_5hr_tokens: max5hr || 1000000,
                        max_weekly_budget_usd: weeklyBudget || 100.0
                    })
                });
                const data = await res.json();
                if (data.success) {
                    closeAddAccountModal();
                    fetchDashboardState();
                    location.reload();
                } else {
                    alert('Error: ' + data.message);
                }
            } catch(err) {
                alert('Network error: ' + err);
            }
        }

        async function drainAccount(accountId) {
            if (!confirm(`Are you sure you want to drain account '${accountId}'?`)) return;
            try {
                const res = await fetch('/api/accounts/drain', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ account_id: accountId })
                });
                const data = await res.json();
                if (data.success) {
                    fetchDashboardState();
                    location.reload();
                }
            } catch(e) {}
        }

        async function resumeAccount(accountId) {
            try {
                const res = await fetch('/api/accounts/resume', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ account_id: accountId })
                });
                const data = await res.json();
                if (data.success) {
                    fetchDashboardState();
                    location.reload();
                }
            } catch(e) {}
        }
    "#
}
