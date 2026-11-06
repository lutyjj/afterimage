//! TRMNL's firmware list (`GET /api/firmware/flash`): every board it builds
//! firmware for, and each board's releases as flash images.
//!
//! TRMNL also lists over-the-air images for its own hardware (`GET
//! /api/firmware`), by display model rather than by board. Their bytes are the
//! app images the server cuts from the flash images of the same boards, so the
//! flash list alone serves every board.

use std::sync::Arc;
use std::time::Duration;

use afterimage_management_api::model::{FirmwareBoardName, FirmwareVersion};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::clock::Clock;
use crate::outbound::Outbound;

/// How long a fetched list stands; TRMNL publishes a release every few weeks.
const FRESH_SECONDS: i64 = 6 * 60 * 60;
/// How long after a failed fetch the next waits, so that a server without
/// access to TRMNL does not hold every reader up.
const RETRY_SECONDS: i64 = 10 * 60;
const TIMEOUT: Duration = Duration::from_secs(15);
/// Largest list read; it is about 14 KB.
const LIST_BYTES: usize = 1024 * 1024;

/// A board TRMNL builds firmware for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// The list's `keyname`, such as `seeed_OG_DIY_Kit`.
    pub name: String,
    pub label: String,
    /// Such as `ESP32-S3`.
    pub chip_family: String,
    /// In the list's order, newest first.
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// As the firmware reports it in `FW-Version`, such as `1.8.18`. The list
    /// writes `FW1.8.18`.
    pub version: String,
    /// The flash image.
    pub url: String,
}

impl Board {
    /// The newest release that is not a variant: TRMNL publishes variants of a
    /// release, such as `1.7.8-sensors`, as its pre-release in semantic version
    /// terms.
    #[must_use]
    pub fn latest(&self) -> Option<&Release> {
        self.releases
            .iter()
            .filter_map(|release| {
                let version = semver::Version::parse(&release.version).ok()?;
                version.pre.is_empty().then_some((version, release))
            })
            .max_by(|(one, _), (other, _)| one.cmp(other))
            .map(|(_, release)| release)
    }

    #[must_use]
    pub fn release(&self, version: &str) -> Option<&Release> {
        self.releases
            .iter()
            .find(|release| release.version == version)
    }
}

/// The board TRMNL names like the model the firmware reports, when one is.
/// TRMNL publishes no other link from a firmware's `DEVICE_MODEL` to its boards:
/// the two names differ for most boards (`xiao_epaper_display` is built as
/// `seeed_OG_DIY_Kit`), and a board's model name changes between releases.
#[must_use]
pub fn named<'a>(boards: &'a [Board], firmware_model: &str) -> Option<&'a Board> {
    boards.iter().find(|board| board.name == firmware_model)
}

#[derive(Deserialize)]
struct List {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    models: Vec<ListedBoard>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedBoard {
    keyname: String,
    label: String,
    chip_family: String,
    versions: Vec<ListedRelease>,
}

#[derive(Deserialize)]
struct ListedRelease {
    version: String,
    url: String,
}

/// The boards of a list, leaving out names and versions the management API's
/// shapes do not admit.
fn parse(bytes: &[u8]) -> Result<Vec<Board>, String> {
    let list: List = serde_json::from_slice(bytes)
        .map_err(|error| format!("answered a list in another shape: {error}"))?;
    Ok(list
        .data
        .models
        .into_iter()
        .filter(|board| FirmwareBoardName::try_from(board.keyname.clone()).is_ok())
        .map(|board| Board {
            name: board.keyname,
            label: board.label,
            chip_family: board.chip_family,
            releases: board
                .versions
                .into_iter()
                .map(|release| Release {
                    version: release
                        .version
                        .strip_prefix("FW")
                        .unwrap_or(&release.version)
                        .to_owned(),
                    url: release.url,
                })
                .filter(|release| FirmwareVersion::try_from(release.version.clone()).is_ok())
                .collect(),
        })
        .collect())
}

/// The list, fetched when first needed and again once it is old.
pub struct Releases {
    url: String,
    http: Outbound,
    clock: Arc<dyn Clock>,
    /// Held across a fetch, so that callers wait for one fetch rather than start
    /// their own.
    cached: Mutex<Cached>,
}

#[derive(Default)]
struct Cached {
    boards: Option<(Arc<[Board]>, i64)>,
    /// The last failed fetch: when, and why.
    failure: Option<(i64, String)>,
}

impl Releases {
    /// `trmnl` is TRMNL's site, such as `https://trmnl.com`.
    #[must_use]
    pub fn new(trmnl: &str, clock: Arc<dyn Clock>) -> Self {
        Self {
            url: format!("{trmnl}/api/firmware/flash"),
            http: Outbound::new(TIMEOUT),
            clock,
            cached: Mutex::new(Cached::default()),
        }
    }

