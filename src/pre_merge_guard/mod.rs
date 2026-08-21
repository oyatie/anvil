//! PreMergeGuard: 70-Gate Quality Certification & Governance Matrix

pub mod evaluator;
pub mod matrix;
pub mod product_bar;
pub mod report;
pub mod scanner;

pub use evaluator::PreMergeGuard;
pub use matrix::MatrixRenderer;
pub use report::{GateStatus, PreMergeCertificationReport};
pub use scanner::PreMergeScanner;
