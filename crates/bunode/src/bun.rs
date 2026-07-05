//! This module is used to call `bun` binary.
//! This module should only include `bun` binary finding, and exported `bun` function.
//! Any wrapper logic (translate, argv generation) should be put outside of this module.
//!
//! Core function: `bun` function, it receives argvs you push to Bun.

use std::ffi::OsString;

#[allow(dead_code)]
pub const fn bun(_args: &[OsString]) {}
