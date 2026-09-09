//! Exact shape checks for the prompt-value and provider-byte boundary.
//!
//! Visibility alone is insufficient here: `model_prompt` descendants may use
//! private fields, and a conditional transport rewrite can preserve every
//! public signature. These tests inventory the value escapes and pin the
//! implementations whose dataflow is the security claim.

use proc_macro2::TokenStream;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::Visit;

const MODEL_PROMPT: &str = "src/model_prompt.rs";
const HARNESS: &str = "src/model_prompt/harness.rs";
const RUBRIC: &str = "src/reviewer/rubric.rs";
const TRANSPORT: &str = "src/exec/agent/transport.rs";
const TRANSPORT_SESSION: &str = "src/exec/agent/transport/session.rs";

const MODEL_PROMPT_SHA256: &str =
    "7707c32309fe867f1897ae4d879a55793e19b83b2d0fdc5497777afb6f2fc8d9";
const HARNESS_SHA256: &str = "4fed353d75ecf43a47d85f03700248071fe905e388234fc40729e5b3d05c666c";
const RUBRIC_SHA256: &str = "c9ba2aca89c5e2183318636befa317597e6ffc827d768fe0bfe416e07b799b4f";
// Reviewed for #216. The prompt-byte dataflow gains ONE branch:
// `Framing::MusePromptFile` writes the rendered prompt to a 0600 file and names
// it with `--prompt-file`, because `muse exec` reads no prompt from STDIN and
// refuses `/dev/stdin`. The bytes still leave `ModelPrompt` through the same
// permit and reach exactly one `deliver_with_stdin` call; the file holds the
// prompt and is unlinked on drop, and only its PATH reaches argv.
const TRANSPORT_SHA256: &str = "58e2d6e69ba5f94326ffcff2aec6a9f659f6b6a56f27e99e3037d19acf7c6c11";
const TRANSPORT_SESSION_SHA256: &str =
    "585bbba3f6c211c8c452cfe347c687499455b6e0046faa513f624b5a245870ff";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn source(path: &str) -> String {
    fs::read_to_string(repo().join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn raw_sha256(source: &str) -> String {
    hex::encode(Sha256::digest(source.as_bytes()))
}

fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| {
            segment
                .ident
                .to_string()
                .trim_start_matches("r#")
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("::")
}

fn type_mentions_prompt(ty: &syn::Type) -> bool {
    struct Finder(bool);
    impl<'ast> Visit<'ast> for Finder {
        fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
            if ty
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "ModelPrompt")
            {
                self.0 = true;
            }
            syn::visit::visit_type_path(self, ty);
        }
    }
    let mut finder = Finder(false);
    finder.visit_type(ty);
    finder.0
}

fn returns_prompt(output: &syn::ReturnType) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if type_mentions_prompt(ty))
}

#[derive(Default)]
struct PromptValueVisitor {
    owner: String,
    implementation: String,
    events: Vec<String>,
}

impl PromptValueVisitor {
    fn with_owner(&mut self, owner: String, visit: impl FnOnce(&mut Self)) {
        let previous = std::mem::replace(&mut self.owner, owner);
        visit(self);
        self.owner = previous;
    }
}

impl<'ast> Visit<'ast> for PromptValueVisitor {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let name = item.sig.ident.to_string();
        if returns_prompt(&item.sig.output) {
            self.events.push(format!("free-return:{name}"));
        }
        self.with_owner(name, |this| syn::visit::visit_item_fn(this, item));
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let previous = std::mem::replace(
            &mut self.implementation,
            match item.self_ty.as_ref() {
                syn::Type::Path(path) => path_name(&path.path),
                _ => "<other>".to_owned(),
            },
        );
        syn::visit::visit_item_impl(self, item);
        self.implementation = previous;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let name = item.sig.ident.to_string();
        if returns_prompt(&item.sig.output) {
            self.events.push(format!("associated-return:{name}"));
        }
        self.with_owner(name, |this| syn::visit::visit_impl_item_fn(this, item));
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        if expression
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "ModelPrompt")
        {
            self.events.push(format!(
                "construct:{}:{}",
                self.owner,
                path_name(&expression.path)
            ));
        }
        syn::visit::visit_expr_struct(self, expression);
    }

    fn visit_expr_field(&mut self, expression: &'ast syn::ExprField) {
        if matches!(&expression.member, syn::Member::Named(member) if member == "rendered")
            && self.implementation != "ModelPromptBuilder"
        {
            self.events.push(format!("field-read:{}", self.owner));
        }
        syn::visit::visit_expr_field(self, expression);
    }

    fn visit_pat_struct(&mut self, pattern: &'ast syn::PatStruct) {
        if pattern
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "ModelPrompt")
        {
            self.events.push(format!(
                "destructure:{}:{}",
                self.owner,
                path_name(&pattern.path)
            ));
        }
        syn::visit::visit_pat_struct(self, pattern);
    }
}

