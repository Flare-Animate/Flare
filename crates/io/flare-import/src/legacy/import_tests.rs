#![allow(clippy::expect_used)]

use super::*;

const FREE: u32 = u32::MAX;
const END: u32 = 0xFFFF_FFFE;
const FAT_SECTOR: u32 = 0xFFFF_FFFD;

fn directory_entry(dir: &mut [u8], at: usize, name: &str, kind: u8, start: u32, size: u32) {
    let bytes: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
    dir[at..at + bytes.len()].copy_from_slice(&bytes);
    dir[at + 64..at + 66].copy_from_slice(&((bytes.len() + 2) as u16).to_le_bytes());
    dir[at + 66] = kind;
    dir[at + 116..at + 120].copy_from_slice(&start.to_le_bytes());
    dir[at + 120..at + 124].copy_from_slice(&u64::from(size).to_le_bytes()[..4]);
}

fn compound_with_page(contents: &[u8], page: &[u8]) -> Vec<u8> {
    let contents_sectors = contents.len().div_ceil(512);
    let page_sectors = page.len().div_ceil(512);
    let page_start = 2 + contents_sectors as u32;
    let total_sectors = 2 + contents_sectors + page_sectors;

    let mut file = vec![0_u8; 512];
    file[..8].copy_from_slice(&CFB_MAGIC);
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
    for index in 0..109 {
        let id = if index == 0 { 0 } else { FREE };
        file[76 + index * 4..80 + index * 4].copy_from_slice(&id.to_le_bytes());
    }

    let mut fat = vec![FREE; 128];
    fat[0] = FAT_SECTOR;
    fat[1] = END;
    for sector in 2..page_start {
        fat[sector as usize] = if sector + 1 == page_start {
            END
        } else {
            sector + 1
        };
    }
    for sector in page_start..total_sectors as u32 {
        fat[sector as usize] = if sector + 1 == total_sectors as u32 {
            END
        } else {
            sector + 1
        };
    }
    file.extend(fat.iter().flat_map(|value| value.to_le_bytes()));

    let mut dir = vec![0_u8; 512];
    directory_entry(&mut dir, 0, "Root Entry", 5, END, 0);
    directory_entry(&mut dir, 128, "Contents", 2, 2, contents.len() as u32);
    directory_entry(&mut dir, 256, "Page 1", 2, page_start, page.len() as u32);
    file.extend(dir);
    file.extend_from_slice(contents);
    file.resize(512 * (2 + 1 + contents_sectors), 0);
    file.extend_from_slice(page);
    file.resize(512 * (1 + total_sectors), 0);
    file
}

fn layer_record(name: &str, locked: bool, hidden: bool) -> Vec<u8> {
    let mut bytes = [0, 0, 0, 0, 0, 0x80, 0, 0, 0, 0x80, 11].to_vec();
    let units: Vec<u16> = name.encode_utf16().collect();
    bytes.extend([0xFF, 0xFE, 0xFF, units.len() as u8]);
    bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    bytes.extend([1, u8::from(locked), u8::from(hidden)]);
    bytes
}

#[test]
fn imports_stage_settings_and_bounded_page_layer_metadata() {
    let mut contents = vec![0_u8; 4096];
    contents[..2].copy_from_slice(&0x0129_u16.to_le_bytes());
    contents[104..108].copy_from_slice(&16_000_u32.to_le_bytes());
    contents[112..116].copy_from_slice(&12_000_u32.to_le_bytes());
    contents[153..156].copy_from_slice(&[12, 34, 56]);
    contents[161..165].copy_from_slice(&(24_u32 << 16).to_le_bytes());
    contents[167..172].copy_from_slice(&[3, 0xB4, 0, 0, 0]);

    let mut page = vec![0_u8; 4096];
    page.extend(layer_record("Layer 1", true, true));
    page.extend(layer_record("Guide: Motion", false, false));
    page.resize(5120, 0);
    let imported = import_legacy_bytes(&compound_with_page(&contents, &page)).expect("import");
    let doc = imported.document;

    assert_eq!((doc.stage().width(), doc.stage().height()), (800, 600));
    assert_eq!(doc.frame_rate().per_second(), 24.0);
    assert_eq!(doc.scenes().len(), 1);
    let layers = doc.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].name(), "Layer 1");
    assert!(layers[0].is_locked() && !layers[0].is_visible());
    assert_eq!(layers[1].name(), "Guide: Motion");
    assert!(layers[1].is_guide() && layers[1].is_visible());
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("artwork, keyframe contents") }),
        "unsupported content is reported"
    );
}
