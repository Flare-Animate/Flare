//! Running Rhubarb Lip Sync, a program of its own that is never bundled or linked: found on the
//! computer, handed the voice as a WAV file in a directory of its own, run with its arguments as
//! a list (no shell reads a name), stopped after [`RHUBARB_TIME_LIMIT`], and read only as far as
//! `flare_audio::lipsync::MAX_RHUBARB_OUTPUT_BYTES`. A browser has no programs to run.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use flare_audio::Audio;

/// The environment variable that names the Rhubarb program to run in place of the one next to
/// Flare or on the PATH.
pub const RHUBARB_ENV: &str = "FLARE_RHUBARB";
/// How long one analysis may take: Rhubarb reads about ten times faster than speech, so ten
/// minutes of voice, the longest sound, is well inside it.
pub const RHUBARB_TIME_LIMIT: Duration = Duration::from_secs(600);
/// The sample rate the voice is handed over at: Rhubarb's recognizer works at 16 kHz.
#[cfg(not(target_arch = "wasm32"))]
const RATE: u32 = 16_000;
/// The most of Rhubarb's error output that is read and reported.
#[cfg(not(target_arch = "wasm32"))]
const STDERR_READ: u64 = 64 * 1024;

/// The installed Rhubarb: the program [`RHUBARB_ENV`] names, else `rhubarb` beside this program
/// (or in a `rhubarb` folder there, as its release archive unpacks), else on the PATH.
pub fn locate_rhubarb() -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    choose_rhubarb(
        std::env::var_os(RHUBARB_ENV),
        beside.as_deref(),
        std::env::var_os("PATH"),
    )
}

/// The Rhubarb to run, given what [`RHUBARB_ENV`] holds, the folder this program is in and the
/// PATH. A program named in [`RHUBARB_ENV`] is the only one looked at: when it is not a file
/// there is no Rhubarb, rather than some other `rhubarb` on the PATH running in its place, which
/// whoever could write to a folder on the PATH would choose. An empty value names nothing.
pub(super) fn choose_rhubarb(
    named: Option<OsString>,
    beside: Option<&Path>,
    path: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(named) = named.filter(|named| !named.is_empty()) {
        let named = PathBuf::from(named);
        return named.is_file().then_some(named);
    }
    let name = if cfg!(windows) {
        "rhubarb.exe"
    } else {
        "rhubarb"
    };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = beside {
        candidates.push(dir.join(name));
        candidates.push(dir.join("rhubarb").join(name));
    }
    if let Some(path) = path {
        candidates.extend(std::env::split_paths(&path).map(|dir| dir.join(name)));
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// Rhubarb's JSON output for `audio`, or what went wrong in words.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run(program: &Path, audio: &Audio) -> Result<Vec<u8>, String> {
    run_with_limit(program, audio, RHUBARB_TIME_LIMIT)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn run(_program: &Path, _audio: &Audio) -> Result<Vec<u8>, String> {
    Err("a browser cannot run programs; use the built-in analysis".to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run_with_limit(
    program: &Path,
    audio: &Audio,
    limit: Duration,
) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let wav = voice_wav(audio)?;
    let dir = Private::new()?;
    let input = dir.0.join("voice.wav");
    std::fs::write(&input, wav).map_err(|e| format!("cannot write the voice for it: {e}"))?;
    let mut child = Command::new(program)
        .args(["--exportFormat", "json", "--quiet"])
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", program.display()))?;
    let max = flare_audio::lipsync::MAX_RHUBARB_OUTPUT_BYTES as u64;
    let stdout = child.stdout.take().map(|pipe| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = pipe.take(max + 1).read_to_end(&mut out);
            out
        })
    });
    let stderr = child.stderr.take().map(|pipe| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = pipe.take(STDERR_READ).read_to_end(&mut out);
            out
        })
    });
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("it took longer than {} seconds", limit.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => break Err(e.to_string()),
        }
    };
    let out = stdout.and_then(|t| t.join().ok()).unwrap_or_default();
    let said = stderr.and_then(|t| t.join().ok()).unwrap_or_default();
    let status = status?;
    if !status.success() {
        let said = String::from_utf8_lossy(&said);
        let said = said.trim();
        return Err(if said.is_empty() {
            format!("it stopped with {status}")
        } else {
            said.lines().last().unwrap_or(said).to_owned()
        });
    }
    if out.len() as u64 > max {
        return Err("it wrote more than Flare reads".to_owned());
    }
    Ok(out)
}

/// The voice as Rhubarb reads it: one channel of 16-bit samples at 16 kHz.
#[cfg(not(target_arch = "wasm32"))]
fn voice_wav(audio: &Audio) -> Result<Vec<u8>, String> {
    let channels = usize::from(audio.channels());
    let mono: Vec<f32> = audio
        .samples()
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    let too_long = || "the sound is too long to hand over".to_owned();
    let mono = Audio::new(audio.sample_rate(), 1, mono).ok_or_else(too_long)?;
    let (pcm, _) = mono
        .to_pcm16(RATE, flare_audio::MAX_DECODED_SAMPLES)
        .ok_or_else(too_long)?;
    flare_audio::wav_pcm16(&pcm, RATE, 1).ok_or_else(too_long)
}

/// A directory only this run uses, removed with everything in it when dropped.
#[cfg(not(target_arch = "wasm32"))]
struct Private(PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Private {
    fn new() -> Result<Self, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let name = format!(
            "flare-rhubarb-{}-{}-{stamp}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(name);
        // Only Unix sets a mode on the builder; elsewhere it is never changed.
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        // `create`, not `create_all`: a directory already there is someone else's.
        builder
            .create(&dir)
            .map_err(|e| format!("cannot make a directory for it: {e}"))?;
        Ok(Self(dir))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Private {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
