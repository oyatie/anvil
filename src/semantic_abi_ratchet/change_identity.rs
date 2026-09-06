use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingAbiFinding {
    pub file_path: String,
    pub symbol_name: String,
    pub change_kind: String,
    pub detail: String,
    // Only a fresh scanner observation can carry authorizing evidence.
    #[serde(skip)]
    pub(super) evidence: Option<ChangeEvidence>,
}

#[derive(Debug, Clone)]
enum ChangeKind {
    SignatureChange,
    Removal,
}

#[derive(Debug, Clone)]
pub(super) struct ChangeEvidence {
    kind: ChangeKind,
    symbol: String,
    before_path: String,
    after_path: Option<String>,
    before_line: String,
    after_line: Option<String>,
}

impl ChangeEvidence {
    pub(super) fn removal(symbol: &str, path: Option<&str>, line: Option<&str>) -> Option<Self> {
        Some(Self {
            kind: ChangeKind::Removal,
            symbol: symbol.to_owned(),
            before_path: path.filter(|p| !p.is_empty())?.to_owned(),
            after_path: None,
            before_line: line?.to_owned(),
            after_line: None,
        })
    }

    pub(super) fn signature_change(
        symbol: &str,
        before_path: Option<&str>,
        after_path: Option<&str>,
        before_line: Option<&str>,
        after_line: Option<&str>,
    ) -> Option<Self> {
        Some(Self {
            kind: ChangeKind::SignatureChange,
            symbol: symbol.to_owned(),
            before_path: before_path.filter(|p| !p.is_empty())?.to_owned(),
            after_path: Some(after_path.filter(|p| !p.is_empty())?.to_owned()),
            before_line: before_line?.to_owned(),
            after_line: Some(after_line?.to_owned()),
        })
    }
}

/// A declaration-line transition, not a resolved ABI or a one-use signing.
/// The tuple retains field boundaries; only outer line whitespace is omitted.
pub(super) fn abi_key(repo: &str, finding: &BreakingAbiFinding) -> Option<String> {
    if repo.trim().is_empty() {
        return None;
    }
    let evidence = finding.evidence.as_ref()?;
    let kind = match evidence.kind {
        ChangeKind::SignatureChange => "SIGNATURE_CHANGE",
        ChangeKind::Removal => "REMOVAL",
    };
    let tuple = (
        repo,
        kind,
        &evidence.before_path,
        &evidence.after_path,
        &evidence.symbol,
        &evidence.before_line,
        &evidence.after_line,
    );
    serde_json::to_string(&tuple)
        .ok()
        .map(|encoded| format!("abi-change/v2:{encoded}"))
}
