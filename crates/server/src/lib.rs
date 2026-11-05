//! The Afterimage server: TRMNL's device API and the management API over SQLite.
//!
//! [`app::build`] assembles it. [`registrations`] owns setups, keys and check-ins;
//! [`devices`], [`screens`] and [`accounts`] own what accounts hold; [`device_api`]
//! and [`management_api`] adapt the generated services to them.

pub mod accounts;
pub mod app;
pub mod clock;
pub mod config;
pub mod database;
pub mod device_api;
pub mod devices;
pub mod keys;
pub mod management_api;
pub mod oidc;
pub mod registrations;
pub mod screens;
pub mod secrets;
pub mod sessions;
