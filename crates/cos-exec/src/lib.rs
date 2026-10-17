//! Host execution owner. Command policy is a launch filter, not an OS sandbox.
pub mod batch;
pub mod buffer;
pub mod policy;
pub mod process;
pub mod shell;
pub mod ssh;
pub mod terminal;
pub mod tree;
#[cfg(windows)]
mod winpty;
