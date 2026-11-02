//! Hostile and damaged legacy files: whatever the input, the reader returns an error or a bounded
//! result and never panics, loops or allocates what the file does not hold.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;

use flare_import::legacy::{self, CfbError, LegacyError};
use flare_import::{import_bytes, import_path};

const FREE: u32 = 0xFFFF_FFFF;
const END: u32 = 0xFFFF_FFFE;

/// A version 3 compound file with a FAT sector, a directory sector and `data_sectors` sectors of
/// data, holding one stream `Contents` that starts at sector 2 and claims `size` bytes.
fn compound(fat_tail: &[u32], data_sectors: usize, size: u32) -> Vec<u8> {
    let mut file = vec![0_u8; 512];
    file[..8].copy_from_slice(&legacy::CFB_MAGIC);
    file[24..26].copy_from_slice(&0x3E_u16.to_le_bytes());
    file[26..28].copy_from_slice(&3_u16.to_le_bytes());
    file[28..30].copy_from_slice(&0xFFFE_u16.to_le_bytes());
    file[30..32].copy_from_slice(&9_u16.to_le_bytes());
    file[32..34].copy_from_slice(&6_u16.to_le_bytes());
    file[44..48].copy_from_slice(&1_u32.to_le_bytes());
    file[48..52].copy_from_slice(&1_u32.to_le_bytes());
    file[56..60].copy_from_slice(&4096_u32.to_le_bytes());
    file[60..64].copy_from_slice(&END.to_le_bytes());
    file[68..72].copy_from_slice(&END.to_le_bytes());
    for i in 0..109 {
        let id = if i == 0 { 0 } else { FREE };
        file[76 + i * 4..80 + i * 4].copy_from_slice(&id.to_le_bytes());
    }
    let mut fat = vec![FREE; 128];
    fat[0] = 0xFFFF_FFFD;
    fat[1] = END;
    for (i, &next) in fat_tail.iter().enumerate() {
        fat[2 + i] = next;
    }
    file.extend(fat.iter().flat_map(|v| v.to_le_bytes()));
    let mut dir = vec![0_u8; 512];
    let entry = |dir: &mut [u8], at: usize, name: &str, kind: u8, start: u32, size: u32| {
        let units: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        dir[at..at + units.len()].copy_from_slice(&units);
        dir[at + 64..at + 66].copy_from_slice(&((units.len() + 2) as u16).to_le_bytes());
        dir[at + 66] = kind;
        dir[at + 116..at + 120].copy_from_slice(&start.to_le_bytes());
        dir[at + 120..at + 124].copy_from_slice(&size.to_le_bytes());
    };
    entry(&mut dir, 0, "Root Entry", 5, END, 0);
    entry(&mut dir, 128, "Contents", 2, 2, size);
    file.extend(dir);
    file.extend(vec![0_u8; 512 * data_sectors]);
    file
}

#[test]
fn a_well_formed_synthetic_file_opens_and_has_no_properties() {
    let file = compound(&[3, 4, 5, 6, 7, 8, 9, END], 8, 4096);
    let legacy = legacy::read(&file).expect("a valid container");
    assert_eq!(legacy.info.pages, 0);
    assert!(legacy.properties.is_err());
}

#[test]
fn a_cyclic_chain_is_an_error() {
    // Sector 2 leads to 3 and 3 back to 2, for a stream that claims more bytes than both hold.
    let file = compound(&[3, 2], 10, 5000);
    assert!(matches!(
        legacy::read(&file),
        Err(LegacyError::Container(CfbError::Cycle(_)))
    ));
    // A sector that leads to itself.
    let file = compound(&[2], 10, 5000);
    assert!(matches!(
        legacy::read(&file),
        Err(LegacyError::Container(CfbError::Cycle(_)))
    ));
}

