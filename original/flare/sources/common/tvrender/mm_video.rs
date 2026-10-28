//! Import Video's frames: ffmpeg, when it's on the PATH, turns a video into a PNG image
//! for each frame of the document.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::Result;

/// Whether ffmpeg runs.
pub fn ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").stdin(Stdio::null()).output().is_ok_and(|o| o.status.success())
}

/// The video at `path`, `fps` frames a second, as PNG files' bytes.
// ponytail: every frame is held in memory; stream them in if long videos come in.
pub fn frames(path: &Path, fps: f32) -> Result<Vec<Vec<u8>>> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("minnamate-video-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let run = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vf", &format!("fps={fps}"), "-f", "image2"])
        .arg(dir.join("%06d.png"))
        .stdin(Stdio::null())
        .output();
    let read = || -> Result<Vec<Vec<u8>>> {
        let out = run.map_err(|e| format!("ffmpeg didn't run: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string().into());
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir)?.filter_map(|e| Some(e.ok()?.path())).collect();
        files.sort();
        files.iter().map(|f| Ok(std::fs::read(f)?)).collect()
    };
    let frames = read();
    let _ = std::fs::remove_dir_all(&dir);
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_come_at_the_documents_rate() {
        if !ffmpeg() {
            eprintln!("skipped: no ffmpeg on the PATH");
            return;
        }
        let dir = std::env::temp_dir().join(format!("minnamate-video-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let video = dir.join("clip.mp4");
        let made = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc=duration=0.5:size=32x24:rate=10"])
            .arg(&video)
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(made.success());
        let frames = frames(&video, 20.0).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(frames.len(), 10, "half a second at 20 fps");
        assert!(frames.iter().all(|f| f.starts_with(b"\x89PNG")));
        assert!(super::frames(Path::new("no such video.mp4"), 24.0).is_err());
    }
}
