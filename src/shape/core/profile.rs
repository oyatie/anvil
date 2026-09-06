//! Language profiles: how a tree declares a build unit and where its
//! dependency edges are read from. These are facts about ecosystems, not
//! about any tenant's layout, which is why the marker file names live in code.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageProfile {
    /// Cargo packages: unit marker `Cargo.toml`; edges from `[dependencies]`
    /// path entries and `workspace = true`; workspace `members` glob check.
    RustCargo,
    /// Buck2 packages: unit marker `BUCK`; edges from `deps = ["//..."]` labels.
    RustBuck2,
    /// npm/pnpm workspaces: unit marker `package.json`; edges from imports
    /// resolved through `workspaces`.
    TsWorkspace,
    /// A single Rust crate whose units are modules: marker `mod.rs`; edges from
    /// `use crate::<unit>::<face>` paths.
    RustModuleTree,
}

impl LanguageProfile {
    pub const ALL: [LanguageProfile; 4] = [
        LanguageProfile::RustCargo,
        LanguageProfile::RustBuck2,
        LanguageProfile::TsWorkspace,
        LanguageProfile::RustModuleTree,
    ];

    /// The file whose presence makes a directory a build unit under this profile.
    pub fn unit_marker(self) -> &'static str {
        match self {
            LanguageProfile::RustCargo => "Cargo.toml",
            LanguageProfile::RustBuck2 => "BUCK",
            LanguageProfile::TsWorkspace => "package.json",
            LanguageProfile::RustModuleTree => "mod.rs",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            LanguageProfile::RustCargo => "rust-cargo",
            LanguageProfile::RustBuck2 => "rust-buck2",
            LanguageProfile::TsWorkspace => "ts-workspace",
            LanguageProfile::RustModuleTree => "rust-module-tree",
        }
    }
}

impl std::fmt::Display for LanguageProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
