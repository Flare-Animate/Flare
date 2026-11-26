//! `flare bitmap-compression`: how the library's bitmaps are stored, lossless or as photos
//! (JPEG), as Animate's Bitmap Properties choose it. What changes is
//! `flare_tools::library::set_bitmap_compression`'s; this reads the arguments, writes the
//! project and prints what it saved.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Args, ValueEnum};
use flare_core::BitmapId;
use flare_document::DEFAULT_PHOTO_QUALITY;
use flare_tools::library::{Compression, set_bitmap_compression};
use serde_json::{Value, json};

#[derive(Clone, Copy, ValueEnum)]
pub enum How {
    /// As JPEGs: about a ninth of the room for a frame of video, which looks the same.
    Photo,
    /// Every pixel exactly.
    Lossless,
}

/// What `flare bitmap-compression` is asked.
#[derive(Args)]
pub struct BitmapCompressionArgs {
    path: PathBuf,
    #[arg(value_enum)]
    how: How,
    /// For `photo`: the JPEG quality, 1 to 100 (92 by default).
    #[arg(long)]
    quality: Option<u8>,
    /// Only this bitmap (repeat for more); every bitmap in the library by default.
    #[arg(long = "bitmap")]
    bitmaps: Vec<uuid::Uuid>,
}

pub fn run(args: BitmapCompressionArgs) -> Result<Value> {
    let compression = match (args.how, args.quality) {
        (How::Photo, quality) => Compression::Photo {
            quality: quality.unwrap_or(DEFAULT_PHOTO_QUALITY),
        },
        (How::Lossless, None) => Compression::Lossless,
        (How::Lossless, Some(_)) => {
            return Err(anyhow!(
                "--quality is for photo; lossless keeps every pixel"
            ));
        }
    };
    let which: Vec<BitmapId> = args.bitmaps.into_iter().map(BitmapId::from_uuid).collect();
    let mut session = crate::project::open_session(&args.path)?;
    let done =
        set_bitmap_compression(session.document(), &which, compression).map_err(|e| anyhow!(e))?;
    let file_before = std::fs::metadata(&args.path).map(|m| m.len()).ok();
    if !done.commands.is_empty() {
        session.apply(done.commands)?;
        crate::project::commit_session(&session, &args.path)?;
    }
    let file_after = std::fs::metadata(&args.path).map(|m| m.len()).ok();
    Ok(json!({
        "changed": done.changed,
        "unchanged": done.unchanged,
        "kept_lossless_not_smaller": done.not_smaller,
        "bitmap_bytes_before": done.bytes_before,
        "bitmap_bytes_after": done.bytes_after,
        "file_bytes_before": file_before,
        "file_bytes_after": file_after,
    }))
}
