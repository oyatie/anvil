//! Offline contracts for the workflow's actual pure proposal decision helper.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

// Exact shell-body review binding, not a shell parser or credential boundary.
const REVIEWED_RECONCILIATION_SHA256: &str =
    "c4f244570c589dbb47effd0d5bd57976ddad3d2b1f9e586527e01dbef3371ffd";
const BASE: &str = "1111111111111111111111111111111111111111";
const HEAD: &str = "2222222222222222222222222222222222222222";

fn observation(existing: bool) -> Value {
    let before = "[toolchain]\nchannel = \"nightly-2026-03-05\"\nprofile = \"minimal\"\n";
    json!({
        "phase": "observe", "repo": "owner/repository", "latest": "nightly-2026-03-12", "channel": "nightly-2026-03-05",
        "base": {"ref": "refs/heads/dev", "object": {"type": "commit", "sha": BASE}},
        "checkout": BASE, "dirty": "", "pages": [[]],
        "remote_status": if existing {0} else {2},
        "remote_ref": if existing {format!("{HEAD}\trefs/heads/chore/toolchain-nightly-2026-03-12\n")} else {String::new()},
        "fetched": if existing {HEAD} else {""},
        "parents": if existing {format!("{HEAD} {BASE}")} else {String::new()},
        "changes": if existing {"M\trust-toolchain.toml\n"} else {""},
        "base_entry": format!("100644 blob {BASE}\trust-toolchain.toml\n"),
        "head_entry": if existing {format!("100644 blob {HEAD}\trust-toolchain.toml\n")} else {String::new()},
        "before_hex": hex::encode(before),
        "after_hex": if existing {hex::encode(before.replace("nightly-2026-03-05", "nightly-2026-03-12"))} else {String::new()}
    })
}

fn proposal() -> Value {
    json!({"number": 12, "state": "open", "merged_at": null,
        "head": {"ref": "chore/toolchain-nightly-2026-03-12", "sha": HEAD, "repo": {"full_name": "owner/repository"}},
        "base": {"ref": "dev", "repo": {"full_name": "owner/repository"}}})
}

fn decide(input: &Value) -> Result<Value, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new("python3")
        .arg(root.join(".github/scripts/toolchain-proposal.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python3 is a required test prerequisite");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    if !output.status.success() {
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.starts_with("proposal refused: "),
            "helper did not return a policy decision: {error}"
        );
        return Err(error);
    }
    Ok(serde_json::from_slice(&output.stdout).expect("decision is JSON"))
}

#[test]
fn missing_branch_and_proposal_select_the_normal_probe_path() {
    assert_eq!(decide(&observation(false)).unwrap()["action"], "new");
}

#[test]
fn a_prepared_local_commit_requires_proof_before_any_push() {
    let mut input = observation(true);
    input["phase"] = "prepared".into();
    input["remote_status"] = 2.into();
    input["remote_ref"] = "".into();
    assert_eq!(decide(&input).unwrap()["action"], "push");
    input["pages"] = json!([[proposal()]]);
    assert!(decide(&input).is_err());
    input["pages"] = json!([[]]);
    input["after_hex"] = hex::encode("different pin bytes").into();
    assert!(decide(&input).is_err());
}

#[test]
fn validated_orphan_selects_recovery_without_claiming_a_probe() {
    let decision = decide(&observation(true)).unwrap();
    assert_eq!(decision["action"], "recover");
    assert_eq!(decision["head_sha"], HEAD);
    assert_eq!(decision["probed"], false);
}

#[test]
fn only_one_exact_open_proposal_is_acknowledged() {
    let mut input = observation(true);
    input["pages"] = json!([[], [proposal()]]);
    let decision = decide(&input).unwrap();
    assert_eq!(decision["action"], "open");
    assert_eq!(decision["number"], 12);
    for field in ["state", "head", "base"] {
        let mut other = proposal();
        other[field] = Value::Null;
        input["pages"] = json!([[other]]);
        assert!(decide(&input).is_err(), "missing {field}");
    }
    for (section, field, value) in [
        ("head", "ref", "other"),
        ("head", "sha", BASE),
        ("base", "ref", "staging"),
    ] {
        let mut other = proposal();
        other[section][field] = value.into();
        input["pages"] = json!([[other]]);
        assert!(decide(&input).is_err());
    }
    for section in ["head", "base"] {
        let mut other = proposal();
        other[section]["repo"]["full_name"] = "other/repository".into();
        input["pages"] = json!([[other]]);
        assert!(decide(&input).is_err());
    }
}

