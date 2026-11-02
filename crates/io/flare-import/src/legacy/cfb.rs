//! A small, bounded reader for the Compound File Binary container (OLE2) that holds a legacy
//! `.fla`.
//!
//! Only what a reader needs: the sector tables, the directory's streams (the storage tree is
//! ignored, a legacy `.fla` has none) and the bytes of a stream. Every number in the file is
//! untrusted: sector ids are range-checked, a chain longer than the file has sectors is a cycle,
//! and nothing is allocated for a size the file does not back with real bytes.

use thiserror::Error;

/// The largest file the reader looks at.
pub(crate) const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
/// The largest single stream it hands out.
pub(crate) const MAX_STREAM_BYTES: u64 = 256 * 1024 * 1024;
/// The most directory entries it reads (each is 128 bytes).
const MAX_DIR_ENTRIES: usize = 65_536;

const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const HEADER_DIFAT_ENTRIES: usize = 109;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;
/// Sector ids from here up are markers (free, end of chain, FAT, DIFAT), never data.
const MAX_REGULAR_SECTOR: u32 = 0xFFFF_FFFA;
const MINI_SECTOR_SIZE: usize = 64;
const DIR_ENTRY_SIZE: usize = 128;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CfbError {
    #[error("it does not start with the compound file signature")]
    NotCfb,
    #[error("the file is {0} bytes, over the {MAX_FILE_BYTES} byte limit")]
    FileTooLarge(u64),
    #[error("the header is not valid: {0}")]
    BadHeader(&'static str),
    #[error("the file ends inside {0}")]
    Truncated(&'static str),
    #[error("{0} points outside the file")]
    OutOfRange(&'static str),
    #[error("{0} loops back on itself")]
    Cycle(&'static str),
    #[error("the directory has more than {MAX_DIR_ENTRIES} entries")]
    TooManyEntries,
    #[error(
        "stream {name:?} claims {size} bytes, over the {MAX_STREAM_BYTES} byte limit or past the end of the file"
    )]
    StreamTooLarge { name: String, size: u64 },
    #[error("there is no stream named {0:?}")]
    NoStream(String),
}

/// A stream in the directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    pub name: String,
    /// The size the directory claims.
    pub size: u64,
    start: u32,
}

/// An open container over bytes already in memory.
pub struct Container<'a> {
    data: &'a [u8],
    sector_size: usize,
    sector_count: usize,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    mini_stream: Vec<u8>,
    mini_cutoff: u64,
    streams: Vec<StreamInfo>,
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn words(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
}

impl<'a> Container<'a> {
    pub fn open(data: &'a [u8]) -> Result<Self, CfbError> {
        if !data.starts_with(&SIGNATURE) {
            return Err(CfbError::NotCfb);
        }
        if data.len() as u64 > MAX_FILE_BYTES {
            return Err(CfbError::FileTooLarge(data.len() as u64));
        }
        let header = |at, what| u16_at(data, at).ok_or(CfbError::Truncated(what));
        let header32 = |at, what| u32_at(data, at).ok_or(CfbError::Truncated(what));
        let major = header(26, "the header")?;
        if header(28, "the header")? != 0xFFFE {
            return Err(CfbError::BadHeader("it is not little-endian"));
        }
        let sector_shift = header(30, "the header")?;
        let sector_size = match (major, sector_shift) {
            (3, 9) => 512,
            (4, 12) => 4096,
            _ => return Err(CfbError::BadHeader("unsupported version or sector size")),
        };
        if header(32, "the header")? != 6 {
            return Err(CfbError::BadHeader("unsupported mini sector size"));
        }
        let fat_sectors = header32(44, "the header")?;
        let dir_start = header32(48, "the header")?;
        let mini_cutoff = u64::from(header32(56, "the header")?);
        if mini_cutoff != 4096 {
            return Err(CfbError::BadHeader("unsupported mini stream cutoff"));
        }
        let mini_fat_start = header32(60, "the header")?;
        let difat_start = header32(68, "the header")?;
        let difat_sectors = header32(72, "the header")?;

        // The header occupies the first sector; a writer may leave the last one short.
        let sector_count = data.len().saturating_sub(sector_size).div_ceil(sector_size);
        let mut container = Self {
            data,
            sector_size,
            sector_count,
            fat: Vec::new(),
            mini_fat: Vec::new(),
            mini_stream: Vec::new(),
            mini_cutoff,
            streams: Vec::new(),
        };
        if fat_sectors as usize > sector_count || difat_sectors as usize > sector_count {
            return Err(CfbError::OutOfRange("the sector table count"));
        }

        // Which sectors hold the FAT: 109 ids in the header, then a chain of DIFAT sectors.
        let mut fat_ids: Vec<u32> = words(
            data.get(76..76 + 4 * HEADER_DIFAT_ENTRIES)
                .ok_or(CfbError::Truncated("the header"))?,
        )
        .filter(|&id| id <= MAX_REGULAR_SECTOR)
        .collect();
        let mut next = difat_start;
        for _ in 0..difat_sectors {
            let sector = container.sector(next, "a sector table index")?;
            let mut ids: Vec<u32> = words(sector).collect();
            next = ids.pop().unwrap_or(END_OF_CHAIN);
            fat_ids.extend(ids.into_iter().filter(|&id| id <= MAX_REGULAR_SECTOR));
        }
        fat_ids.truncate(fat_sectors as usize);
        if fat_ids.len() < fat_sectors as usize {
            return Err(CfbError::Truncated("the sector table index"));
        }
        let mut fat = Vec::with_capacity(fat_ids.len() * (sector_size / 4));
        for id in fat_ids {
            fat.extend(words(container.sector(id, "the sector table")?));
        }
        container.fat = fat;

        let dir =
            container.read_chain(dir_start, MAX_DIR_ENTRIES * DIR_ENTRY_SIZE, "the directory")?;
        let entries = dir.len() / DIR_ENTRY_SIZE;
        if entries > MAX_DIR_ENTRIES {
            return Err(CfbError::TooManyEntries);
        }
        let mut root = None;
        for index in 0..entries {
            let entry = &dir[index * DIR_ENTRY_SIZE..(index + 1) * DIR_ENTRY_SIZE];
            let kind = entry.get(66).copied().unwrap_or(0);
            let start = u32_at(entry, 116).unwrap_or(END_OF_CHAIN);
            let mut size = u64_at(entry, 120).unwrap_or(0);
            if sector_size == 512 {
                // A version 3 file keeps only the low half; the high half is garbage in the wild.
                size &= 0xFFFF_FFFF;
            }
            match kind {
                5 if root.is_none() => root = Some((start, size)),
                2 => {
                    let name_bytes = usize::from(u16_at(entry, 64).unwrap_or(0)).min(64);
                    let units: Vec<u16> = entry[..name_bytes.saturating_sub(2)]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| u16::from_le_bytes(*c))
                        .collect();
                    container.streams.push(StreamInfo {
                        name: String::from_utf16_lossy(&units),
                        size,
                        start,
                    });
                }
                _ => {}
            }
        }

        if let Some((start, size)) = root
            && start <= MAX_REGULAR_SECTOR
            && size > 0
        {
            // The small streams live in a stream of their own, chained from the root entry.
            let limit = usize::try_from(size.min(MAX_STREAM_BYTES)).unwrap_or(0);
            container.mini_stream = container.read_chain(start, limit, "the small stream store")?;
            let mini_fat = container.read_chain(
                mini_fat_start,
                container.fat.len().saturating_mul(4),
                "the small stream table",
            )?;
            container.mini_fat = words(&mini_fat).collect();
        }
        Ok(container)
    }

    /// The streams, in directory order.
    pub fn streams(&self) -> &[StreamInfo] {
        &self.streams
    }

    fn sector(&self, id: u32, what: &'static str) -> Result<&'a [u8], CfbError> {
        if id > MAX_REGULAR_SECTOR || id as usize >= self.sector_count {
            return Err(CfbError::OutOfRange(what));
        }
        let start = (id as usize + 1) * self.sector_size;
        let end = (start + self.sector_size).min(self.data.len());
        self.data.get(start..end).ok_or(CfbError::Truncated(what))
    }

    fn next_in_fat(&self, id: u32, what: &'static str) -> Result<u32, CfbError> {
        self.fat
            .get(id as usize)
            .copied()
            .ok_or(CfbError::OutOfRange(what))
    }

    /// The bytes of a sector chain, at most `limit` of them. A sector visited twice is a cycle.
    fn read_chain(
        &self,
        start: u32,
        limit: usize,
        what: &'static str,
    ) -> Result<Vec<u8>, CfbError> {
        let mut bytes = Vec::new();
        let mut id = start;
        let mut seen = vec![false; self.sector_count];
        while id != END_OF_CHAIN && id != FREE && bytes.len() < limit {
            let visited = seen
                .get_mut(id as usize)
                .ok_or(CfbError::OutOfRange(what))?;
            if std::mem::replace(visited, true) {
                return Err(CfbError::Cycle(what));
            }
            bytes.extend_from_slice(self.sector(id, what)?);
            id = self.next_in_fat(id, what)?;
        }
        bytes.truncate(limit);
        Ok(bytes)
    }

    /// The bytes of the stream called `name`.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, CfbError> {
        let info = self
            .streams
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| CfbError::NoStream(name.to_owned()))?;
        let too_large = || CfbError::StreamTooLarge {
            name: info.name.clone(),
            size: info.size,
        };
        if info.size > MAX_STREAM_BYTES || info.size > self.data.len() as u64 {
            return Err(too_large());
        }
        let size = usize::try_from(info.size).map_err(|_| too_large())?;
        let bytes = if info.size < self.mini_cutoff {
            self.read_mini(info.start, size)?
        } else {
            self.read_chain(info.start, size, "a stream")?
        };
        if bytes.len() < size {
            return Err(CfbError::Truncated("a stream"));
        }
        Ok(bytes)
    }

    fn read_mini(&self, start: u32, size: usize) -> Result<Vec<u8>, CfbError> {
        let mut bytes = Vec::with_capacity(size);
        let mut id = start;
        let mut seen = vec![false; self.mini_fat.len()];
        while id != END_OF_CHAIN && id != FREE && bytes.len() < size {
            let visited = seen
                .get_mut(id as usize)
                .ok_or(CfbError::OutOfRange("a small stream"))?;
            if std::mem::replace(visited, true) {
                return Err(CfbError::Cycle("a small stream"));
            }
            let from = (id as usize)
                .checked_mul(MINI_SECTOR_SIZE)
                .ok_or(CfbError::OutOfRange("a small stream"))?;
            let sector = self
                .mini_stream
                .get(from..from + MINI_SECTOR_SIZE)
                .ok_or(CfbError::OutOfRange("a small stream"))?;
            bytes.extend_from_slice(sector);
            id = self
                .mini_fat
                .get(id as usize)
                .copied()
                .ok_or(CfbError::OutOfRange("a small stream"))?;
        }
        bytes.truncate(size);
        Ok(bytes)
    }
}
