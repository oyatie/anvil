use proc_macro2::{TokenStream, TokenTree};

use super::imports::ident_name;

pub(super) struct MacroPaths {
    pub(super) dynamic_local: bool,
    pub(super) literal: Vec<Vec<String>>,
}

pub(super) fn analyze(tokens: TokenStream) -> MacroPaths {
    let dynamic_local = has_dynamic_local_path(tokens.clone());
    let mut literal = Vec::new();
    collect_literal_paths(tokens, &mut literal);
    MacroPaths {
        dynamic_local,
        literal,
    }
}

fn collect_literal_paths(tokens: TokenStream, paths: &mut Vec<Vec<String>>) {
    let flat = tokens.into_iter().collect::<Vec<_>>();
    let mut index = 0;
    while index < flat.len() {
        let TokenTree::Ident(first) = &flat[index] else {
            index += 1;
            continue;
        };
        let mut segments = vec![ident_name(first)];
        let mut cursor = index + 1;
        while double_colon(&flat, cursor)
            && matches!(flat.get(cursor + 2), Some(TokenTree::Ident(_)))
        {
            if let Some(TokenTree::Ident(segment)) = flat.get(cursor + 2) {
                segments.push(ident_name(segment));
            }
            cursor += 3;
        }
        paths.push(segments);
        index = cursor.max(index + 1);
    }
    for token in flat {
        if let TokenTree::Group(group) = token {
            collect_literal_paths(group.stream(), paths);
        }
    }
}

fn has_dynamic_local_path(tokens: TokenStream) -> bool {
    let flat = tokens.into_iter().collect::<Vec<_>>();
    for index in 0..flat.len() {
        if matches!(flat.get(index), Some(TokenTree::Ident(root)) if matches!(ident_name(root).as_str(), "crate" | "self" | "super"))
            && double_colon(&flat, index + 1)
            && matches!(flat.get(index + 3), Some(TokenTree::Punct(punct)) if punct.as_char() == '$')
        {
            return true;
        }
    }
    flat.into_iter().any(
        |token| matches!(token, TokenTree::Group(group) if has_dynamic_local_path(group.stream())),
    )
}

fn double_colon(tokens: &[TokenTree], index: usize) -> bool {
    matches!(tokens.get(index), Some(TokenTree::Punct(a)) if a.as_char() == ':')
        && matches!(tokens.get(index + 1), Some(TokenTree::Punct(b)) if b.as_char() == ':')
}
