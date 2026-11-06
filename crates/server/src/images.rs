//! The images drawn for each device, as files under `<data dir>/images/<device>/`.
//! A file is named by its content's hash, so an unchanged screen keeps its name and
//! the firmware, which caches by `filename`, skips the download.

use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use crate::content::Store;

/// How long an image outlives a newer one of its device.
pub const RETAIN: Duration = Duration::from_mins(5);

pub struct Images {
    root: PathBuf,
    /// Held while any device's images change, so that one check-in's store of an
    /// image cannot interleave with another's removal of old ones: the second
    /// would read the first image's time before the first renews it. One lock
    /// serves every device, since a save takes milliseconds and a server drives
    /// a household's few displays.
    changing: Mutex<()>,
}

impl Images {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            changing: Mutex::new(()),
        }
    }

    fn of(&self, device: i64) -> Store {
        Store::new(self.root.join(device.to_string()), 16, "png")
    }

    /// Stores a device's image and returns its name. Other images of the device
    /// go once they were last stored more than [`RETAIN`] ago, so an image a
    /// check-in just named stays downloadable even when another check-in of the
    /// device, such as the firmware's retry of a slow answer, draws a different
    /// one meanwhile.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn save(&self, device: i64, bytes: &[u8]) -> io::Result<String> {
        let _changing = self.changing.lock().unwrap_or_else(PoisonError::into_inner);
        let store = self.of(device);
        let name = store.store(bytes)?;
        let now = SystemTime::now();
        store.retain(|candidate, stored| {
            let age = stored.and_then(|stored| now.duration_since(stored).ok());
            candidate == name || age.is_none_or(|age| age <= RETAIN)
        })?;
        Ok(name)
    }

    /// A device's image by name; `None` when it has no such image.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read.
    pub fn read(&self, device: i64, name: &str) -> io::Result<Option<Vec<u8>>> {
        self.of(device).read(name)
    }

    /// Whether the device still has an image of this name.
    #[must_use]
    pub fn has(&self, device: i64, name: &str) -> bool {
        self.of(device).contains(name)
    }

    /// Removes a device's images.
    ///
    /// # Errors
    ///
    /// When the directory exists but cannot be removed.
    pub fn forget(&self, device: i64) -> io::Result<()> {
        let _changing = self.changing.lock().unwrap_or_else(PoisonError::into_inner);
        self.of(device).remove()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_by_content_and_drops_images_older_than_a_newer_one() {
        let directory = tempfile::TempDir::new().unwrap();
        let images = Images::new(directory.path().to_owned());
        let first = images.save(7, b"first").unwrap();
        assert_eq!(images.save(7, b"first").unwrap(), first);
        // A second image drawn meanwhile leaves the first downloadable.
        let second = images.save(7, b"second").unwrap();
        assert_eq!(
            images.read(7, &first).unwrap().as_deref(),
            Some(&b"first"[..])
        );
        // Once the first is older than RETAIN, the next save removes it.
        let old = SystemTime::now() - RETAIN - Duration::from_secs(1);
        std::fs::File::options()
            .write(true)
            .open(directory.path().join("7").join(&first))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let third = images.save(7, b"third").unwrap();
        assert_eq!(images.read(7, &first).unwrap(), None, "older images go");
        assert!(images.read(7, &second).unwrap().is_some());
        assert!(images.read(7, &third).unwrap().is_some());
        assert_eq!(images.read(8, &third).unwrap(), None, "per device");
    }

    #[test]
    fn an_image_named_again_counts_its_age_from_then() {
        let directory = tempfile::TempDir::new().unwrap();
        let images = Images::new(directory.path().to_owned());
        let unchanged = images.save(7, b"unchanged").unwrap();
        let old = SystemTime::now() - RETAIN - Duration::from_secs(1);
        std::fs::File::options()
            .write(true)
            .open(directory.path().join("7").join(&unchanged))
            .unwrap()
            .set_modified(old)
            .unwrap();
        // A check-in names the unchanged image again, and another check-in of the
        // device draws a new one right after: the first must stay downloadable.
        assert_eq!(images.save(7, b"unchanged").unwrap(), unchanged);
        images.save(7, b"new").unwrap();
        assert!(images.has(7, &unchanged));
    }

    #[test]
    fn reads_only_names_it_gives_out() {
        let directory = tempfile::TempDir::new().unwrap();
        let images = Images::new(directory.path().to_owned());
        assert_eq!(images.read(1, "../../afterimage.db").unwrap(), None);
        assert_eq!(images.read(1, "0123456789ABCDEF.png").unwrap(), None);
    }
}
