/// The device state a check-in reports, read from header text.
///
/// Firmware builds disagree with the published types (`Battery-Capacity` arrives as
/// `3000/3500`, `Battery-Charging` as `1`), and a reading that does not parse or is
/// out of physical range is unknown rather than a reason to refuse the check-in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Readings {
    /// Volts, from `Battery-Voltage`.
    pub battery_voltage: Option<f64>,
    /// 0 to 100, from `Percent-Charged`, or else estimated from the voltage by
    /// [`percent_charged`].
    pub battery_percent: Option<f64>,
    /// dBm, from `RSSI`.
    pub signal_dbm: Option<i32>,
    /// From `FW-Version`, such as `1.8.18`.
    pub firmware_version: Option<String>,
}

/// Longest firmware version kept; release tags are a few characters.
const VERSION_LENGTH: usize = 32;

impl Readings {
    /// Reads each header's text when it is present. Only TRMNL X sends
    /// `Percent-Charged` (`buildDisplayHeaders`), and it sends -1 when its gauge
    /// fails, so every other reading of the charge comes from the voltage.
    #[must_use]
    pub fn parse(
        battery_voltage: Option<&str>,
        battery_percent: Option<&str>,
        signal_dbm: Option<&str>,
        firmware_version: Option<&str>,
    ) -> Self {
        let battery_voltage = battery_voltage.and_then(|text| number(text, 0.0, 10.0));
        Self {
            battery_voltage,
            battery_percent: battery_percent
                .and_then(|text| number(text, 0.0, 100.0))
                .or_else(|| battery_voltage.map(percent_charged)),
            signal_dbm: signal_dbm
                .and_then(|text| text.trim().parse::<i32>().ok())
                .filter(|dbm| (-150..=0).contains(dbm)),
            firmware_version: firmware_version.and_then(version),
        }
    }
}

/// TRMNL's estimate of a battery's charge from its voltage: 3.0 V is empty and
/// every 0.012 V above it a percent, with steps near the top, where a full charge
/// settles around 4.08 V, and a floor of 1%. Firmware 1.8.18 applies it as the
/// server's `percent_charged_calculation` when its gauge is bypassed
/// (`src/battery/bq27427_battery.cpp`).
fn percent_charged(volts: f64) -> f64 {
    let percent = (volts - 3.0) / 0.012;
    if percent >= 88.0 {
        100.0
    } else if percent >= 85.0 {
        95.0
    } else if percent >= 83.0 {
        90.0
    } else if percent >= 10.0 {
        percent.round()
    } else {
        1.0
    }
}

/// The panel size from the `Width` and `Height` headers, when both read as pixels.
#[must_use]
pub fn panel_size(width: Option<&str>, height: Option<&str>) -> Option<(u32, u32)> {
    let pixels = |text: Option<&str>| {
        text?
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|pixels| (1..=10_000).contains(pixels))
    };
    Some((pixels(width)?, pixels(height)?))
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

    /// Firmware 1.6.10 sends no `Percent-Charged`, nor does 1.8.18 on any board
    /// but TRMNL X, whose gauge sends -1 when it fails.
    #[test]
    fn estimates_the_charge_from_the_voltage_when_none_is_reported() {
        let percent = |volts: &str, reported: Option<&str>| {
            Readings::parse(Some(volts), reported, None, None).battery_percent
        };
        assert_eq!(percent("3.9", None), Some(75.0));
        assert_eq!(percent("3.9", Some("-1")), Some(75.0));
        assert_eq!(percent("3.9", Some("52")), Some(52.0));
        assert_eq!(
            Readings::parse(None, None, None, None).battery_percent,
            None
        );
    }

    /// Worked through TRMNL's formula: (volts - 3.0) / 0.012.
    #[test]
    fn follows_trmnls_steps_near_full_and_its_floor() {
        for (volts, percent) in [
            (4.2, 100.0),
            (4.06, 100.0), // 88.3
            (4.03, 95.0),  // 85.8
            (4.0, 90.0),   // 83.3
            (3.98, 82.0),  // 81.7
            (3.5, 42.0),   // 41.7
            (3.13, 11.0),  // 10.8
            (3.11, 1.0),   // 9.2
            (0.0, 1.0),
        ] {
            assert_eq!(percent_charged(volts), percent, "{volts} V");
        }
    }

    #[test]
    fn reads_the_panel_size_when_both_sides_are_pixels() {
        assert_eq!(panel_size(Some("800"), Some(" 480")), Some((800, 480)));
        assert_eq!(panel_size(Some("800"), None), None);
        assert_eq!(panel_size(Some("0"), Some("480")), None);
        assert_eq!(panel_size(Some("-800"), Some("480")), None);
        assert_eq!(panel_size(Some("800px"), Some("480")), None);
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
