//! Messages exchanged between the engine-side AI shim and the bot process, and their framing.
//!
//! The exchange is credit-based (see `DESIGN.md`): after [`ToBot::Hello`] the bot sends one
//! [`Commands`] per [`Tick`] it receives, and the shim sends the next tick only once it has one.

mod framing;
mod messages;

pub use framing::{FrameReader, write_frame};
pub use messages::*;

use std::path::PathBuf;

/// Socket the bot listens on: `$WITHIN_REASON_SOCKET`, else `$XDG_RUNTIME_DIR/within-reason.sock`.
pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("WITHIN_REASON_SOCKET") {
        return PathBuf::from(path);
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    dir.join("within-reason.sock")
}
