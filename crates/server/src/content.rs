//! Files in one directory, each named by a hash of its content: the screen
//! images a device downloads and the firmware images the server serves.

use std::fmt::Write;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

pub struct Store {
    root: PathBuf,
    /// Hex digits of the SHA-256 a name carries.
    digits: usize,
    extension: &'static str,
}

impl Store {
    #[must_use]
    pub fn new(root: PathBuf, digits: usize, extension: &'static str) -> Self {
        Self {
            root,
            digits,
            extension,
        }
    }

    /// Whether `name` is one this store gives out: its digits in lowercase hex
    /// and its extension.
    fn valid(&self, name: &str) -> bool {
        name.strip_suffix(self.extension)
            .and_then(|stem| stem.strip_suffix('.'))
            .is_some_and(|hash| {
                hash.len() == self.digits
                    && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            })
    }

    /// Keeps `bytes` and returns their name. A file already kept is stored again
    /// only in its time, so what ages files counts from the latest store.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn store(&self, bytes: &[u8]) -> io::Result<String> {
        let mut name = String::with_capacity(self.digits + 1 + self.extension.len());
        for byte in Sha256::digest(bytes).iter().take(self.digits.div_ceil(2)) {
            let _ = write!(name, "{byte:02x}");
        }
        name.truncate(self.digits);
        name.push('.');
        name.push_str(self.extension);
        std::fs::create_dir_all(&self.root)?;
        let path = self.root.join(&name);
        match std::fs::File::options().write(true).open(&path) {
            Ok(file) => file.set_modified(SystemTime::now())?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Written aside and renamed, so a reader never sees half a file.
                let partial = self.root.join(format!("{name}.partial"));
                std::fs::write(&partial, bytes)?;
                std::fs::rename(&partial, &path)?;
            }
            Err(error) => return Err(error),
        }
        Ok(name)
    }

    /// A file by name; `None` when the store has none of that name.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read.
    pub fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !self.valid(name) {
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
        self.valid(name) && self.root.join(name).is_file()
    }

    /// Removes every file for which `keep`, given its name and when it was last
    /// stored, says no.
    ///
    /// # Errors
    ///
    /// When the directory cannot be read or a file removed.
    pub fn retain(&self, keep: impl Fn(&str, Option<SystemTime>) -> bool) -> io::Result<()> {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let stored = entry.metadata()?.modified().ok();
            if !keep(&entry.file_name().to_string_lossy(), stored) {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    /// Removes the store's directory and everything in it.
    ///
    /// # Errors
    ///
    /// When the directory exists but cannot be removed.
    pub fn remove(&self) -> io::Result<()> {
        match std::fs::remove_dir_all(&self.root) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn names_by_the_sha256_and_reads_back_only_its_own_names() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::new(directory.path().join("firmware"), 64, "bin");
        let name = store.store(b"abc").unwrap();
        // SHA-256 of "abc", FIPS 180-2's first example.
        assert_eq!(
            name,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.bin"
        );
        assert!(store.contains(&name));
        assert_eq!(store.read(&name).unwrap().unwrap(), b"abc");
        let short = Store::new(directory.path().join("images"), 16, "png");
        assert_eq!(short.store(b"abc").unwrap(), "ba7816bf8f01cfea.png");
        for foreign in [
            "../firmware",
            "BA7816BF8F01CFEA.png",
            "ba7816bf8f01cfea.bin",
        ] {
            assert_eq!(short.read(foreign).unwrap(), None, "{foreign}");
            assert!(!short.contains(foreign), "{foreign}");
        }
    }

    #[test]
    fn storing_again_renews_the_time_and_retain_removes_what_it_refuses() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::new(directory.path().to_owned(), 16, "png");
        let name = store.store(b"kept").unwrap();
        let dropped = store.store(b"dropped").unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        let file = std::fs::File::options()
            .write(true)
            .open(directory.path().join(&name))
            .unwrap();
        file.set_modified(old).unwrap();
        store.store(b"kept").unwrap();
        let renewed = file.metadata().unwrap().modified().unwrap();
        assert!(renewed > old + Duration::from_secs(60), "{renewed:?}");
        store.retain(|candidate, _| candidate == name).unwrap();
        assert!(store.contains(&name));
        assert!(!store.contains(&dropped));
    }
}
