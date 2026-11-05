//! The server's time zone: playlists follow it, templates format dates in it, and
//! the web UI shows times in it, so it always has an IANA name.

#[derive(Debug, Clone)]
pub struct TimeZone {
    zone: jiff::tz::TimeZone,
    name: String,
}

impl TimeZone {
    /// The zone of this IANA name, such as `Europe/Berlin`.
    ///
    /// # Errors
    ///
    /// When the bundled database has no such zone.
    pub fn named(name: &str) -> Result<Self, jiff::Error> {
        Ok(Self {
            zone: jiff::tz::TimeZone::get(name)?,
            name: name.to_owned(),
        })
    }

    /// The host's zone; `None` when the host gives it no name, as with a copied
    /// rather than linked `/etc/localtime`.
    #[must_use]
    pub fn system() -> Option<Self> {
        let zone = jiff::tz::TimeZone::system();
        let name = zone.iana_name()?.to_owned();
        Some(Self { zone, name })
    }

    #[must_use]
    pub fn zone(&self) -> &jiff::tz::TimeZone {
        &self.zone
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
