#[derive(Clone)]
pub(super) struct Import {
    pub(super) target: Vec<String>,
    pub(super) binding: Option<String>,
}

pub(super) fn collect_imports(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    imports: &mut Vec<Import>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(ident_name(&path.ident));
            collect_imports(&path.tree, prefix, imports);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let name = ident_name(&name.ident);
            let mut target = prefix.clone();
            if name != "self" {
                target.push(name.clone());
            }
            let binding = (name != "self")
                .then_some(name)
                .or_else(|| prefix.last().cloned());
            imports.push(Import { target, binding });
        }
        syn::UseTree::Rename(rename) => {
            let mut target = prefix.clone();
            if ident_name(&rename.ident) != "self" {
                target.push(ident_name(&rename.ident));
            }
            imports.push(Import {
                target,
                binding: Some(ident_name(&rename.rename)),
            });
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_imports(tree, prefix, imports);
            }
        }
        syn::UseTree::Glob(_) => imports.push(Import {
            target: prefix.clone(),
            binding: None,
        }),
    }
}

pub(super) fn ident_name(ident: &proc_macro2::Ident) -> String {
    ident.to_string().trim_start_matches("r#").to_owned()
}
