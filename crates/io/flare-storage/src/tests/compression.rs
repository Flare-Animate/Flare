//! What the compression does and what it must never allow.

use super::*;
use crate::codec::{Codec, NativeZstd, Packer, frame_header, unpack};

fn stored(path: &Path) -> Vec<(String, String, String, i64, usize)> {
    let conn = Connection::open(path).expect("open");
    let mut statement = conn
        .prepare("SELECT id, kind, codec, size, length(payload) FROM entity ORDER BY kind, ordinal")
        .expect("prepare");
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get::<_, i64>(4)? as usize,
            ))
        })
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

/// Flips the low bit of byte `at` of the stored payload of row `id`, leaving the digest and
/// everything else alone. False when the payload is shorter.
fn flip_stored_byte(path: &Path, id: &str, at: usize) -> bool {
    let conn = Connection::open(path).expect("open");
    let mut bytes: Vec<u8> = conn
        .query_row("SELECT payload FROM entity WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .expect("payload");
    if at >= bytes.len() {
        return false;
    }
    bytes[at] ^= 1;
    conn.execute(
        "UPDATE entity SET payload = ?1 WHERE id = ?2",
        params![bytes, id],
    )
    .expect("write");
    true
}

/// A document with a lot of repetitive drawing: 12 layers of 60 one-frame shapes.
fn busy() -> Document {
    let layers: Vec<Layer> = (0..12)
        .map(|l| {
            let spans = (0..60)
                .map(|s| {
                    let shape = Shape::rect(
                        ShapeId::new(),
                        f64::from(s) * 3.0,
                        f64::from(l) * 5.0,
                        20.0,
                        10.0,
                        Rgba8::new(200, 40, 40, 255),
                    )
                    .expect("rect");
                    FrameSpan::new(s, NonZeroU32::MIN, Content::Shape(shape))
                })
                .collect();
            Layer::new(LayerId::new(), format!("Layer {l}"), spans).expect("layer")
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn a_busy_project_is_a_small_file_and_a_tiny_one_is_a_tiny_database() {
    let dir = tempfile::tempdir().expect("dir");
    let big = dir.path().join("busy.flare");
    let document = busy();
    save(&document, &big).expect("save");
    let raw: usize = rows(&big).iter().map(|r| r.record.len()).sum();
    let size = fs::metadata(&big).expect("meta").len() as usize;
    assert!(raw > 20_000, "there is something to squeeze: {raw}");
    assert!(
        size < raw / 3,
        "the file ({size}) is under a third of its records ({raw})"
    );
    assert_eq!(load(&big).expect("load"), document);

    // The database's own cost: a handful of 1 KiB pages, not a handful of 4 KiB ones.
    let tiny = dir.path().join("tiny.flare");
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&empty, &tiny).expect("save");
    let small = fs::metadata(&tiny).expect("meta").len();
    assert!(small <= 4 * 1024, "an empty project is {small} bytes");
    let conn = Connection::open(&tiny).expect("open");
    let page: i64 = conn
        .pragma_query_value(None, "page_size", |row| row.get(0))
        .expect("page size");
    assert_eq!(page, PAGE_SIZE);
}

#[test]
fn a_record_is_compressed_only_when_that_makes_it_smaller() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&busy(), &path).expect("save");
    let rows = stored(&path);
    let scene = rows.iter().find(|r| r.1 == "scene").expect("a scene");
    assert_eq!(scene.2, "cbor+zstd");
    assert!((scene.4 as i64) < scene.3, "stored smaller than its size");
    // A record under the threshold, and noise, stay as they are.
    let mut zstd = NativeZstd::new().expect("zstd");
    let mut packer = Packer::new(&mut zstd);
    let short = packer.pack(&[7u8; 20]).expect("pack");
    assert_eq!(short.codec, Codec::Plain);
    let mut state = 99u64;
    let noise: Vec<u8> = (0..4000).map(|_| (lcg(&mut state) & 0xff) as u8).collect();
    assert_eq!(packer.pack(&noise).expect("pack").codec, Codec::Plain);
    assert_eq!(packer.pack(&[0u8; 4000]).expect("pack").codec, Codec::Zstd);
}

#[test]
fn a_sound_is_stored_as_its_own_bytes_not_as_text_made_from_them() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let mut state = 7u64;
    let noise: Vec<u8> = (0..300_000)
        .map(|_| (lcg(&mut state) & 0xff) as u8)
        .collect();
    let sound = flare_document::Sound::new(
        flare_core::SoundId::new(),
        "noise",
        noise.clone(),
        44_100,
        2,
        1000,
    )
    .expect("sound");
    let id = sound.id();
    let document = busy_with(sound, id);
    save(&document, &path).expect("save");
    let size = fs::metadata(&path).expect("meta").len() as usize;
    // Base64 text would be a third bigger than the bytes; this is within a few percent of them.
    assert!(
        size < noise.len() * 105 / 100 + 20_000,
        "{size} for {} bytes of noise",
        noise.len()
    );
    assert_eq!(
        load(&path).expect("load").sounds()[0].data(),
        noise.as_slice()
    );
}

