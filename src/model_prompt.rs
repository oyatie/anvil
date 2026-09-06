//! A model prompt whose authorship boundary is enforced by its constructors.
//!
//! Model transports accept this type rather than `&str`. Callers can append
//! one of the finite harness fragments defined here, narrowly validated
//! metadata, or an [`Untrusted`] segment. There is deliberately no constructor
//! from `String` or `&'static str`, no dynamic harness-text arm, and no public
//! accessor for the rendered bytes.

use anyhow::{Result, bail};

use crate::reviewer::untrusted::Untrusted;

/// Hard ceiling for the complete rendered prompt, including trusted harness
/// prose, metadata, fences, truncation declarations, and contributor data.
///
/// Per-channel caps remain responsible for selecting truthful excerpts. This
/// aggregate cap prevents a caller from defeating them by repeating otherwise
/// valid segments. Builders fail closed on overflow; the prompt is never
/// sliced, so a final task/schema and an untrusted frame are either present in
/// full or the whole prompt is rejected.
pub const MAX_MODEL_PROMPT_BYTES: usize = 256 * 1024;

/// Closed trusted task available to external probe callers.
///
/// Production prompt builders use their own crate-private terminal harness
/// variants. This one narrow purpose preserves subscription smoke tests without
/// reopening arbitrary trusted `String` construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPromptPurpose {
    SubscriptionProbe,
}

/// The only value a model transport accepts as a prompt.
///
/// Raw strings cannot cross the model sink:
///
/// ```compile_fail
/// use anvil::model_prompt::ModelPrompt;
/// let dynamic = String::from("unchecked contributor text");
/// let _prompt: ModelPrompt = dynamic.into();
/// ```
///
/// Leaking a runtime string does not turn it into trusted harness text. A
/// `'static` lifetime proves only how long a value lives, not who authored it:
///
/// ```compile_fail
/// use anvil::model_prompt::ModelPrompt;
/// let dynamic = String::from("unchecked contributor text");
/// let leaked: &'static str = Box::leak(dynamic.into_boxed_str());
/// let mut builder = ModelPrompt::builder();
/// builder.push_static(leaked);
/// ```
///
/// The provider executor itself requires the opaque type:
///
/// ```compile_fail
/// use anvil::ai_driver::{ModelExecutionConfig, SubscriptionExecutor};
/// fn raw_prompt_does_not_typecheck(executor: &SubscriptionExecutor) {
///     let _ = executor.execute_prompt(
///         "unchecked",
///         std::path::Path::new("."),
///         &ModelExecutionConfig::default(),
///     );
/// }
/// ```
pub struct ModelPrompt {
    rendered: String,
}

impl ModelPrompt {
    /// Starts a prompt assembled from classified pieces.
    pub fn builder() -> ModelPromptBuilder {
        ModelPromptBuilder {
            rendered: String::new(),
            overflowed: false,
            terminal: false,
        }
    }

    /// The rendered bytes are visible only to the sealed model transport,
    /// which owns the unconstructible permit accepted here.
    pub(crate) fn as_str(&self, _permit: &crate::exec::agent::ModelPromptPermit) -> &str {
        &self.rendered
    }

    /// The size is safe to expose for budgeting without exposing prompt bytes.
    pub fn len(&self) -> usize {
        self.rendered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rendered.is_empty()
    }
}

/// Assembly of a [`ModelPrompt`] from pieces with known authorship.
pub struct ModelPromptBuilder {
    rendered: String,
    overflowed: bool,
    terminal: bool,
}

mod harness;
pub(crate) use harness::HarnessText;

impl ModelPromptBuilder {
    fn push_fragment(&mut self, fragment: &str, terminal: bool) {
        self.terminal = false;
        if self.overflowed {
            return;
        }
        let Some(total) = self.rendered.len().checked_add(fragment.len()) else {
            self.overflowed = true;
            return;
        };
        if total > MAX_MODEL_PROMPT_BYTES {
            self.overflowed = true;
            return;
        }
        self.rendered.push_str(fragment);
        self.terminal = terminal;
    }

    /// Appends one member of the closed harness-authored vocabulary.
    pub(crate) fn push_harness(&mut self, text: HarnessText) -> &mut Self {
        let terminal = text.is_terminal();
        let mut fragment = String::new();
        text.append_to(&mut fragment);
        self.push_fragment(&fragment, terminal);
        self
    }

    /// Appends an integer, which cannot contain prompt structure.
    pub fn push_u64(&mut self, value: u64) -> &mut Self {
        self.push_fragment(&value.to_string(), false);
        self
    }

    /// Appends an index, which cannot contain prompt structure.
    pub fn push_usize(&mut self, value: usize) -> &mut Self {
        self.push_fragment(&value.to_string(), false);
        self
    }

    /// Appends a GitHub `owner/repository` identifier after validating its
    /// grammar. Arbitrary dynamic text cannot use this path.
    pub fn push_repository(&mut self, repository: &str) -> Result<&mut Self> {
        let mut pieces = repository.split('/');
        let owner = pieces.next().unwrap_or_default();
        let name = pieces.next().unwrap_or_default();
        let valid_piece = |piece: &str| {
            !piece.is_empty()
                && piece
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        };
        if pieces.next().is_some() || !valid_piece(owner) || !valid_piece(name) {
            bail!("invalid repository identifier for model metadata: {repository:?}");
        }
        self.push_fragment(repository, false);
        Ok(self)
    }

    /// Appends a hexadecimal commit identifier after validating its grammar.
    pub fn push_commit_sha(&mut self, sha: &str) -> Result<&mut Self> {
        if !(7..=64).contains(&sha.len()) || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("invalid commit SHA for model metadata: {sha:?}");
        }
        self.push_fragment(sha, false);
        Ok(self)
    }

    /// Appends contributor-controlled text through its only rendering path.
    pub fn push_untrusted(&mut self, value: Untrusted<'_>) -> &mut Self {
        if self.overflowed {
            return self;
        }
        self.push_fragment(&value.render(), false);
        self
    }

    /// Completes an externally authored subscription smoke probe with a
    /// finite harness-owned task. The consuming API prevents appending data
    /// after that terminal task.
    pub fn finish_for(mut self, purpose: ModelPromptPurpose) -> Result<ModelPrompt> {
        match purpose {
            ModelPromptPurpose::SubscriptionProbe => {
                self.push_harness(HarnessText::SubscriptionProbeTask);
            }
        }
        self.finish()
    }

    /// Completes the prompt only when every requested fragment fit in full.
    ///
    /// Overflow is sticky so callers may keep using the fluent API, but no
    /// partial prompt can cross a sink. Empty prompts are rejected as absent
    /// model instructions rather than being transported as a valid turn.
    pub fn finish(self) -> Result<ModelPrompt> {
        if self.overflowed {
            bail!(
                "model prompt exceeds aggregate rendered-byte ceiling of {}",
                MAX_MODEL_PROMPT_BYTES
            );
        }
        if self.rendered.is_empty() {
            bail!("model prompt must not be empty");
        }
        if !self.terminal {
            bail!("model prompt must end with a trusted terminal task or response schema");
        }
        Ok(ModelPrompt {
            rendered: self.rendered,
        })
    }
}

#[cfg(test)]
mod tests;
