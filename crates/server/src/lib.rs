//! The Afterimage server: TRMNL's device API over SQLite.
//!
//! [`registrations::Registrations`] owns setups, keys and their storage;
//! [`device_api`] adapts the generated device API to it.

pub mod clock;
pub mod config;
pub mod device_api;
pub mod registrations;
pub mod secrets;
