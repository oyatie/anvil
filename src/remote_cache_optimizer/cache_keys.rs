pub struct CacheKeyGenerator;

impl Default for CacheKeyGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheKeyGenerator {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic calculation of remote Sccache and Cargo cache keys from lockfiles and toolchains
    /// Uses FNV-1a 64-bit fixed-seed hashing for cross-process, cross-machine determinism
    pub fn compute_cache_key(&self, lockfile_content: &str, toolchain_version: &str) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in toolchain_version
            .as_bytes()
            .iter()
            .chain(lockfile_content.as_bytes().iter())
        {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!(
            "sccache-v2-{}-{:016x}",
            toolchain_version.replace(' ', "-"),
            hash
        )
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
        assert!(key1.starts_with("sccache-v2-rustc-1.80.0-"));
    }
}