#[test]
fn closed_ambiguous_and_incomplete_censuses_refuse() {
    let mut input = observation(true);
    let mut closed = proposal();
    closed["state"] = "closed".into();
    for pages in [
        json!([[closed]]),
        json!([[proposal()], [proposal()]]),
        json!([]),
        json!([null]),
    ] {
        input["pages"] = pages;
        assert!(decide(&input).is_err());
    }
    input = observation(false);
    input["pages"] = json!([[proposal()]]);
    assert!(decide(&input).is_err());
}

#[test]
fn branch_recovery_requires_current_dev_and_exact_single_pin_bytes_and_mode() {
    for (field, value) in [
        ("checkout", json!(HEAD)),
        ("dirty", json!(" M rust-toolchain.toml")),
        ("parents", json!(format!("{HEAD} {BASE} {BASE}"))),
        ("parents", json!(format!("{HEAD} {HEAD}"))),
        ("fetched", json!(BASE)),
        ("changes", json!("M\trust-toolchain.toml\nM\tCargo.toml\n")),
        (
            "head_entry",
            json!(format!("100755 blob {HEAD}\trust-toolchain.toml\n")),
        ),
        (
            "after_hex",
            json!(hex::encode("channel = \"nightly-2026-03-12\"\n")),
        ),
        (
            "before_hex",
            json!(hex::encode(
                "channel = \"nightly-2026-03-05\"\nchannel = \"nightly-2026-03-05\"\n"
            )),
        ),
        ("remote_status", json!(128)),
        ("remote_ref", json!("")),
        ("latest", json!("nightly-2026-03-05")),
        ("latest", json!("stable")),
    ] {
        let mut input = observation(true);
        input[field] = value;
        assert!(decide(&input).is_err(), "unsupported {field}");
    }
}

#[test]
fn workflow_binds_collection_and_mutation_to_the_reviewed_decision_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let body = fs::read_to_string(root.join(".github/workflows/toolchain-weekly.yml")).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
    let channel = &doc["jobs"]["channel"];
    assert_eq!(
        channel["concurrency"]["group"].as_str(),
        Some("toolchain-channel")
    );
    assert_eq!(
        channel["concurrency"]["cancel-in-progress"].as_bool(),
        Some(false)
    );
    let steps = channel["steps"].as_sequence().unwrap();
    let step = steps
        .iter()
        .find(|step| step["name"].as_str() == Some("Open the bump"))
        .unwrap();
    let script = step["run"].as_str().unwrap();
    let mut syntax = Command::new("bash")
        .arg("-n")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    syntax
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    assert!(
        syntax.wait().unwrap().success(),
        "owning script must parse without execution"
    );
    assert_eq!(
        hex::encode(Sha256::digest(script.as_bytes())),
        REVIEWED_RECONCILIATION_SHA256
    );
    for command in [
        "command -v python3",
        "command -v jq",
        "--paginate --slurp",
        "state=all",
        ".github/scripts/toolchain-proposal.py",
        "git push -q origin",
        "gh pr create",
        "did not run the toolchain probe",
    ] {
        assert!(
            script.contains(command),
            "missing reviewed owner operation: {command}"
        );
    }
    assert!(!script.contains("already open; nothing to raise"));
    assert!(step["continue-on-error"].is_null());
}

#[test]
fn workflow_json_assembly_preserves_complete_pages_and_exact_blob_encodings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let body = fs::read_to_string(root.join(".github/workflows/toolchain-weekly.yml")).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
    let script = doc["jobs"]["channel"]["steps"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|step| step["name"].as_str() == Some("Open the bump"))
        .unwrap()["run"]
        .as_str()
        .unwrap();
    let filter = script
        .split_once("'{phase:")
        .unwrap()
        .1
        .split_once("' > \"$scratch/input.json\"")
        .unwrap()
        .0;
    let input = observation(true);
    let mut command = Command::new("jq");
    command.arg("-n");
    for (key, value) in input.as_object().unwrap() {
        let raw = if key == "pages" || key == "base" {
            json!([value])
        } else {
            value.clone()
        };
        command.args(["--argjson", key, &raw.to_string()]);
    }
    let output = command
        .arg(format!("{{phase:{filter}"))
        .output()
        .expect("jq is required");
    assert!(
        output.status.success(),
        "actual observation filter must evaluate"
    );
    let assembled: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(assembled, input);
    assert_eq!(decide(&assembled).unwrap()["action"], "recover");
}
