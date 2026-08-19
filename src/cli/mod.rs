//! Command-line Interface & Subcommand Handlers

pub mod args;
pub mod handlers;
pub mod server;

pub use args::{Cli, Commands};
pub use handlers::handle_cli;
