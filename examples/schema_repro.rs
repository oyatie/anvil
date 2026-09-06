//! Runs gate 53 over a unified diff on stdin, so the false-positive rate can be
//! measured against real history instead of asserted. See
//! `scripts/schema_regression.sh`.

use anvil::schema_evolution::SchemaEvolutionRatchet;
use std::io::Read;

fn main() {
    let mut diff = String::new();
    std::io::stdin()
        .read_to_string(&mut diff)
        .expect("a unified diff on stdin");

    let report = SchemaEvolutionRatchet::new().evaluate_schema_evolution(&diff);
    println!(
        "{:>2} finding(s)  {}",
        report.breaking_field_changes,
        report.status.badge()
    );
    if report.breaking_field_changes > 0 {
        for finding in report.summary.split("; ") {
            println!("     {finding}");
        }
    }
}
