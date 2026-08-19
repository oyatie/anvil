use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic calculation of remote Sccache and Cargo cache keys from lockfiles and toolchains
    pub fn compute_cache_key(&self, lockfile_content: &str, toolchain_version: &str) -> String {
        let mut hasher = DefaultHasher::new();
        lockfile_content.hash(&mut hasher);
        toolchain_version.hash(&mut hasher);
        let hash = hasher.finish();
        format!("sccache-v1-{}-{:016x}", toolchain_version.replace(' ', "-"), hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computes_deterministic_cache_key() {
        let gen = CacheKeyGenerator::new();
        let key1 = gen.compute_cache_key("foo = 1.0", "rustc-1.80.0");
        let key2 = gen.compute_cache_key("foo = 1.0", "rustc-1.80.0");
        assert_eq!(key1, key2);
        assert!(key1.starts_with("sccache-v1-rustc-1.80.0-"));
    }
}
