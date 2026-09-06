use syn::{Expr, ForeignItem, GenericParam, ImplItem, Pat, TraitItem};

pub(in crate::source_scan::paths::module_graph) fn expression(
    expression: &Expr,
) -> &[syn::Attribute] {
    match expression {
        Expr::Array(value) => &value.attrs,
        Expr::Assign(value) => &value.attrs,
        Expr::Async(value) => &value.attrs,
        Expr::Await(value) => &value.attrs,
        Expr::Binary(value) => &value.attrs,
        Expr::Block(value) => &value.attrs,
        Expr::Break(value) => &value.attrs,
        Expr::Call(value) => &value.attrs,
        Expr::Cast(value) => &value.attrs,
        Expr::Closure(value) => &value.attrs,
        Expr::Const(value) => &value.attrs,
        Expr::Continue(value) => &value.attrs,
        Expr::Field(value) => &value.attrs,
        Expr::ForLoop(value) => &value.attrs,
        Expr::Group(value) => &value.attrs,
        Expr::If(value) => &value.attrs,
        Expr::Index(value) => &value.attrs,
        Expr::Infer(value) => &value.attrs,
        Expr::Let(value) => &value.attrs,
        Expr::Lit(value) => &value.attrs,
        Expr::Loop(value) => &value.attrs,
        Expr::Macro(value) => &value.attrs,
        Expr::Match(value) => &value.attrs,
        Expr::MethodCall(value) => &value.attrs,
        Expr::Paren(value) => &value.attrs,
        Expr::Path(value) => &value.attrs,
        Expr::Range(value) => &value.attrs,
        Expr::RawAddr(value) => &value.attrs,
        Expr::Reference(value) => &value.attrs,
        Expr::Repeat(value) => &value.attrs,
        Expr::Return(value) => &value.attrs,
        Expr::Struct(value) => &value.attrs,
        Expr::Try(value) => &value.attrs,
        Expr::TryBlock(value) => &value.attrs,
        Expr::Tuple(value) => &value.attrs,
        Expr::Unary(value) => &value.attrs,
        Expr::Unsafe(value) => &value.attrs,
        Expr::While(value) => &value.attrs,
        Expr::Yield(value) => &value.attrs,
        Expr::Verbatim(_) => &[],
        _ => &[],
    }
}

pub(super) fn impl_item(item: &ImplItem) -> &[syn::Attribute] {
    match item {
        ImplItem::Const(value) => &value.attrs,
        ImplItem::Fn(value) => &value.attrs,
        ImplItem::Macro(value) => &value.attrs,
        ImplItem::Type(value) => &value.attrs,
        ImplItem::Verbatim(_) => &[],
        _ => &[],
    }
}

pub(super) fn trait_item(item: &TraitItem) -> &[syn::Attribute] {
    match item {
        TraitItem::Const(value) => &value.attrs,
        TraitItem::Fn(value) => &value.attrs,
        TraitItem::Macro(value) => &value.attrs,
        TraitItem::Type(value) => &value.attrs,
        TraitItem::Verbatim(_) => &[],
        _ => &[],
    }
}

pub(super) fn foreign_item(item: &ForeignItem) -> &[syn::Attribute] {
    match item {
        ForeignItem::Fn(value) => &value.attrs,
        ForeignItem::Macro(value) => &value.attrs,
        ForeignItem::Static(value) => &value.attrs,
        ForeignItem::Type(value) => &value.attrs,
        ForeignItem::Verbatim(_) => &[],
        _ => &[],
    }
}

pub(super) fn generic_param(parameter: &GenericParam) -> &[syn::Attribute] {
    match parameter {
        GenericParam::Lifetime(value) => &value.attrs,
        GenericParam::Type(value) => &value.attrs,
        GenericParam::Const(value) => &value.attrs,
    }
}

pub(super) fn pattern(pattern: &Pat) -> &[syn::Attribute] {
    match pattern {
        Pat::Const(value) => &value.attrs,
        Pat::Ident(value) => &value.attrs,
        Pat::Lit(value) => &value.attrs,
        Pat::Macro(value) => &value.attrs,
        Pat::Or(value) => &value.attrs,
        Pat::Paren(value) => &value.attrs,
        Pat::Path(value) => &value.attrs,
        Pat::Range(value) => &value.attrs,
        Pat::Reference(value) => &value.attrs,
        Pat::Rest(value) => &value.attrs,
        Pat::Slice(value) => &value.attrs,
        Pat::Struct(value) => &value.attrs,
        Pat::Tuple(value) => &value.attrs,
        Pat::TupleStruct(value) => &value.attrs,
        Pat::Type(value) => &value.attrs,
        Pat::Wild(value) => &value.attrs,
        Pat::Verbatim(_) => &[],
        _ => &[],
    }
}
