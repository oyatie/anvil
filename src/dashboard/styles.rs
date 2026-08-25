/// Hyperscaler DevOps Cockpit Design Tokens and CSS Rules
pub fn get_cockpit_css() -> &'static str {
    r#"
        :root {
            --bg-dark: #0a0e17;
            --surface-dark: #111827;
            --surface-card: #162032;
            --surface-border: #1f2d47;
            --text-primary: #f3f4f6;
            --text-secondary: #9ca3af;
            --text-muted: #6b7280;
            --accent-cyan: #06b6d4;
            --accent-blue: #3b82f6;
            --accent-emerald: #10b981;
            --accent-amber: #f59e0b;
            --accent-rose: #f43f5e;
            --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            background-color: var(--bg-dark);
            color: var(--text-primary);
            font-family: var(--font-sans);
            padding: 16px 20px;
            line-height: 1.4;
        }
        .top-hero-bar {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px 20px;
            background: var(--surface-card);
            border: 1px solid var(--surface-border);
            border-radius: 10px;
            margin-bottom: 16px;
            backdrop-filter: blur(8px);
        }
        .brand-cluster {
            display: flex;
            align-items: center;
            gap: 12px;
        }
        .brand-title {
            font-size: 17px;
            font-weight: 800;
            color: var(--accent-cyan);
            letter-spacing: -0.3px;
        }
        .dora-kpis {
            display: flex;
            gap: 20px;
            align-items: center;
        }
        .dora-metric {
            display: flex;
            flex-direction: column;
            text-align: center;
        }
        .dora-lbl {
            font-size: 10px;
            text-transform: uppercase;
            color: var(--text-muted);
            font-weight: 700;
            letter-spacing: 0.5px;
        }
        .dora-num {
            font-size: 15px;
            font-weight: 800;
            color: var(--text-primary);
        }
        .socket-status {
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 6px 12px;
            background: rgba(16, 185, 129, 0.1);
            border: 1px solid rgba(16, 185, 129, 0.25);
            border-radius: 9999px;
            font-size: 12px;
            font-weight: 700;
            color: var(--accent-emerald);
        }
        .pulse-dot {
            width: 7px;
            height: 7px;
            background: var(--accent-emerald);
            border-radius: 50%;
            box-shadow: 0 0 8px var(--accent-emerald);
        }
        .cockpit-quadrant-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 16px;
            margin-bottom: 16px;
        }
        .panel-card {
            background: var(--surface-card);
            border: 1px solid var(--surface-border);
            border-radius: 10px;
            padding: 16px;
            display: flex;
            flex-direction: column;
        }
        .panel-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 12px;
            padding-bottom: 8px;
            border-bottom: 1px solid var(--surface-border);
        }
        .panel-title {
            font-size: 14px;
            font-weight: 700;
            color: var(--accent-cyan);
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .repo-row {
            background: rgba(255,255,255,0.02);
            border: 1px solid var(--surface-border);
            border-radius: 8px;
            padding: 10px 12px;
            margin-bottom: 10px;
        }
        .repo-meta {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 8px;
        }
        .repo-name {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 13px;
        }
        .repo-stats {
            display: flex;
            gap: 12px;
            font-size: 11px;
            color: var(--text-secondary);
        }
        .gitops-dag {
            display: flex;
            align-items: center;
            gap: 6px;
            overflow-x: auto;
        }
        .dag-node {
            background: rgba(0,0,0,0.3);
            border: 1px solid var(--surface-border);
            border-radius: 6px;
            padding: 4px 8px;
            display: flex;
            flex-direction: column;
            min-width: 90px;
        }
        .dag-label {
            font-size: 9px;
            text-transform: uppercase;
            color: var(--text-muted);
            font-weight: 700;
        }
        .dag-arrow {
            color: var(--accent-cyan);
            font-size: 12px;
        }
        .train-item {
            background: rgba(255,255,255,0.02);
            border: 1px solid var(--surface-border);
            border-radius: 8px;
            padding: 10px;
            margin-bottom: 8px;
        }
        .train-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 6px;
            font-size: 12px;
        }
        .train-pr {
            font-weight: 700;
            color: var(--accent-cyan);
        }
        .train-title {
            color: var(--text-secondary);
            max-width: 250px;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        .train-progress {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 11px;
        }
        .gate-grid-container {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
            gap: 6px;
            max-height: 260px;
            overflow-y: auto;
            padding-right: 4px;
        }
        .gate-cell {
            border-radius: 6px;
            padding: 6px 8px;
            display: flex;
            flex-direction: column;
            font-size: 10px;
            border: 1px solid transparent;
        }
        .gate-green {
            background: rgba(16, 185, 129, 0.1);
            border-color: rgba(16, 185, 129, 0.3);
            color: #34d399;
        }
        .gate-amber {
            background: rgba(245, 158, 11, 0.1);
            border-color: rgba(245, 158, 11, 0.3);
            color: #fbbf24;
        }
        .gate-red {
            background: rgba(244, 63, 94, 0.15);
            border-color: rgba(244, 63, 94, 0.4);
            color: #f87171;
        }
        .gate-num {
            font-weight: 800;
            font-size: 9px;
            opacity: 0.7;
        }
        .gate-name {
            font-weight: 600;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        .gate-mkr {
            font-size: 9px;
            font-weight: 700;
            margin-top: 2px;
        }
        .badge {
            padding: 2px 6px;
            border-radius: 4px;
            font-size: 10px;
            font-weight: 700;
            text-transform: uppercase;
        }
        .badge-healthy {
            background: rgba(16, 185, 129, 0.15);
            color: var(--accent-emerald);
            border: 1px solid rgba(16, 185, 129, 0.3);
        }
        .badge-warning {
            background: rgba(245, 158, 11, 0.15);
            color: var(--accent-amber);
            border: 1px solid rgba(245, 158, 11, 0.3);
        }
        .badge-queued {
            background: rgba(59, 130, 246, 0.15);
            color: var(--accent-blue);
            border: 1px solid rgba(59, 130, 246, 0.3);
        }
        .text-cyan { color: var(--accent-cyan); }
        code {
            font-family: var(--font-mono);
            font-size: 11px;
            color: var(--accent-cyan);
        }
        table {
            width: 100%;
            border-collapse: collapse;
            font-size: 12px;
        }
        th {
            color: var(--text-muted);
            padding: 8px 10px;
            font-weight: 600;
            text-transform: uppercase;
            font-size: 10px;
            border-bottom: 1px solid var(--surface-border);
            text-align: left;
        }
        td {
            padding: 8px 10px;
            border-bottom: 1px solid var(--surface-border);
        }
        tr:last-child td { border-bottom: none; }
        .progress-bar-bg {
            background: rgba(255,255,255,0.08);
            border-radius: 4px;
            height: 6px;
            width: 90px;
            overflow: hidden;
            display: inline-block;
            vertical-align: middle;
        }
        .progress-bar-fill {
            background: var(--accent-emerald);
            height: 100%;
        }
        .progress-text {
            font-size: 10px;
            font-weight: 700;
        }
        .empty-state {
            padding: 24px;
            text-align: center;
            color: var(--text-muted);
            font-size: 12px;
        }
        .btn-add-account {
            background: rgba(6, 182, 212, 0.15);
            color: var(--accent-cyan);
            border: 1px solid rgba(6, 182, 212, 0.3);
            border-radius: 6px;
            padding: 4px 10px;
            font-size: 11px;
            font-weight: 700;
            cursor: pointer;
            transition: all 0.15s ease;
        }
        .btn-add-account:hover {
            background: rgba(6, 182, 212, 0.3);
        }
        .btn-action {
            padding: 2px 8px;
            border-radius: 4px;
            font-size: 10px;
            font-weight: 700;
            cursor: pointer;
            border: 1px solid transparent;
        }
        .btn-drain {
            background: rgba(245, 158, 11, 0.15);
            color: var(--accent-amber);
            border-color: rgba(245, 158, 11, 0.3);
        }
        .btn-resume {
            background: rgba(16, 185, 129, 0.15);
            color: var(--accent-emerald);
            border-color: rgba(16, 185, 129, 0.3);
        }
        dialog {
            background: var(--surface-card);
            border: 1px solid var(--surface-border);
            border-radius: 12px;
            color: var(--text-primary);
            padding: 24px;
            max-width: 440px;
            margin: auto;
            backdrop-filter: blur(16px);
            box-shadow: 0 20px 25px -5px rgba(0,0,0,0.5);
        }
        dialog::backdrop {
            background: rgba(0, 0, 0, 0.7);
            backdrop-filter: blur(4px);
        }
        .form-group {
            margin-bottom: 12px;
            display: flex;
            flex-direction: column;
            gap: 4px;
        }
        .form-group label {
            font-size: 11px;
            font-weight: 600;
            color: var(--text-secondary);
        }
        .form-control {
            background: rgba(0, 0, 0, 0.3);
            border: 1px solid var(--surface-border);
            border-radius: 6px;
            padding: 8px 10px;
            color: var(--text-primary);
            font-size: 12px;
            font-family: inherit;
        }
        .form-control:focus {
            outline: none;
            border-color: var(--accent-cyan);
        }
        .modal-actions {
            display: flex;
            justify-content: flex-end;
            gap: 8px;
            margin-top: 16px;
        }
        @media (max-width: 1000px) {
            .cockpit-quadrant-grid { grid-template-columns: 1fr; }
            .dora-kpis { display: none; }
        }
    "#
}
