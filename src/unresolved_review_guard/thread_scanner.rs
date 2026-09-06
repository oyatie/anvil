use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedReviewThread {
    pub thread_id: String,
    pub path: String,
    pub line: Option<u64>,
    pub comment_body: String,
    pub author: String,
}

pub struct ThreadScanner;

impl Default for ThreadScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadScanner {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic validation of review threads: zero unresolved threads permitted before merge queue admission
    pub fn evaluate_unresolved_threads(
        &self,
        threads: &[UnresolvedReviewThread],
    ) -> Result<(), Vec<UnresolvedReviewThread>> {
        if threads.is_empty() {
            Ok(())
        } else {
            Err(threads.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_unresolved_thread() {
        let scanner = ThreadScanner::new();
        let threads = vec![UnresolvedReviewThread {
            thread_id: "thread_1".to_string(),
            path: "src/main.rs".to_string(),
            line: Some(42),
            comment_body: "Please fix unwrap".to_string(),
            author: "reviewer".to_string(),
        }];

        let res = scanner.evaluate_unresolved_threads(&threads);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().len(), 1);
    }

    #[test]
    fn test_passes_when_zero_unresolved_threads() {
        let scanner = ThreadScanner::new();
        let res = scanner.evaluate_unresolved_threads(&[]);
        assert!(res.is_ok());
    }
}
