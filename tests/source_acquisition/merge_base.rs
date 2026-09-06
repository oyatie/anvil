//! Complete measurement inputs for required merge-base comparisons.

use anvil::ratchet::facade::derived::Derived;
use anvil::ratchet::ports::RefError;
use anvil::shape::facade::TreeSource;

pub fn rust_sources(tree: &dyn TreeSource) -> Result<Vec<(String, String)>, String> {
    let mut sources = Vec::new();
    for path in tree
        .paths()
        .iter()
        .filter(|p| p.starts_with("src/") && p.ends_with(".rs"))
    {
        let bytes = tree
            .read(path)
            .map_err(|error| format!("cannot read {path}: {error}"))?
            .ok_or_else(|| format!("listed source {path} has no contents"))?;
        let text = std::str::from_utf8(bytes)
            .map_err(|error| format!("source {path} is not UTF-8: {error}"))?;
        sources.push((path.clone(), text.to_owned()));
    }
    if sources.is_empty() {
        return Err("no Rust source files in the merge-base corpus".into());
    }
    Ok(sources)
}

pub fn required<T>(
    result: Result<Derived<Result<T, String>>, RefError>,
) -> Result<Derived<T>, String> {
    let derived = result.map_err(|error| format!("cannot acquire merge-base: {error}"))?;
    Ok(Derived {
        at_merge_base: derived.at_merge_base.map_err(|error| {
            format!("cannot measure merge-base {}: {error}", derived.merge_base)
        })?,
        merge_base: derived.merge_base,
    })
}
