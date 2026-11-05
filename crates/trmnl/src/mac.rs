use std::fmt;
use std::str::FromStr;

/// A device's MAC address, the identity it presents in the `ID` header.
///
/// The firmware sends `AA:BB:CC:DD:EE:FF`; parsing also takes lowercase digits and
/// `-` separators, and the canonical form is uppercase with colons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mac([u8; 6]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacError;

impl fmt::Display for MacError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a MAC address of six hexadecimal pairs")
    }
}

impl std::error::Error for MacError {}

impl Mac {
    #[must_use]
    pub const fn octets(self) -> [u8; 6] {
        self.0
    }
}

impl FromStr for Mac {
    type Err = MacError;

    fn from_str(text: &str) -> Result<Self, MacError> {
        let separator = if text.contains('-') { '-' } else { ':' };
        let mut octets = [0; 6];
        let mut parts = text.trim().split(separator);
        for octet in &mut octets {
            let part = parts.next().ok_or(MacError)?;
            // from_str_radix also takes a sign, as in `+1`.
            if part.len() != 2 || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(MacError);
            }
            *octet = u8::from_str_radix(part, 16).map_err(|_| MacError)?;
        }
        if parts.next().is_some() {
            return Err(MacError);
        }
        Ok(Self(octets))
    }
}

impl fmt::Display for Mac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, octet) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(":")?;
            }
            write!(f, "{octet:02X}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_firmware_form_and_writes_it_back() {
        let mac: Mac = "41:B4:10:39:A1:24".parse().unwrap();
        assert_eq!(mac.octets(), [0x41, 0xB4, 0x10, 0x39, 0xA1, 0x24]);
        assert_eq!(mac.to_string(), "41:B4:10:39:A1:24");
    }

    #[test]
    fn accepts_lowercase_and_dashes() {
        let mac: Mac = "41-b4-10-39-a1-24".parse().unwrap();
        assert_eq!(mac.to_string(), "41:B4:10:39:A1:24");
    }

    #[test]
    fn rejects_anything_but_six_pairs() {
        for text in [
            "",
            "41:B4:10:39:A1",
            "41:B4:10:39:A1:24:00",
            "41:B4:10:39:A1:2",
            "41:B4:10:39:A1:ZZ",
            "41B410:39:A1:24",
            "+1:+2:+3:+4:+5:+6",
        ] {
            assert_eq!(text.parse::<Mac>(), Err(MacError), "{text:?}");
        }
    }
}
