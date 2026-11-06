//! The app images the server serves, as files in its cache directory, each named
//! by its SHA-256. Every one can be cut from TRMNL's flash image again.

use std::fmt::Write;
use std::io;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

pub struct Cache {
    root: PathBuf,
}

/// Whether `name` is one this cache gives out: 64 lowercase hex digits and
/// `.bin`.
fn valid(name: &str) -> bool {
    name.strip_suffix(".bin").is_some_and(|hash| {
        hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

impl Cache {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Keeps an image and returns its name.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn store(&self, image: &[u8]) -> io::Result<String> {
        let mut name = String::with_capacity(68);
        for byte in Sha256::digest(image) {
            let _ = write!(name, "{byte:02x}");
        }
        name.push_str(".bin");
        std::fs::create_dir_all(&self.root)?;
        let path = self.root.join(&name);
        if !path.exists() {
            // Written aside and renamed, so a reader never sees half a file.
            let partial = self.root.join(format!("{name}.partial"));
            std::fs::write(&partial, image)?;
            std::fs::rename(&partial, &path)?;
        }
        Ok(name)
    }

    /// An image by name; `None` when the cache has none of that name.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read.
    pub fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !valid(name) {
            return Ok(None);
        }
        match std::fs::read(self.root.join(name)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        valid(name) && self.root.join(name).is_file()
    }

    /// Removes every file but the images named in `keep`.
    ///
    /// # Errors
    ///
    /// When the directory cannot be read or a file removed.
    pub fn retain(&self, keep: &[String]) -> io::Result<()> {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            if !keep.iter().any(|kept| name.as_os_str() == kept.as_str()) {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_an_image_by_its_sha256_and_reads_it_back() {
        let directory = tempfile::tempdir().unwrap();
        let cache = Cache::new(directory.path().join("firmware"));
        let name = cache.store(b"abc").unwrap();
        // SHA-256 of "abc", FIPS 180-2's first example.
        assert_eq!(
            name,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.bin"
        );
        assert!(cache.contains(&name));
        assert_eq!(cache.read(&name).unwrap().unwrap(), b"abc");
        assert_eq!(cache.read("../firmware").unwrap(), None);
        assert!(!cache.contains("../firmware"));
    }

    #[test]
    fn keeps_only_the_images_it_is_told_to() {
        let directory = tempfile::tempdir().unwrap();
        let cache = Cache::new(directory.path().to_owned());
        let kept = cache.store(b"kept").unwrap();
        let dropped = cache.store(b"dropped").unwrap();
        cache.retain(std::slice::from_ref(&kept)).unwrap();
        assert!(cache.contains(&kept));
        assert!(!cache.contains(&dropped));
    }
}
