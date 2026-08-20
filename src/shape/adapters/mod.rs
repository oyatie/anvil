//! Adapters: concrete tree sources. `InMemoryTree` for fixtures and as the
//! value every loader produces; `GitTreeAtRev` reads a named revision through
//! git plumbing, never a checkout (D7, I3).

pub mod git_tree_at_rev;
pub mod in_memory_tree;

pub use git_tree_at_rev::GitTreeAtRev;
pub use in_memory_tree::InMemoryTree;