#[test]
fn a_stream_larger_than_the_file_is_refused_before_anything_is_allocated() {
    let file = compound(&[3, END], 10, 60 * 1024 * 1024);
    assert!(matches!(
        legacy::read(&file),
        Err(LegacyError::Container(CfbError::StreamTooLarge { .. }))
    ));
    let file = compound(&[3, END], 10, u32::MAX);
    assert!(matches!(
        legacy::read(&file),
        Err(LegacyError::ContentsTooLarge { .. })
    ));
}

#[test]
fn a_chain_that_ends_early_is_truncated() {
    let file = compound(&[END], 10, 4096);
    assert!(matches!(
        legacy::read(&file),
        Err(LegacyError::Container(CfbError::Truncated(_)))
    ));
}

#[test]
fn a_sector_id_outside_the_file_is_an_error() {
    let file = compound(&[9999], 10, 5000);
    assert!(legacy::read(&file).is_err());
}

#[test]
fn a_compound_file_without_contents_is_not_a_flash_document() {
    let mut file = compound(&[END], 1, 100);
    // Rename the stream.
    file[512 + 512 + 128] = b'X';
    assert_eq!(legacy::read(&file), Err(LegacyError::NoContents));
}

#[test]
fn bad_headers_are_errors() {
    let good = compound(&[END], 1, 100);
    for (at, value) in [
        (26_usize, 9_u8),
        (28, 0),
        (30, 7),
        (32, 5),
        (56, 1),
        (44, 0xFF),
        (72, 0xFF),
    ] {
        let mut file = good.clone();
        file[at] = value;
        if at == 44 || at == 72 {
            file[at + 3] = 0x7F;
        }
        assert!(legacy::read(&file).is_err(), "byte {at} = {value}");
    }
    assert!(matches!(
        legacy::read(b"PK\x03\x04"),
        Err(LegacyError::Container(CfbError::NotCfb))
    ));
    assert!(legacy::read(&legacy::CFB_MAGIC).is_err());
}

fn small_real_file() -> Option<Vec<u8>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../third_party/ruffle/swf/tests/swfs/DefineEditText-MX.fla");
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            eprintln!("skipped: the vendored Ruffle test files are absent");
            None
        }
    }
}

#[test]
fn a_real_file_cut_at_every_length_never_panics() {
    let Some(file) = small_real_file() else {
        return;
    };
    assert!(
        legacy::read(&file)
            .expect("the whole file")
            .properties
            .is_ok()
    );
    let mut readable = 0;
    for len in 0..file.len() {
        // Both the reader and the importer, which wraps it.
        if legacy::read(&file[..len]).is_ok_and(|l| l.properties.is_ok()) {
            readable += 1;
        }
        let _ = import_bytes(file[..len].to_vec());
    }
    // Cutting only the tail of the last sector may still leave a readable document; cutting into
    // the header or the directory may not.
    assert!(
        readable < file.len(),
        "{readable} of {} cuts still read",
        file.len()
    );
    assert!(legacy::read(&file[..511]).is_err());
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

#[test]
fn random_damage_to_a_real_file_never_panics() {
    let Some(file) = small_real_file() else {
        return;
    };
    let mut rng = Rng(0x000F_1A5E);
    for round in 0..3000 {
        let mut damaged = file.clone();
        for _ in 0..=(round % 12) {
            let at = rng.next() as usize % damaged.len();
            damaged[at] = rng.next() as u8;
        }
        // Half the rounds aim at the header and directory, where the structure is.
        if round % 2 == 0 {
            let at = rng.next() as usize % 1024.min(damaged.len());
            damaged[at] = rng.next() as u8;
        }
        if let Ok(imported) = import_bytes(damaged) {
            assert!(imported.document.symbols().len() <= 10_000);
        }
    }
}

#[test]
fn import_path_reports_what_a_file_it_cannot_open_is() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("broken.fla");
    let mut file = compound(&[END], 1, 100);
    file[512 + 512 + 128] = b'X';
    fs::write(&path, file).expect("write");
    let error = import_path(&path).expect_err("no Contents").to_string();
    assert!(error.contains("Contents"), "{error}");
}