    /// TRMNL's boards: the list fetched within [`FRESH_SECONDS`], or else a new
    /// fetch, or else the last list fetched.
    ///
    /// # Errors
    ///
    /// Why the list could not be fetched, when none was before.
    pub async fn boards(&self) -> Result<Arc<[Board]>, String> {
        let mut cached = self.cached.lock().await;
        let now = self.clock.now();
        if let Some((boards, fetched_at)) = &cached.boards
            && now - fetched_at < FRESH_SECONDS
        {
            return Ok(Arc::clone(boards));
        }
        let failed_lately = cached
            .failure
            .as_ref()
            .filter(|(failed_at, _)| now - failed_at < RETRY_SECONDS);
        let failure = match failed_lately {
            Some((_, failure)) => failure.clone(),
            None => match self.fetch().await {
                Ok(boards) => {
                    let boards: Arc<[Board]> = boards.into();
                    cached.boards = Some((Arc::clone(&boards), now));
                    cached.failure = None;
                    return Ok(boards);
                }
                Err(failure) => {
                    tracing::warn!(%failure, "TRMNL's firmware list could not be read");
                    cached.failure = Some((now, failure.clone()));
                    failure
                }
            },
        };
        cached
            .boards
            .as_ref()
            .map(|(boards, _)| Arc::clone(boards))
            .ok_or(failure)
    }

    async fn fetch(&self) -> Result<Vec<Board>, String> {
        let request = self
            .http
            .client()
            .get(&self.url)
            .header("Accept", "application/json");
        parse(&self.http.read(request, LIST_BYTES).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An excerpt of TRMNL's list as it reads on 2026-11-06.
    const LIST: &str = r#"{"data":{"models":[
        {"keyname":"trmnl","chipFamily":"ESP32-C3","label":"TRMNL OG (native)","versions":[
            {"version":"FW1.8.18","url":"https://trmnl-fw.example.test/trmnl_og/flash/FW1.8.18.bin","elf_url":"https://trmnl-fw.example.test/trmnl_og/flash/FW1.8.18.elf"},
            {"version":"FW1.7.8-sensors","url":"https://trmnl-fw.example.test/trmnl_og/flash/FW1.7.8-sensors.bin"},
            {"version":"FW1.7.8","url":"https://trmnl-fw.example.test/trmnl_og/flash/FW1.7.8.bin"}]},
        {"keyname":"xteink_x4","chipFamily":"ESP32-C3","label":"Xteink X4","versions":[
            {"version":"FW1.8.18-push-to-refresh","url":"https://trmnl-fw.example.test/byod/xteink_x4/flash/FW1.8.18-push-to-refresh.bin"},
            {"version":"FW1.8.18","url":"https://trmnl-fw.example.test/byod/xteink_x4/flash/FW1.8.18.bin"},
            {"version":"FW1.8.9","url":"https://trmnl-fw.example.test/byod/xteink_x4/flash/FW1.8.9.bin"}]},
        {"keyname":"a board/with a path","chipFamily":"ESP32","label":"Not a name","versions":[]}
    ]}}"#;

    #[test]
    fn reads_boards_and_releases_as_the_firmware_names_them() {
        let boards = parse(LIST.as_bytes()).unwrap();
        assert_eq!(
            boards.len(),
            2,
            "a keyname the API cannot carry is left out"
        );
        let og = &boards[0];
        assert_eq!(
            (og.name.as_str(), og.label.as_str(), og.chip_family.as_str()),
            ("trmnl", "TRMNL OG (native)", "ESP32-C3")
        );
        assert_eq!(og.releases[0].version, "1.8.18");
        assert_eq!(
            og.releases[0].url,
            "https://trmnl-fw.example.test/trmnl_og/flash/FW1.8.18.bin"
        );
        assert_eq!(
            og.release("1.7.8-sensors").unwrap().version,
            "1.7.8-sensors"
        );
        assert_eq!(og.release("1.6.0"), None);
    }

    #[test]
    fn the_latest_release_is_the_newest_that_is_no_variant() {
        let boards = parse(LIST.as_bytes()).unwrap();
        assert_eq!(boards[0].latest().unwrap().version, "1.8.18");
        assert_eq!(boards[1].latest().unwrap().version, "1.8.18");
        let none = Board {
            releases: Vec::new(),
            ..boards[0].clone()
        };
        assert_eq!(none.latest(), None);
    }

    #[test]
    fn matches_a_board_only_by_its_exact_name() {
        let boards = parse(LIST.as_bytes()).unwrap();
        assert_eq!(named(&boards, "xteink_x4").unwrap().name, "xteink_x4");
        assert_eq!(named(&boards, "og"), None);
        assert_eq!(named(&boards, "Xteink_X4"), None);
    }
}
