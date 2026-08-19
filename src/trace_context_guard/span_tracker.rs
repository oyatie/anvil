use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedSpanFinding {
    pub file_path: String,
    pub line_number: usize,
    pub snippet: String,
    pub issue: String,
}

pub struct SpanTracker;

impl SpanTracker {
    pub fn new() -> Self {
        Self
    }

    /// Scans diff for detached async tasks (e.g. `tokio::spawn` without `.instrument(...)`)
    pub fn scan_detached_tasks(&self, file_path: &str, content: &str) -> Vec<DetachedSpanFinding> {
        let mut findings = Vec::new();
        let spawn_re = Regex::new(r"tokio::spawn\s*\(").unwrap();
        let instrument_re = Regex::new(r"\.instrument\s*\(").unwrap();

        let lines: Vec<&str> = content.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if spawn_re.is_match(line) {
                // Check following 5 lines for `.instrument(...)`
                let end = (idx + 6).min(lines.len());
                let window = lines[idx..end].join("\n");

                if !instrument_re.is_match(&window) {
                    findings.push(DetachedSpanFinding {
                        file_path: file_path.to_string(),
                        line_number: idx + 1,
                        snippet: line.trim().to_string(),
                        issue: "Asynchronous task spawned without attaching tracing span via `.instrument(...)`".to_string(),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_uninstrumented_spawn() {
        let tracker = SpanTracker::new();
        let code = r#"
pub async fn handle_event() {
    tokio::spawn(async move {
        do_work().await;
    });
}
"#;
        let findings = tracker.scan_detached_tasks("src/handler.rs", code);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_passes_instrumented_spawn() {
        let tracker = SpanTracker::new();
        let code = r#"
pub async fn handle_event() {
    tokio::spawn(async move {
        do_work().await;
    }.instrument(tracing::info_span!("event_worker")));
}
"#;
        let findings = tracker.scan_detached_tasks("src/handler.rs", code);
        assert_eq!(findings.len(), 0);
    }
}
