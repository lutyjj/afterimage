//! The app image inside one of TRMNL's flash images, which an over-the-air update
//! writes.
//!
//! A flash image is what a USB flasher writes from address 0: the bootloader, the
//! partition table at 0x8000, the app in an app partition, and on some boards more
//! partitions after it. The server reads the partition table, takes the app
//! partition that holds an image, and reads the image's own header for its
//! length. It checks the image's checksum and SHA-256 as the bootloader does
//! before starting one, so a corrupt download never reaches a device. For TRMNL's
//! own boards the result is byte for byte the over-the-air image TRMNL publishes
//! at `GET /api/firmware`.
//!
//! `esp-idf-part` reads the partition table and checks its MD5. The image format
//! is ESP-IDF's (`esp_image_format.h`, "App Image Format" in its guide); no
//! maintained crate reads it (espflash only writes it, with private header
//! types), so the walk over its segments is here.

use esp_idf_part::{PartitionTable, Type};
use sha2::{Digest, Sha256};

/// Where ESP-IDF places the partition table (`CONFIG_PARTITION_TABLE_OFFSET`).
const PARTITION_TABLE: usize = 0x8000;
/// The space ESP-IDF gives the table's entries (`ESP_PARTITION_TABLE_MAX_LEN`).
const PARTITION_TABLE_LENGTH: usize = 0xC00;
/// `ESP_IMAGE_HEADER_MAGIC`.
const MAGIC: u8 = 0xE9;
/// `esp_image_header_t`, the extended header included.
const HEADER_LENGTH: usize = 24;
/// Where `hash_appended` sits in the header.
const HASH_APPENDED: usize = 23;
/// `esp_image_segment_header_t`: a load address and a length.
const SEGMENT_HEADER_LENGTH: usize = 8;
/// `ESP_IMAGE_MAX_SEGMENTS`.
const MAX_SEGMENTS: u8 = 16;
/// `ESP_ROM_CHECKSUM_INITIAL`, the start of the XOR over every byte of segment
/// data.
const CHECKSUM_SEED: u8 = 0xEF;
const DIGEST_LENGTH: usize = 32;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("its partition table does not read: {0}")]
    PartitionTable(String),
    #[error("no app partition holds an image")]
    NoApp,
    #[error("its app image is cut short")]
    Truncated,
    #[error("its app image claims {0} segments, more than ESP-IDF allows")]
    TooManySegments(u8),
    #[error("its app image fails its checksum")]
    Checksum,
    #[error("its app image carries no SHA-256")]
    NoDigest,
    #[error("its app image fails its SHA-256")]
    Digest,
}

/// The checked app image of a flash image.
///
/// # Errors
///
/// When the flash image has no readable partition table or app image, or the app
/// image fails its checksum or SHA-256.
pub fn cut(flash: &[u8]) -> Result<&[u8], Error> {
    let table = flash
        .get(PARTITION_TABLE..PARTITION_TABLE + PARTITION_TABLE_LENGTH)
        .ok_or(Error::Truncated)?;
    let table = PartitionTable::try_from_bytes(table)
        .map_err(|error| Error::PartitionTable(error.to_string()))?;
    let app = table
        .partitions()
        .iter()
        .filter(|partition| partition.ty() == Type::App)
        .find_map(|partition| {
            let start = usize::try_from(partition.offset()).ok()?;
            let end = start.checked_add(usize::try_from(partition.size()).ok()?)?;
            let contents = flash.get(start..end.min(flash.len()))?;
            (contents.first() == Some(&MAGIC)).then_some(contents)
        })
        .ok_or(Error::NoApp)?;
    let length = checked_length(app)?;
    Ok(&app[..length])
}

/// The length of the app image at the start of `bytes`, once its checksum and
/// SHA-256 hold. Segments follow the header, each a segment header and its data;
/// the checksum is the last byte of the 16-byte block they end in, and the
/// SHA-256 of everything before it follows.
fn checked_length(bytes: &[u8]) -> Result<usize, Error> {
    let header = bytes.get(..HEADER_LENGTH).ok_or(Error::Truncated)?;
    let segments = header[1];
    if segments > MAX_SEGMENTS {
        return Err(Error::TooManySegments(segments));
    }
    let mut position = HEADER_LENGTH;
    let mut checksum = CHECKSUM_SEED;
    for _ in 0..segments {
        let segment = bytes
            .get(position..position + SEGMENT_HEADER_LENGTH)
            .ok_or(Error::Truncated)?;
        let length = u32::from_le_bytes([segment[4], segment[5], segment[6], segment[7]]);
        let length = usize::try_from(length).map_err(|_| Error::Truncated)?;
        position += SEGMENT_HEADER_LENGTH;
        let data = bytes
            .get(position..position.checked_add(length).ok_or(Error::Truncated)?)
            .ok_or(Error::Truncated)?;
        checksum = data.iter().fold(checksum, |checksum, byte| checksum ^ byte);
        position += length;
    }
    let checksum_at = (position + 1).next_multiple_of(16) - 1;
    if *bytes.get(checksum_at).ok_or(Error::Truncated)? != checksum {
        return Err(Error::Checksum);
    }
    let digest_at = checksum_at + 1;
    if header[HASH_APPENDED] != 1 {
        return Err(Error::NoDigest);
    }
    let digest = bytes
        .get(digest_at..digest_at + DIGEST_LENGTH)
        .ok_or(Error::Truncated)?;
    if Sha256::digest(&bytes[..digest_at]).as_slice() != digest {
        return Err(Error::Digest);
    }
    Ok(digest_at + DIGEST_LENGTH)
}

