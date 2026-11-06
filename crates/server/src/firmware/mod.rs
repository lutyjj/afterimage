//! Firmware updates: TRMNL's releases, the app images the server cuts from them
//! and serves, and each device's install request.
//!
//! An install fetches the release's flash image from TRMNL, cuts and checks its
//! app image ([`app_image`]), keeps it in the cache directory and records the
//! request ([`updates`]); the device's next check-in names the image, which the
//! device downloads from this server, so devices without internet access update
//! too.

pub mod app_image;
pub mod cache;
pub mod releases;
pub mod updates;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::clock::Clock;
use crate::devices::Device;
use crate::outbound::Outbound;

pub use releases::{Board, Release};
pub use updates::{State, Update};

use cache::Cache;
use releases::Releases;
use updates::{Delivery, Updates};

/// How long a flash image download may take.
const IMAGE_TIMEOUT: Duration = Duration::from_secs(120);
/// Largest flash image read. TRMNL X's is 16 MB, its whole flash.
const FLASH_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Choose this device's board before installing firmware.")]
    NoBoard,
    #[error("TRMNL no longer lists the board {0}.")]
    UnknownBoard(String),
    #[error("TRMNL lists no release {version} for {board}.")]
    NoSuchRelease { board: String, version: String },
    #[error("TRMNL lists no release for {0}.")]
    NoRelease(String),
    #[error("TRMNL's firmware list could not be read: it {0}.")]
    List(String),
    #[error("TRMNL's image of {version} for {board}: {reason}.")]
    Image {
        board: String,
        version: String,
        reason: String,
    },
    #[error("firmware storage: {0}")]
    Storage(#[from] sqlx::Error),
    #[error("the firmware cache: {0}")]
    Cache(#[from] std::io::Error),
}

/// Why an image could not be cut.
#[derive(Debug, thiserror::Error)]
enum CutError {
    #[error("it {0}")]
    Fetch(String),
    #[error("{0}")]
    Image(#[from] app_image::Error),
    #[error(transparent)]
    Cache(#[from] std::io::Error),
}

pub struct Firmware {
    releases: Releases,
    cache: Cache,
    updates: Updates,
    http: Outbound,
    /// Held while an image is stored and its request recorded, so that the cache
    /// is pruned only between installs.
    installing: Mutex<()>,
}

impl Firmware {
    /// `trmnl` is TRMNL's site, such as `https://trmnl.com`; images live under
    /// `cache_dir`.
    #[must_use]
    pub fn new(trmnl: &str, cache_dir: PathBuf, pool: SqlitePool, clock: Arc<dyn Clock>) -> Self {
        Self {
            releases: Releases::new(trmnl, Arc::clone(&clock)),
            cache: Cache::new(cache_dir),
            updates: Updates::new(pool, clock),
            http: Outbound::new(IMAGE_TIMEOUT),
            installing: Mutex::new(()),
        }
    }

    /// TRMNL's boards.
    ///
    /// # Errors
    ///
    /// Why TRMNL's list could not be read, when it never was.
    pub async fn boards(&self) -> Result<Arc<[Board]>, String> {
        self.releases.boards().await
    }

    /// The board the device installs firmware for: the one a user chose, or else
    /// the one TRMNL names like the device's firmware model.
    #[must_use]
    pub fn board<'a>(boards: &'a [Board], device: &Device) -> Option<&'a Board> {
        match &device.firmware_board {
            Some(chosen) => boards.iter().find(|board| &board.name == chosen),
            None => releases::named(boards, &device.firmware_model),
        }
    }

    /// Asks the device to install `version` of its board's firmware, or the
    /// newest, once its image is cut, checked and cached.
    ///
    /// # Errors
    ///
    /// When the device has no board or TRMNL no such release, when TRMNL's list or
    /// image cannot be read or the image fails its checks, and when storage
    /// fails.
    pub async fn install(
        &self,
        device: &Device,
        version: Option<&str>,
    ) -> Result<(), InstallError> {
        let boards = self.boards().await.map_err(InstallError::List)?;
        let board = Self::board(&boards, device).ok_or_else(|| match &device.firmware_board {
            Some(chosen) => InstallError::UnknownBoard(chosen.clone()),
            None => InstallError::NoBoard,
        })?;
        let release = match version {
            Some(version) => board
                .release(version)
                .ok_or_else(|| InstallError::NoSuchRelease {
                    board: board.label.clone(),
                    version: version.to_owned(),
                })?,
            None => board
                .latest()
                .ok_or_else(|| InstallError::NoRelease(board.label.clone()))?,
        };
        let _installing = self.installing.lock().await;
        let image = self.cut(release).await.map_err(|error| match error {
            CutError::Cache(error) => InstallError::Cache(error),
            other => InstallError::Image {
                board: board.label.clone(),
                version: release.version.clone(),
                reason: other.to_string(),
            },
        })?;
        self.updates
            .request(device.id, &board.name, &release.version, &image)
            .await?;
        tracing::info!(
            device = device.id,
            board = board.name,
            version = release.version,
            "firmware install requested"
        );
        self.prune().await;
        Ok(())
    }

    /// Withdraws the device's request.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn cancel(&self, device_id: i64) -> Result<(), sqlx::Error> {
        self.updates.cancel(device_id).await?;
        self.forget_unused().await;
        Ok(())
    }

    /// Removes cached images that no request names any more, as after a device's
    /// board changed and withdrew its request.
    pub async fn forget_unused(&self) {
        let _installing = self.installing.lock().await;
        self.prune().await;
    }

    /// The image this check-in's answer tells the device to install, if any. A
    /// request whose image left the cache waits for it to be cut again, which
    /// starts here.
    ///
    /// # Errors
    ///
    /// When storage fails.
    pub async fn check_in(
        self: &Arc<Self>,
        device: &Device,
    ) -> Result<Option<String>, sqlx::Error> {
        let delivery = self
            .updates
            .check_in(
                device.id,
                device.readings.firmware_version.as_deref(),
                |image| self.cache.contains(image),
            )
            .await?;
        Ok(match delivery {
            Delivery::Nothing => None,
            Delivery::Send(image) => {
                tracing::info!(
                    device = device.id,
                    image,
                    "sending a firmware install request"
                );
                Some(image)
            }
            Delivery::Missing { board, version } => {
                let firmware = Arc::clone(self);
                let device_id = device.id;
                tokio::spawn(async move { firmware.cut_again(device_id, &board, &version).await });
                None
            }
        })
    }

    /// A cached image by name; `None` when there is none of that name.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read.
    pub fn image(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        self.cache.read(name)
    }

    /// Fetches the release's flash image and caches its checked app image.
    async fn cut(&self, release: &Release) -> Result<String, CutError> {
        let request = self.http.client().get(&release.url);
        let flash = self
            .http
            .read(request, FLASH_BYTES)
            .await
            .map_err(CutError::Fetch)?;
        let app = app_image::cut(&flash)?;
        Ok(self.cache.store(app)?)
    }

    /// Cuts a waiting request's image again after the cache lost it.
    async fn cut_again(&self, device_id: i64, board: &str, version: &str) {
        let release = match self.boards().await {
            Ok(boards) => boards
                .iter()
                .find(|listed| listed.name == board)
                .and_then(|board| board.release(version))
                .cloned(),
            Err(failure) => {
                tracing::warn!(%failure, device = device_id, "a lost firmware image waits for TRMNL's list");
                return;
            }
        };
        let Some(release) = release else {
            tracing::warn!(
                device = device_id,
                board,
                version,
                "TRMNL no longer lists a requested release"
            );
            return;
        };
        let _installing = self.installing.lock().await;
        match self.cut(&release).await {
            Ok(image) => {
                if let Err(error) = self.updates.replace_image(device_id, version, &image).await {
                    tracing::error!(%error, device = device_id, "a firmware image was cut again but not recorded");
                }
            }
            Err(error) => {
                tracing::warn!(%error, device = device_id, board, version, "a lost firmware image could not be cut again");
            }
        }
    }

    /// Removes cached images no device may still download. A failure leaves
    /// files behind, which the next prune removes.
    async fn prune(&self) {
        let result = match self.updates.images_in_use().await {
            Ok(images) => self
                .cache
                .retain(&images)
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if let Err(error) = result {
            tracing::warn!(%error, "unused firmware images stay in the cache");
        }
    }
}
