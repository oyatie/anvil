use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAdvisory {
    pub package_name: String,
    pub advisory_id: String,
    pub vulnerable_version: String,
    pub patched_version: String,
    pub severity: String,
}

pub struct AdvisoryListener;

impl Default for AdvisoryListener {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvisoryListener {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of upstream RustSec / GitHub security advisories against repository lockfiles
    pub fn reconcile_advisories(
        &self,
        lockfile_content: &str,
        advisories: &[SecurityAdvisory],
    ) -> Vec<SecurityAdvisory> {
        let mut affected = Vec::new();

        for adv in advisories {
            if lockfile_content.contains(&format!("name = \"{}\"", adv.package_name))
                && lockfile_content.contains(&format!("version = \"{}\"", adv.vulnerable_version))
            {
                affected.push(adv.clone());
            }
        }

        affected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_vulnerable_package() {
        let listener = AdvisoryListener::new();
        let lockfile = "name = \"openssl\"\nversion = \"0.10.30\"";
        let advs = vec![SecurityAdvisory {
            package_name: "openssl".to_string(),
            advisory_id: "RUSTSEC-2026-0001".to_string(),
            vulnerable_version: "0.10.30".to_string(),
            patched_version: "0.10.31".to_string(),
            severity: "HIGH".to_string(),
        }];

        let res = listener.reconcile_advisories(lockfile, &advs);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].advisory_id, "RUSTSEC-2026-0001");
    }
}
