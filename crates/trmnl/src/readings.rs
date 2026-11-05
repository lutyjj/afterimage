/// The device state a check-in reports, read from header text.
///
/// Firmware builds disagree with the published types (`Battery-Capacity` arrives as
/// `3000/3500`, `Battery-Charging` as `1`), and a reading that does not parse or is
/// out of physical range is unknown rather than a reason to refuse the check-in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Readings {
    /// Volts, from `Battery-Voltage`.
    pub battery_voltage: Option<f64>,
    /// 0 to 100, from `Percent-Charged`.
    pub battery_percent: Option<f64>,
    /// dBm, from `RSSI`.
    pub signal_dbm: Option<i32>,
    /// From `FW-Version`, such as `1.8.18`.
    pub firmware_version: Option<String>,
}

/// Longest firmware version kept; release tags are a few characters.
const VERSION_LENGTH: usize = 32;

impl Readings {
    /// Reads each header's text when it is present.
    #[must_use]
    pub fn parse(
        battery_voltage: Option<&str>,
        battery_percent: Option<&str>,
        signal_dbm: Option<&str>,
        firmware_version: Option<&str>,
    ) -> Self {
        Self {
            battery_voltage: battery_voltage.and_then(|text| number(text, 0.0, 10.0)),
            battery_percent: battery_percent.and_then(|text| number(text, 0.0, 100.0)),
            signal_dbm: signal_dbm
                .and_then(|text| text.trim().parse::<i32>().ok())
                .filter(|dbm| (-150..=0).contains(dbm)),
            firmware_version: firmware_version.and_then(version),
        }
    }
}

fn number(text: &str, low: f64, high: f64) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && (low..=high).contains(value))
}

fn version(text: &str) -> Option<String> {
    let text = text.trim();
    let printable = text.chars().all(|c| c.is_ascii_graphic());
    (!text.is_empty() && text.len() <= VERSION_LENGTH && printable).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_firmware_sends() {
        let readings = Readings::parse(Some("4.12"), Some("69.4"), Some("-61"), Some("1.8.18"));
        assert_eq!(
            readings,
            Readings {
                battery_voltage: Some(4.12),
                battery_percent: Some(69.4),
                signal_dbm: Some(-61),
                firmware_version: Some("1.8.18".into()),
            }
        );
    }

    #[test]
    fn treats_absent_malformed_and_impossible_values_as_unknown() {
        assert_eq!(Readings::parse(None, None, None, None), Readings::default());
        let readings = Readings::parse(Some("3000/3500"), Some("NaN"), Some("12"), Some(" "));
        assert_eq!(readings, Readings::default());
        let readings =
            Readings::parse(Some("-1"), Some("101"), Some("-200"), Some(&"9".repeat(33)));
        assert_eq!(readings, Readings::default());
    }

    #[test]
    fn rejects_control_characters_in_versions() {
        assert_eq!(
            Readings::parse(None, None, None, Some("1.8\n")).firmware_version,
            Some("1.8".into())
        );
        assert_eq!(
            Readings::parse(None, None, None, Some("1.\u{1b}8")).firmware_version,
            None
        );
    }
}