fn prompt_value_events(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("parse prompt-boundary source");
    let mut visitor = PromptValueVisitor::default();
    visitor.visit_file(&file);
    visitor.events.sort();
    visitor.events
}

fn declared_modules(source: &str) -> Vec<(String, bool)> {
    let file = syn::parse_file(source).expect("parse prompt module declarations");
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) => Some((
                module.ident.to_string(),
                module.attrs.iter().any(|attribute| {
                    attribute.path().is_ident("cfg")
                        && matches!(
                            &attribute.meta,
                            syn::Meta::List(list) if list.tokens.to_string() == "test"
                        )
                }),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn model_prompt_construction_and_rendered_field_access_have_a_closed_inventory() {
    assert_eq!(
        prompt_value_events(&source(MODEL_PROMPT)),
        [
            "associated-return:finish".to_owned(),
            "associated-return:finish_for".to_owned(),
            "construct:finish:ModelPrompt".to_owned(),
            "field-read:as_str".to_owned(),
            "field-read:is_empty".to_owned(),
            "field-read:len".to_owned(),
        ]
    );
    assert!(prompt_value_events(&source(HARNESS)).is_empty());
}

#[test]
fn model_prompt_descendants_have_a_closed_inventory() {
    assert_eq!(
        declared_modules(&source(MODEL_PROMPT)),
        [("harness".to_owned(), false), ("tests".to_owned(), true)],
        "a new model_prompt descendant inherits access to private rendered bytes and requires review"
    );
    assert!(
        declared_modules(&source(HARNESS)).is_empty(),
        "a harness descendant inherits access to the closed trusted vocabulary"
    );
}

#[test]
fn free_constructor_accessor_and_destructure_seeds_are_visible() {
    let seed = r#"
        struct ModelPrompt { rendered: String }
        fn raw(value: String) -> ModelPrompt { ModelPrompt { rendered: value } }
        fn expose(prompt: &ModelPrompt) -> &str { &prompt.rendered }
        fn take(prompt: ModelPrompt) { let ModelPrompt { rendered: _ } = prompt; }
    "#;
    let events = prompt_value_events(seed);
    for expected in [
        "construct:raw:ModelPrompt",
        "destructure:take:ModelPrompt",
        "field-read:expose",
        "free-return:raw",
    ] {
        assert!(
            events.iter().any(|event| event == expected),
            "prompt escape {expected} was invisible: {events:?}"
        );
    }
}

#[test]
fn transport_harness_and_trusted_tables_match_the_reviewed_bytes() {
    for (path, expected) in [
        (MODEL_PROMPT, MODEL_PROMPT_SHA256),
        (HARNESS, HARNESS_SHA256),
        (RUBRIC, RUBRIC_SHA256),
        (TRANSPORT, TRANSPORT_SHA256),
        (TRANSPORT_SESSION, TRANSPORT_SESSION_SHA256),
    ] {
        assert_eq!(
            raw_sha256(&source(path)),
            expected,
            "{path} changed; re-review its complete prompt-byte dataflow and update the fingerprint explicitly"
        );
    }
}

#[test]
fn transport_success_is_an_os_stdin_handoff_not_a_provider_consumption_claim() {
    let transport = source(TRANSPORT);
    assert!(
        transport.contains("accepted by the OS stdin stream"),
        "the transport must state the positive evidence its successful write establishes"
    );
    assert!(
        transport.contains("does not prove that the provider consumed or parsed those bytes"),
        "a buffered write cannot be described as proof of provider consumption"
    );
}

#[test]
fn conditional_transport_and_dynamic_harness_mutations_change_the_inventory() {
    let transport = source(TRANSPORT);
    assert_eq!(
        transport
            .matches("Framing::Plain => Cow::Borrowed(rendered)")
            .count(),
        1
    );
    let conditional = transport.replacen(
        "Framing::Plain => Cow::Borrowed(rendered)",
        "Framing::Plain => Cow::Borrowed(\"raw attacker payload\")",
        1,
    );
    assert_ne!(conditional, transport);
    assert_ne!(raw_sha256(&conditional), TRANSPORT_SHA256);

    let harness = source(HARNESS);
    assert_eq!(harness.matches("pub(crate) enum HarnessText {").count(), 1);
    let dynamic = harness.replacen(
        "pub(crate) enum HarnessText {",
        "pub(crate) enum HarnessText { Raw(String),",
        1,
    );
    assert_ne!(dynamic, harness);
    assert_ne!(raw_sha256(&dynamic), HARNESS_SHA256);
}

#[test]
fn transport_descendants_have_a_closed_inventory() {
    assert_eq!(
        declared_modules(&source(TRANSPORT)),
        [("session".to_owned(), false), ("tests".to_owned(), true)]
    );
    assert_eq!(
        declared_modules(&source(TRANSPORT_SESSION)),
        [("tests".to_owned(), true)]
    );
}

#[test]
fn extracted_session_output_mutation_changes_the_inventory() {
    let session = source(TRANSPORT_SESSION);
    assert_eq!(raw_sha256(&session), TRANSPORT_SESSION_SHA256);
    assert_eq!(session.matches("Ok((stdout, stderr))").count(), 1);
    let swapped = session.replacen("Ok((stdout, stderr))", "Ok((stderr, stdout))", 1);
    assert_ne!(swapped, session);
    assert_ne!(raw_sha256(&swapped), TRANSPORT_SESSION_SHA256);
}

#[test]
fn harness_variants_carry_only_the_two_reviewed_numeric_tokens() {
    let file = syn::parse_file(&source(HARNESS)).expect("parse harness vocabulary");
    let vocabulary = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(item) if item.ident == "HarnessText" => Some(item),
            _ => None,
        })
        .expect("HarnessText enum");
    for variant in &vocabulary.variants {
        match (variant.ident.to_string().as_str(), &variant.fields) {
            ("ReviewerAspect" | "ReviewerStance", syn::Fields::Unnamed(fields)) => {
                assert_eq!(fields.unnamed.len(), 1);
                assert!(
                    matches!(&fields.unnamed[0].ty, syn::Type::Path(path) if path.path.is_ident("usize"))
                );
            }
            (_, syn::Fields::Unit) => {}
            (name, _) => panic!("unreviewed dynamic HarnessText payload {name}"),
        }
    }
}

#[test]
fn rubric_tables_are_literal_only() {
    fn literal_tree(expression: &syn::Expr) -> bool {
        match expression {
            syn::Expr::Array(array) => array.elems.iter().all(literal_tree),
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(_),
                ..
            }) => true,
            syn::Expr::Reference(reference) => literal_tree(&reference.expr),
            syn::Expr::Tuple(tuple) => tuple.elems.iter().all(literal_tree),
            _ => false,
        }
    }

    let file = syn::parse_file(&source(RUBRIC)).expect("parse reviewer rubric");
    for name in ["REVIEW_ASPECTS", "REVIEW_STANCES"] {
        let expression = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Const(item) if item.ident == name => Some(item.expr.as_ref()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing trusted table {name}"));
        assert!(
            literal_tree(expression),
            "{name} gained dynamic prompt text"
        );
    }
}

#[test]
fn fingerprints_are_token_parseable_rust_not_an_opaque_text_fixture() {
    for path in [MODEL_PROMPT, HARNESS, RUBRIC, TRANSPORT, TRANSPORT_SESSION] {
        source(path)
            .parse::<TokenStream>()
            .unwrap_or_else(|error| panic!("{path} is not Rust tokens: {error}"));
    }
}
