//! The server's time zone: schedules follow it, templates format dates in it, and
//! the web UI shows times in it, so it always has an IANA name.

#[derive(Debug, Clone)]
pub struct TimeZone {
    zone: jiff::tz::TimeZone,
    name: String,
}

impl TimeZone {
    /// The zone of this IANA name, such as `Europe/Berlin`, under the name the
    /// database spells it with, whatever case it was given in.
    ///
    /// # Errors
    ///
    /// When the bundled database has no such zone.
    pub fn named(name: &str) -> Result<Self, jiff::Error> {
        let zone = jiff::tz::TimeZone::get(name)?;
        let name = zone.iana_name().unwrap_or(name).to_owned();
        Ok(Self { zone, name })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zone_takes_the_databases_spelling() {
        assert_eq!(
            TimeZone::named("europe/berlin").unwrap().name(),
            "Europe/Berlin"
        );
    }
}
