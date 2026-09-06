//! Build policy and actual constructor/consumer bindings. Private tests inspect
//! supplied observations and unspawned command configuration. These source
//! contracts are not live-child measurements or filesystem/network containment.
use anvil::exec::build_env::{BUILD_INHERITED, NEVER_HANDED_OVER};
use quote::ToTokens;
use syn::visit::Visit;

fn body(module: &str, name: &str) -> String {
    let source = anvil::source_scan::paths::module_source(
        module,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    let parsed = syn::parse_file(&source).expect("actual production source parses");
    struct Bodies<'a> {
        name: &'a str,
        found: Vec<String>,
    }
    impl<'ast> Visit<'ast> for Bodies<'_> {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            if node.sig.ident == self.name {
                self.found.push(node.block.to_token_stream().to_string());
            }
            syn::visit::visit_item_fn(self, node);
        }
        fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
            if node.sig.ident == self.name {
                self.found.push(node.block.to_token_stream().to_string());
            }
            syn::visit::visit_impl_item_fn(self, node);
        }
    }
    let mut bodies = Bodies {
        name,
        found: Vec::new(),
    };
    bodies.visit_file(&parsed);
    assert_eq!(bodies.found.len(), 1, "unique actual function {name}");
    bodies.found[0].split_whitespace().collect()
}

#[test]
fn no_secret_name_is_on_the_allowlist() {
    for forbidden in NEVER_HANDED_OVER {
        assert!(!BUILD_INHERITED.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn actual_constructor_uses_the_observation_seam() {
    let command = body("src/exec/build_env/mod", "command");
    assert!(command.contains("letmutcmd=tokio::process::Command::new(program);apply(&mutcmd);cmd"));
    let apply = body("src/exec/build_env/mod", "apply");
    assert!(apply.contains("apply_from(cmd,|name|std::env::var(name));"));
    let seam = body("src/exec/build_env/mod", "apply_from");
    assert!(seam.contains("super::non_model::clear_environment(cmd);fornameinBUILD_INHERITED{ifletOk(value)=read(name){cmd.env(name,value);}}"));
}

#[test]
fn actual_cargo_gate_build_and_run_use_the_scrubbed_constructor() {
    let gate = body("src/queue_healer/mod", "run_cargo_test_gate");
    assert!(gate.contains("letmutbuild=crate::exec::build_env::command(\"cargo\");build.args([\"test\",\"--no-run\"]).current_dir(repo_dir);"));
    assert!(gate.contains("crate::exec::run_bounded(build,ExecClass::Build,BUILD_LABEL).await"));
    assert!(gate.contains("letmutrun=crate::exec::build_env::command(\"cargo\");run.args([\"test\",\"--no-fail-fast\"]).current_dir(repo_dir);"));
    assert!(gate.contains("crate::exec::run_bounded_for(run,remaining,label).await"));
}

#[test]
fn actual_clear_marker_survives_rebinding_without_being_forwarded() {
    let clear = body("src/exec/non_model/mod", "clear_environment");
    assert!(clear.contains("command.env_clear();command.env(CLEARED_ENV_MARKER,\"1\");"));
    let bind = body("src/exec/non_model/mod", "bind_std_program");
    assert!(bind.contains("command.get_envs()"));
    assert!(bind.contains("letenvironment_cleared=environment.iter().any(|(name,value)|{name==OsStr::new(CLEARED_ENV_MARKER)&&value.as_deref()==Some(OsStr::new(\"1\"))});"));
    assert!(bind.contains("letmutbound=std::process::Command::new(canonical);"));
    assert!(bind.contains("ifenvironment_cleared{bound.env_clear();}"));
    assert!(bind.contains("for(name,value)inenvironment{ifname==OsStr::new(CLEARED_ENV_MARKER){continue;}ifletSome(value)=value{bound.env(name,value);}else{bound.env_remove(name);}}"));
}
