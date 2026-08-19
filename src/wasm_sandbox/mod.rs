pub mod policy_runner;

use policy_runner::WasmPolicyRunner;

#[derive(Clone, Debug)]
pub struct WasmSandboxReport {
    pub passed: bool,
    pub active_wasm_plugins: usize,
    pub policy_violations: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct WasmPolicySandbox {
    runner: WasmPolicyRunner,
}

impl Default for WasmPolicySandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmPolicySandbox {
    pub fn new() -> Self {
        Self {
            runner: WasmPolicyRunner::new(),
        }
    }

    pub fn execute_sandboxed_policies(&self, diff_content: &str) -> WasmSandboxReport {
        let violations = self.runner.run_sandboxed_bytecode_checks(diff_content);
        let passed = violations.is_empty();

        let summary = if passed {
            "All WebAssembly dynamic policy plugins evaluated cleanly with zero violations."
                .to_string()
        } else {
            format!(
                "WebAssembly dynamic policy sandbox detected {} violations.",
                violations.len()
            )
        };

        WasmSandboxReport {
            passed,
            active_wasm_plugins: self.runner.plugin_count(),
            policy_violations: violations,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_sandbox_nominal() {
        let sandbox = WasmPolicySandbox::new();
        let diff = "+ fn safe_operation() -> Result<()> { Ok(()) }";
        let report = sandbox.execute_sandboxed_policies(diff);
        assert!(report.passed);
    }
}
