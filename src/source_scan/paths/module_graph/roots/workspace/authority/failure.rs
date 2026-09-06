//! Private refusal custody. Normal admission discards these without output;
//! only the owning diagnostic test formats them. Never retain Cargo stdout,
//! environment values, source text, or an unbounded underlying error.

use std::fmt;

pub(super) enum Failure {
    Stage(&'static str),
    Metadata {
        code: Option<i32>,
        stderr: String,
        total: usize,
    },
}

impl Failure {
    pub(super) fn metadata(code: Option<i32>, stderr: &[u8]) -> Self {
        Self::Metadata {
            code,
            stderr: stderr
                .iter()
                .take(512)
                .flat_map(|byte| std::ascii::escape_default(*byte))
                .map(char::from)
                .collect(),
            total: stderr.len(),
        }
    }

    #[cfg(test)]
    pub(super) fn message(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stage(stage) => output.write_str(stage),
            Self::Metadata {
                code,
                stderr,
                total,
            } => write!(
                output,
                "metadata non-success: exit={code:?}; stderr_bytes={total}; first_512_bytes={stderr}"
            ),
        }
    }
}
