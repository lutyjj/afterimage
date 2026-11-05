//! What TRMNL firmware sends and how it reads the answers, independent of any HTTP
//! stack or storage. The firmware source is the authority where TRMNL's published
//! API document is silent or differs; each item names the behaviour it relies on.

mod mac;
mod readings;
mod status;

pub use mac::{Mac, MacError};
pub use readings::Readings;
pub use status::{DisplayStatus, SetupStatus};
