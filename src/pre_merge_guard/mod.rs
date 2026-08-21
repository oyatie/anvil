//! PreMergeGuard: the live gate corpus, certification, and governance matrix.
//! The count is `TOTAL_GATES`, never a number written in prose.

pub mod evaluator;
pub mod matrix;
pub mod report;
pub mod scanner;

pub use evaluator::PreMergeGuard;
pub use matrix::MatrixRenderer;
pub use report::{GateStatus, PreMergeCertificationReport};
pub use scanner::PreMergeScanner;
