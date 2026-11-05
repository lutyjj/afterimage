//! What TRMNL firmware sends and how it reads the answers, independent of any HTTP
//! stack or storage, and TRMNL's catalogue of display models and palettes. The
//! firmware source is the authority where TRMNL's published API document is silent
//! or differs; each item names the behaviour it relies on.

pub mod catalog;
mod mac;
mod readings;
mod status;

pub use mac::{Mac, MacError};
pub use readings::{Readings, panel_size};
pub use status::{DisplayStatus, SetupStatus};
