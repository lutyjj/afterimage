//! The images drawn for each device, as files under `<data dir>/images/<device>/`.
//! A file is named by its content's hash, so an unchanged screen keeps its name and
//! the firmware, which caches by `filename`, skips the download.

use std::fmt::Write;
use std::io;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

pub struct Images {
    root: PathBuf,
}

/// Whether `name` is one this store gives out: 16 lowercase hex digits and `.png`.
fn valid(name: &str) -> bool {
    name.strip_suffix(".png").is_some_and(|hash| {
        hash.len() == 16 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

impl Images {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Stores a device's image and returns its name. Every other image of the
    /// device goes, except `keep`, the one it may still be downloading.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn save(&self, device: i64, bytes: &[u8], keep: Option<&str>) -> io::Result<String> {
        let hash = Sha256::digest(bytes);
        let mut name = String::with_capacity(20);
        for byte in &hash[..8] {
            let _ = write!(name, "{byte:02x}");
        }
        name.push_str(".png");
        let directory = self.root.join(device.to_string());
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(&name);
        if !path.exists() {
            // Written aside and renamed, so a reader never sees half a file.
            let partial = directory.join(format!("{name}.partial"));
            std::fs::write(&partial, bytes)?;
            std::fs::rename(&partial, &path)?;
        }
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let file = entry.file_name();
            let file = file.to_string_lossy();
            if file != name && Some(file.as_ref()) != keep {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(name)
    }

    /// A device's image by name; `None` when it has no such image.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read.
    pub fn read(&self, device: i64, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !valid(name) {
            return Ok(None);
        }
        match std::fs::read(self.root.join(device.to_string()).join(name)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Removes a device's images.
    ///
    /// # Errors
    ///
    /// When the directory exists but cannot be removed.
    pub fn forget(&self, device: i64) -> io::Result<()> {
        match std::fs::remove_dir_all(self.root.join(device.to_string())) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_by_content_and_keeps_the_previous_image() {
        let directory = tempfile::TempDir::new().unwrap();
        let images = Images::new(directory.path().to_owned());
        let first = images.save(7, b"first", None).unwrap();
        assert!(valid(&first), "{first}");
        assert_eq!(images.save(7, b"first", None).unwrap(), first);
        let second = images.save(7, b"second", Some(&first)).unwrap();
        assert_eq!(
            images.read(7, &first).unwrap().as_deref(),
            Some(&b"first"[..])
        );
        let third = images.save(7, b"third", Some(&second)).unwrap();
        assert_eq!(images.read(7, &first).unwrap(), None, "older images go");
        assert!(images.read(7, &second).unwrap().is_some());
        assert!(images.read(7, &third).unwrap().is_some());
        assert_eq!(images.read(8, &third).unwrap(), None, "per device");
    }

    #[test]
    fn reads_only_names_it_gives_out() {
        let directory = tempfile::TempDir::new().unwrap();
        let images = Images::new(directory.path().to_owned());
        assert_eq!(images.read(1, "../../afterimage.db").unwrap(), None);
        assert_eq!(images.read(1, "0123456789ABCDEF.png").unwrap(), None);
    }
}
