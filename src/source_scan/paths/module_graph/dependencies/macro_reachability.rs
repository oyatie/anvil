use proc_macro2::{TokenStream, TokenTree};

use super::imports::ident_name;

pub(super) struct MacroInvocation {
    pub(super) path: Vec<String>,
    pub(super) tokens: TokenStream,
}

pub(super) struct ReachableTokens {
    pub(super) declares_module: bool,
    pub(super) forwards_syntax: bool,
    pub(super) invocations: Vec<MacroInvocation>,
}

pub(super) fn analyze(tokens: TokenStream) -> ReachableTokens {
    let mut result = ReachableTokens {
        declares_module: false,
        forwards_syntax: transcribers(tokens.clone())
            .into_iter()
            .any(transcriber_forwards_syntax),
        invocations: Vec::new(),
    };
    collect(tokens, &mut result);
    result
}

fn transcribers(tokens: TokenStream) -> Vec<TokenStream> {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut bodies = Vec::new();
    for window in tokens.windows(3) {
        if matches!(&window[0], TokenTree::Punct(punct) if punct.as_char() == '=')
            && matches!(&window[1], TokenTree::Punct(punct) if punct.as_char() == '>')
            && let TokenTree::Group(group) = &window[2]
        {
            bodies.push(group.stream());
        }
    }
    bodies
}

fn transcriber_forwards_syntax(tokens: TokenStream) -> bool {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    for index in 0..tokens.len() {
        if matches!(&tokens[index], TokenTree::Punct(punct) if punct.as_char() == '$') {
            // `$item`, `$expr`, `$path`, and `$tt` can each transcribe an
            // include, module declaration, or local dependency path. Waiting
            // for a following `!` only caught the special `$macro!` shape and
            // certified ordinary token forwarding as if it were literal.
            if matches!(tokens.get(index + 1), Some(TokenTree::Ident(_))) {
                return true;
            }
            if matches!(tokens.get(index + 1), Some(TokenTree::Group(_)))
                && matches!(tokens.get(index + 2), Some(TokenTree::Punct(punct)) if matches!(punct.as_char(), '*' | '+'))
            {
                return true;
            }
        }
        if let TokenTree::Group(group) = &tokens[index]
            && transcriber_forwards_syntax(group.stream())
        {
            return true;
        }
    }
    false
}

fn collect(tokens: TokenStream, result: &mut ReachableTokens) {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut index = 0;
    while index < tokens.len() {
        if matches!(&tokens[index], TokenTree::Ident(ident) if ident_name(ident) == "mod") {
            result.declares_module = true;
        }
        if let Some((invocation, end)) = invocation_at(&tokens, index) {
            result.invocations.push(invocation);
            index = end;
            continue;
        }
        if let TokenTree::Group(group) = &tokens[index] {
            collect(group.stream(), result);
        }
        index += 1;
    }
}

fn invocation_at(tokens: &[TokenTree], start: usize) -> Option<(MacroInvocation, usize)> {
    let TokenTree::Ident(first) = tokens.get(start)? else {
        return None;
    };
    let mut path = vec![ident_name(first)];
    let mut cursor = start + 1;
    while double_colon(tokens, cursor) {
        let TokenTree::Ident(segment) = tokens.get(cursor + 2)? else {
            return None;
        };
        path.push(ident_name(segment));
        cursor += 3;
    }
    if !matches!(tokens.get(cursor), Some(TokenTree::Punct(punct)) if punct.as_char() == '!') {
        return None;
    }
    let TokenTree::Group(arguments) = tokens.get(cursor + 1)? else {
        return None;
    };
    Some((
        MacroInvocation {
            path,
            tokens: arguments.stream(),
        },
        cursor + 2,
    ))
}

fn double_colon(tokens: &[TokenTree], index: usize) -> bool {
    matches!(tokens.get(index), Some(TokenTree::Punct(first)) if first.as_char() == ':')
        && matches!(tokens.get(index + 1), Some(TokenTree::Punct(second)) if second.as_char() == ':')
}
