//! Span-accurate removal of modules that cannot compile with `test = false`.

use std::ops::Range;

use syn::spanned::Spanned;
use syn::{Item, ItemMod};

use super::cfg::excludes_when_test_is_false;

pub(super) fn strip(source: &str) -> Result<String, String> {
    let parsed = syn::parse_file(source).map_err(|error| {
        format!("cannot parse Rust source before removing test modules: {error}")
    })?;
    let mut ranges = Vec::new();
    collect_test_module_ranges(&parsed.items, source.len(), &mut ranges)?;
    ranges.sort_by_key(|range| range.start);
    for pair in ranges.windows(2) {
        if pair[0].end > pair[1].start {
            return Err("overlapping cfg(test) module spans in parsed Rust source".to_owned());
        }
    }

    let mut bytes = source.as_bytes().to_vec();
    for range in ranges {
        for byte in &mut bytes[range] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(bytes).map_err(|error| format!("stripped Rust source is not UTF-8: {error}"))
}

fn collect_test_module_ranges(
    items: &[Item],
    source_len: usize,
    ranges: &mut Vec<Range<usize>>,
) -> Result<(), String> {
    for item in items {
        let Item::Mod(module) = item else { continue };
        if excludes_when_test_is_false(&module.attrs) {
            ranges.push(module_range(module, source_len)?);
        } else if let Some((_, nested)) = &module.content {
            collect_test_module_ranges(nested, source_len, ranges)?;
        }
    }
    Ok(())
}

fn module_range(module: &ItemMod, source_len: usize) -> Result<Range<usize>, String> {
    let start = module
        .attrs
        .first()
        .map(Spanned::span)
        .unwrap_or_else(|| module.span())
        .byte_range()
        .start;
    let end = if let Some(semi) = &module.semi {
        semi.span.byte_range().end
    } else if let Some((brace, _)) = &module.content {
        brace.span.close().byte_range().end
    } else {
        return Err(format!(
            "cfg(test) module `{}` has neither an external nor inline body",
            module.ident
        ));
    };
    if start >= end || end > source_len {
        return Err(format!(
            "cfg(test) module `{}` has invalid source span {start}..{end} for {source_len} bytes",
            module.ident
        ));
    }
    Ok(start..end)
}

#[cfg(test)]
mod tests;