#[cfg(test)]
pub(crate) mod tests {
    use esp_idf_part::{AppType, DataType, Flags, Partition, SubType};

    use super::*;

    /// An app image of two segments, laid out by hand from ESP-IDF's format:
    /// the header (magic, two segments, `hash_appended`), each segment's load
    /// address and length before its data, zero padding to the checksum at
    /// offset 47, and the SHA-256 of the 48 bytes before it.
    pub(crate) fn app_image() -> Vec<u8> {
        let mut image = vec![0; HEADER_LENGTH];
        image[0] = MAGIC;
        image[1] = 2;
        image[HASH_APPENDED] = 1;
        image.extend_from_slice(&0x3c00_0020_u32.to_le_bytes());
        image.extend_from_slice(&3_u32.to_le_bytes());
        image.extend_from_slice(&[0x01, 0x02, 0x04]); // offset 32 to 34
        image.extend_from_slice(&0x4038_0000_u32.to_le_bytes());
        image.extend_from_slice(&1_u32.to_le_bytes());
        image.push(0x08); // offset 43
        image.extend_from_slice(&[0; 3]);
        // 0xEF ^ 0x01 ^ 0x02 ^ 0x04 ^ 0x08
        image.push(0xE0);
        let digest = Sha256::digest(&image);
        image.extend_from_slice(&digest);
        image
    }

    /// A flash image as TRMNL publishes one: a stand-in bootloader, the partition
    /// table at 0x8000, `app` in `ota_0` at 0x10000 and an empty `ota_1`, then
    /// `after` (the X's flash image carries its other partitions there).
    pub(crate) fn flash_image(app: &[u8], after: &[u8]) -> Vec<u8> {
        let partitions = PartitionTable::new(vec![
            Partition::new(
                "nvs",
                Type::Data,
                SubType::Data(DataType::Nvs),
                0x9000,
                0x5000,
                Flags::empty(),
            ),
            Partition::new(
                "otadata",
                Type::Data,
                SubType::Data(DataType::Ota),
                0xe000,
                0x2000,
                Flags::empty(),
            ),
            Partition::new(
                "app0",
                Type::App,
                SubType::App(AppType::Ota_0),
                0x10000,
                0x10000,
                Flags::empty(),
            ),
            Partition::new(
                "app1",
                Type::App,
                SubType::App(AppType::Ota_1),
                0x20000,
                0x10000,
                Flags::empty(),
            ),
        ]);
        let mut flash = vec![0xFF; PARTITION_TABLE];
        flash[0] = MAGIC;
        flash.extend_from_slice(&partitions.to_bin().unwrap());
        flash.resize(0x10000, 0xFF);
        flash.extend_from_slice(app);
        flash.extend_from_slice(after);
        flash
    }

    #[test]
    fn cuts_the_app_out_of_a_flash_image() {
        let app = app_image();
        assert_eq!(cut(&flash_image(&app, &[])).unwrap(), app.as_slice());
        let followed = flash_image(&app, &[0xAA; 100]);
        assert_eq!(
            cut(&followed).unwrap(),
            app.as_slice(),
            "ends at its own end"
        );
    }

    #[test]
    fn refuses_an_app_whose_data_or_digest_changed() {
        let mut app = app_image();
        app[33] ^= 0x10;
        assert_eq!(cut(&flash_image(&app, &[])), Err(Error::Checksum));
        let mut app = app_image();
        app[50] ^= 0x10;
        assert_eq!(cut(&flash_image(&app, &[])), Err(Error::Digest));
        let mut app = app_image();
        app[HASH_APPENDED] = 0;
        assert_eq!(cut(&flash_image(&app, &[])), Err(Error::NoDigest));
    }

    #[test]
    fn refuses_what_is_not_a_whole_flash_image() {
        let app = app_image();
        let flash = flash_image(&app, &[]);
        assert_eq!(
            cut(&flash[..flash.len() - 1]),
            Err(Error::Truncated),
            "short of its digest"
        );
        assert_eq!(cut(&flash[..0x8100]), Err(Error::Truncated));
        assert_eq!(cut(&app), Err(Error::Truncated), "an app image alone");
        let empty = flash_image(&[], &[]);
        assert_eq!(cut(&empty), Err(Error::NoApp));
        let mut table = flash.clone();
        table[PARTITION_TABLE + 40] ^= 1;
        assert!(matches!(cut(&table), Err(Error::PartitionTable(_))));
        let mut segments = flash;
        segments[0x10001] = 17;
        assert_eq!(cut(&segments), Err(Error::TooManySegments(17)));
    }
}
