//! Interactive browser sessions for agent-style browsing.
//!
//! Each session lives on its own OS thread with a persistent Browser + Page.
//! Commands are dispatched via channels, results returned via oneshot.
//! Sessions auto-expire after 8 minutes of inactivity.
//!
//! Split (ARCHITECTURE.md P2): [`manager`] owns the actor and the
//! per-session thread, [`commands`] is the command protocol, [`state`]
//! holds handles/errors/snapshot backends, [`interact`] the page
//! interaction executors, [`record`] recording/replay. The re-exports
//! below are exactly the externally consumed face (`main.rs`,
//! `flow.rs`); the response types stay reachable inside the module via
//! their defining submodules.

mod commands;
mod interact;
mod manager;
pub(crate) mod preload_recipes;
mod record;
#[cfg(feature = "screenshot")]
mod screenshot;
pub(crate) mod snapshot;
mod state;
#[cfg(test)]
mod tests;

pub use commands::{
    CallOnRequest, EmulateOp, HandleParts, PageOp, SameDocumentKind, ScrollDirection,
    SessionCommand, SessionEvent,
};
pub(crate) use interact::{
    mouse_button_code, mouse_button_mask, mouse_down_js, mouse_move_js, mouse_up_js, INPUT_HELPERS,
};
pub(crate) use manager::{
    close_session_and_wait, send_command, send_command_readonly, send_screenshot,
};
pub use manager::{SessionManager, SESSIONS};
pub use record::replay_bash;
pub use state::{ConsoleFilter, SessionError};
