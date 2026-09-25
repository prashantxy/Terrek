//! Terrek: an AI layer for the shell you already use.
//!
//! `terrek` wraps your login shell in a PTY, records every command (text, exit
//! code, cwd, duration, output tail) through lightweight shell hooks, and lets
//! you ask an AI provider about it without leaving the prompt.

pub mod ai;
pub mod cli;
pub mod commands;
pub mod config;
pub mod context;
pub mod db;
pub mod suggestion_engine;
pub mod terminal;

#[cfg(unix)]
pub mod pty;
#[cfg(unix)]
pub mod sessions;

/// Environment variable set inside every shell Terrek spawns.
pub const SESSION_ENV: &str = "TERREK_SESSION";
