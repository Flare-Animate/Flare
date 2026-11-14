//! Running ffmpeg with a time limit. Its pipes are read and written here, its errors are kept to
//! their last lines, and a run that goes past [`TIME_LIMIT`] is stopped by a watchdog thread.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::error::FfmpegError;

/// How long one run may take before it is stopped.
pub(crate) const TIME_LIMIT: Duration = Duration::from_secs(900);
/// The most of ffmpeg's error output that is read; the end of it is what is reported.
const STDERR_READ: u64 = 1 << 20;
/// The lines of ffmpeg's error output that are reported.
const STDERR_LINES: usize = 4;

/// A child process with its error output being read on a thread of its own.
struct Running {
    child: Arc<Mutex<Child>>,
    done: Arc<AtomicBool>,
    timed_out: Arc<AtomicBool>,
    stderr: std::thread::JoinHandle<Vec<u8>>,
}

fn lock(child: &Mutex<Child>) -> MutexGuard<'_, Child> {
    child
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn spawn(
    program: &Path,
    args: &[std::ffi::OsString],
    stdin: Stdio,
    stdout: Stdio,
) -> Result<Running, FfmpegError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr_pipe = child.stderr.take();
    let stderr = std::thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(pipe) = stderr_pipe {
            let _ = pipe.take(STDERR_READ).read_to_end(&mut text);
        }
        text
    });
    let child = Arc::new(Mutex::new(child));
    let done = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    {
        let (child, done, timed_out) = (
            Arc::clone(&child),
            Arc::clone(&done),
            Arc::clone(&timed_out),
        );
        std::thread::spawn(move || {
            let deadline = Instant::now() + TIME_LIMIT;
            while !done.load(Ordering::SeqCst) {
                if Instant::now() >= deadline {
                    timed_out.store(true, Ordering::SeqCst);
                    let _ = lock(&child).kill();
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }
    Ok(Running {
        child,
        done,
        timed_out,
        stderr,
    })
}

impl Running {
    fn kill(&self) {
        let _ = lock(&self.child).kill();
    }

    /// Waits for the child to end and says how it went.
    fn finish(self, action: &'static str) -> Result<(), FfmpegError> {
        let status = loop {
            if let Some(status) = lock(&self.child).try_wait()? {
                break status;
            }
            if self.timed_out.load(Ordering::SeqCst) {
                break lock(&self.child).wait()?;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        self.done.store(true, Ordering::SeqCst);
        let stderr = self.stderr.join().unwrap_or_default();
        if self.timed_out.load(Ordering::SeqCst) {
            return Err(FfmpegError::TimedOut(TIME_LIMIT.as_secs()));
        }
        if status.success() {
            return Ok(());
        }
        Err(FfmpegError::Failed {
            action,
            detail: last_lines(&stderr),
        })
    }
}

/// The last few non-empty lines of ffmpeg's error output, or a note that it said nothing.
fn last_lines(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return "it stopped without saying why".to_owned();
    }
    lines[lines.len().saturating_sub(STDERR_LINES)..].join(" ")
}

/// Runs `program` with `args` and returns what it wrote to its output, at most `max_bytes` of it.
pub(crate) fn capture(
    program: &Path,
    args: &[std::ffi::OsString],
    action: &'static str,
    max_bytes: u64,
) -> Result<Vec<u8>, FfmpegError> {
    let run = spawn(program, args, Stdio::null(), Stdio::piped())?;
    // The pipe is taken out first: the lock is not held while reading, so a stop can always reach
    // the child.
    let stdout = lock(&run.child).stdout.take();
    let mut out = Vec::new();
    if let Some(stdout) = stdout {
        stdout.take(max_bytes + 1).read_to_end(&mut out)?;
    }
    if out.len() as u64 > max_bytes {
        run.kill();
        run.finish(action)?;
        return Err(FfmpegError::Refused(format!(
            "ffmpeg wrote more than {max_bytes} bytes; the movie is too large to read"
        )));
    }
    run.finish(action)?;
    Ok(out)
}

/// Runs `program` with `args` and hands what it writes to `each`, `chunk` bytes at a time, until
/// it ends, `limit` chunks have come or `each` returns false. A short piece at the very end is not
/// a chunk and is dropped. Returns how many chunks were handed over; a stop asked for by `each` is
/// [`FfmpegError::Stopped`], and the program is ended either way.
pub(crate) fn stream(
    program: &Path,
    args: &[std::ffi::OsString],
    action: &'static str,
    chunk: usize,
    limit: u64,
    each: &mut dyn FnMut(Vec<u8>) -> bool,
) -> Result<u64, FfmpegError> {
    if chunk == 0 {
        return Err(FfmpegError::Refused("a picture has no bytes".to_owned()));
    }
    let run = spawn(program, args, Stdio::null(), Stdio::piped())?;
    let stdout = lock(&run.child).stdout.take();
    let (mut count, mut stopped) = (0_u64, false);
    if let Some(mut stdout) = stdout {
        while count < limit {
            let mut piece = vec![0_u8; chunk];
            match fill(&mut stdout, &mut piece) {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    run.kill();
                    let _ = run.finish(action);
                    return Err(error.into());
                }
            }
            count += 1;
            if !each(piece) {
                stopped = true;
                break;
            }
        }
    }
    if stopped || count >= limit {
        // Ended here, so how it exits says nothing about the movie.
        run.kill();
        let _ = run.finish(action);
        return if stopped {
            Err(FfmpegError::Stopped)
        } else {
            Ok(count)
        };
    }
    run.finish(action)?;
    Ok(count)
}

/// Fills `piece` from `from`: true when it is full, false when the output ended first.
fn fill(from: &mut impl Read, piece: &mut [u8]) -> std::io::Result<bool> {
    let mut filled = 0;
    while filled < piece.len() {
        match from.read(&mut piece[filled..]) {
            Ok(0) => return Ok(false),
            Ok(n) => filled += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

/// Runs `program` with `args`, letting `write` feed its input, and waits for it to end. When
/// ffmpeg fails the input is cut short, so its own error is the one reported.
pub(crate) fn feed(
    program: &Path,
    args: &[std::ffi::OsString],
    action: &'static str,
    write: impl FnOnce(&mut dyn Write) -> Result<(), FfmpegError>,
) -> Result<(), FfmpegError> {
    let run = spawn(program, args, Stdio::piped(), Stdio::null())?;
    let stdin = lock(&run.child).stdin.take();
    let outcome = match stdin {
        Some(mut stdin) => write(&mut stdin),
        None => Err(FfmpegError::Refused("ffmpeg has no input".to_owned())),
    };
    match (outcome, run.finish(action)) {
        (_, Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}
