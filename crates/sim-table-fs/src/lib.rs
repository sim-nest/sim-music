#![forbid(unsafe_code)]
#![allow(deprecated)]
#![deny(missing_docs)]
//! Filesystem-backed table store for SIM.
//!
//! This crate exposes a host directory as a SIM table: each table key maps to a
//! file, and nested tables map to subdirectories. Reads and writes are gated by
//! the kernel's table-fs capabilities and encoded through the configured codec.
//! With the optional format features enabled, recognized extensions (for
//! example `.mid`, `.music`, `.tone`, `.scl`, `.ly`) round-trip through their
//! domain shapes.

mod citizen;
mod fs_dir;
mod roadmap11;

pub use citizen::{FsDirDescriptor, fs_dir_class_symbol};
pub use fs_dir::{FsDir, install_fs_dir_lib};

#[cfg(test)]
mod tests;
