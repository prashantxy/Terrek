//! What Terrek knows about where the user is: OS, shell, directory, project,
//! git state, and recent commands. Rendered into every AI prompt.

pub mod context_builder;
pub mod git;
pub mod project;
pub mod state;

pub use state::ContextState;