fn busy_with(sound: flare_document::Sound, id: flare_core::SoundId) -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![])
        .expect("layer")
        .with_cues(vec![flare_document::SoundCue {
            start: 0,
            sound: id,
        }])
        .expect("cues");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_sounds(vec![sound])
    .expect("sounds")
}

#[test]
fn a_record_that_does_not_match_its_digest_is_refused_whatever_else_is_true_of_it() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&busy(), &path).expect("save");
    // Flip one byte of the stored payload and leave everything else alone.
    let scene = stored(&path)
        .into_iter()
        .find(|r| r.1 == "scene")
        .expect("a scene")
        .0;
    assert!(flip_stored_byte(&path, &scene, 20));
    let error = load(&path).expect_err("damaged");
    assert!(
        matches!(&error, StorageError::Corrupt(why) if why.contains("digest")),
        "{error}"
    );
}

#[test]
fn the_stated_size_the_codec_and_the_frame_are_checked_before_they_are_trusted() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let update = |sql: &str| {
        save(&busy(), &path).expect("save");
        let conn = Connection::open(&path).expect("open");
        conn.execute(sql, []).expect("update");
    };
    // A size over the limit is refused before anything is allocated.
    update("UPDATE entity SET size = 4000000000 WHERE kind = 'scene'");
    assert!(matches!(load(&path), Err(StorageError::Limit(_))));
    // A size that is smaller than the real thing cannot be a way to bring a bigger one in: the
    // decompressor is stopped at the stated size.
    update("UPDATE entity SET size = 100 WHERE kind = 'scene'");
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    // A size larger than the real thing is a lie as well.
    update("UPDATE entity SET size = size + 1 WHERE kind = 'scene'");
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    save(&busy(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    assert!(
        conn.execute("UPDATE entity SET size = -1 WHERE kind = 'scene'", [])
            .is_err(),
        "the table refuses a negative size"
    );
    drop(conn);
    // A record named for a codec this build does not have is refused by the table itself or by
    // the loader, never guessed.
    save(&busy(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    assert!(
        conn.execute("UPDATE entity SET codec = 'lzma' WHERE kind = 'scene'", [])
            .is_err(),
        "the table only allows the codecs there are"
    );
    // The same checks, without a database in the way.
    let raw = to_cbor_for_test();
    let mut zstd = NativeZstd::new().expect("zstd");
    let stored = Packer::new(&mut zstd).pack(&raw).expect("pack");
    let ok = unpack(
        &mut zstd,
        stored.codec.name(),
        stored.size as i64,
        &stored.digest,
        &stored.bytes,
        1 << 20,
    );
    assert_eq!(ok.expect("unpacks"), raw);
    assert!(unpack(&mut zstd, "lzma", 1, &stored.digest, &stored.bytes, 1 << 20).is_err());
    let mut wrong = stored.digest;
    wrong[0] ^= 1;
    assert!(
        unpack(
            &mut zstd,
            stored.codec.name(),
            stored.size as i64,
            &wrong,
            &stored.bytes,
            1 << 20
        )
        .is_err()
    );
}

fn to_cbor_for_test() -> Vec<u8> {
    crate::codec::to_cbor(&vec!["compressible"; 200]).expect("cbor")
}

#[test]
fn a_record_cannot_be_a_decompression_bomb_or_a_nesting_bomb() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    // 64 MiB of zeros compress to a few kilobytes; stated honestly the loader refuses the size,
    // and stated as small the decompressor refuses to go past it.
    let bomb = vec![0u8; 64 * 1024 * 1024];
    let frame = zstd::bulk::compress(&bomb, 3).expect("compress");
    assert!(frame.len() < 10_000, "a real bomb: {} bytes", frame.len());
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    for stated in [bomb.len() as i64, 1000] {
        conn.execute(
            "UPDATE entity SET codec = 'cbor+zstd', size = ?1, digest = ?2, payload = ?3 WHERE kind = 'scene'",
            params![stated, blake3::hash(&frame).as_bytes().as_slice(), frame],
        )
        .expect("plant");
        assert!(load(&path).is_err(), "stated size {stated}");
    }
    drop(conn);
    // Arrays nested a hundred thousand deep: CBOR for that is short, and decoding it must stop.
    let mut deep = vec![0x81u8; 100_000];
    deep.push(0x00);
    save(&sample(), &path).expect("save");
    set_raw(&path, "scene", &deep);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    // A map that claims four billion entries and ends.
    set_raw(
        &path,
        "scene",
        &[0xbb, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    );
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn every_flipped_byte_in_the_stored_records_is_caught() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&busy(), &path).expect("save");
    let ids: Vec<String> = stored(&path).into_iter().map(|r| r.0).collect();
    let mutant = dir.path().join("m.flare");
    let mut caught = 0;
    for id in &ids {
        for at in [0usize, 6, 49, 199] {
            fs::copy(&path, &mutant).expect("copy");
            if flip_stored_byte(&mutant, id, at) {
                assert!(load(&mutant).is_err(), "byte {at} of {id}");
                caught += 1;
            }
        }
    }
    assert!(caught >= 4, "something was flipped: {caught}");
}

#[test]
fn the_limit_on_one_record_is_the_one_the_documentation_names() {
    assert_eq!(MAX_PAYLOAD_BYTES, 256 * 1024 * 1024);
}

/// A zstd that must never be asked to decompress: the checks before it have to refuse first.
struct NeverAsked;

impl crate::codec::Zstd for NeverAsked {
    fn compress(&mut self, _: &[u8]) -> Result<Vec<u8>, StorageError> {
        unreachable!("not asked to compress")
    }
    fn decompress(&mut self, _: &[u8], _: usize) -> Result<Vec<u8>, StorageError> {
        panic!("the decompressor was reached by a frame that should have been refused");
    }
}

fn frame_of(raw: &[u8]) -> Vec<u8> {
    NativeZstd::new().expect("zstd").compress_for_test(raw)
}

impl NativeZstd {
    fn compress_for_test(&mut self, raw: &[u8]) -> Vec<u8> {
        use crate::codec::Zstd;
        self.compress(raw).expect("compress")
    }
}

#[test]
fn the_frame_header_is_read_the_way_the_format_says_for_every_size_of_record() {
    for len in [64usize, 300, 5_000, 70_000, 400_000, 3_000_000] {
        let raw: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let header = frame_header(&frame_of(&raw)).expect("a zstd frame");
        assert_eq!(header.content_size, Some(len as u64), "{len} bytes");
        assert!(header.checksum && !header.dictionary, "{len}");
        assert!(
            header.window <= 1 << 18,
            "window {} for {len}",
            header.window
        );
    }
}

#[test]
fn a_frame_that_lies_or_is_not_what_this_format_writes_is_refused_before_it_is_decompressed() {
    let raw = vec![7u8; 5000];
    let good = frame_of(&raw);
    let check = |frame: &[u8], size: i64| {
        let digest = *blake3::hash(frame).as_bytes();
        unpack(&mut NeverAsked, "cbor+zstd", size, &digest, frame, 1 << 20)
    };
    // The header says 5000; the record says otherwise, either way.
    assert!(matches!(check(&good, 4000), Err(StorageError::Corrupt(_))));
    assert!(matches!(check(&good, 6000), Err(StorageError::Corrupt(_))));
    // Not a zstd frame at all, and a truncated header.
    assert!(check(b"plain bytes that are not a frame", 5000).is_err());
    assert!(check(&good[..5], 5000).is_err());
    // A frame with no content size (a streaming frame) cannot be checked against the size.
    let mut no_size = good.clone();
    no_size[4] &= 0b0011_1111; // size flag 0
    no_size[4] &= !0x20; // not single segment: so no size field at all
    assert!(check(&no_size, 5000).is_err());
    // A reserved bit, a dictionary, and no checksum.
    let mut reserved = good.clone();
    reserved[4] |= 0x08;
    assert!(check(&reserved, 5000).is_err());
    let mut dictionary = good.clone();
    dictionary[4] |= 0x01;
    assert!(check(&dictionary, 5000).is_err());
    let mut unchecked = good.clone();
    unchecked[4] &= !0x04;
    assert!(check(&unchecked, 5000).is_err());
    // A window bigger than the format writes: byte 5 is the window descriptor when the frame is
    // not a single segment; build one that is by hand, with a 128 MiB window.
    let big_window = [
        0x28,
        0xB5,
        0x2F,
        0xFD,
        0b0000_0100,
        0xC8,
        0x01,
        0x00,
        0x00,
        0x00,
    ];
    let header = frame_header(&big_window);
    assert!(
        header.as_ref().is_some_and(|h| h.window > 1 << 18),
        "{header:?}"
    );
    assert!(check(&big_window, 0).is_err());
    // And no header at all is no frame.
    assert!(frame_header(&[]).is_none() && frame_header(&[0x28, 0xB5, 0x2F]).is_none());
}
