//! Pure-std P2P remote transport (port of flare/sources/remote/remoteprotocol.h).
//! UDP hole-punch: both peers fire at each other's public addr until one
//! datagram lands; first frame must be "hello <code>". No libdatachannel.
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use std::ffi::c_char;

pub const ALPHA: &[u8] = b"23456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn make_pairing_code(seed: &mut u64) -> String {
    (0..6).map(|_| {
        *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17;
        ALPHA[(*seed % 30) as usize] as char
    }).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb { Invalid, Hello, Play, Stop, Frame, Command, Cell }
const VERBS: [(Verb, &str); 6] = [(Verb::Hello, "hello"), (Verb::Play, "play"),
    (Verb::Stop, "stop"), (Verb::Frame, "frame"), (Verb::Command, "cmd"), (Verb::Cell, "cell")];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg { pub verb: Verb, pub arg: String }

pub fn encode(m: &Msg) -> String {
    let n = VERBS.iter().find(|v| v.0 == m.verb).map_or("", |v| v.1);
    if m.arg.is_empty() { n.into() } else { format!("{n} {}", m.arg) }
}
pub fn decode(s: &str) -> Msg {
    let (n, a) = s.split_once(' ').unwrap_or((s, ""));
    let verb = VERBS.iter().find(|v| v.1 == n).map_or(Verb::Invalid, |v| v.0);
    let needs = matches!(verb, Verb::Hello | Verb::Frame | Verb::Command | Verb::Cell);
    if verb == Verb::Invalid || needs == a.is_empty() { return Msg { verb: Verb::Invalid, arg: String::new() }; }
    Msg { verb, arg: a.into() }
}

pub struct Peer { pub sock: UdpSocket, pub remote: SocketAddr }

impl Peer {
    /// Hole-punch to `remote`, exchange hello with `code`. Fails on mismatch/timeout.
    pub fn connect(sock: UdpSocket, remote: SocketAddr, code: &str, timeout: Duration) -> std::io::Result<Peer> {
        sock.set_read_timeout(Some(Duration::from_millis(50)))?;
        let hello = encode(&Msg { verb: Verb::Hello, arg: code.into() });
        let end = Instant::now() + timeout;
        let mut buf = [0u8; 1500];
        let (mut got, mut acked) = (false, false);
        while Instant::now() < end {
            sock.send_to(if got { b"ack" } else { hello.as_bytes() }, remote)?;
            if got { sock.send_to(hello.as_bytes(), remote)?; }
            if let Ok((n, from)) = sock.recv_from(&mut buf) {
                if from != remote { continue; }
                match &buf[..n] {
                    b"ack" => acked = true,
                    d => { let m = decode(&String::from_utf8_lossy(d));
                        if m.verb != Verb::Hello { continue; }
                        if m.arg != code { return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "bad code")); }
                        got = true; }
                }
                if got && acked { for _ in 0..3 { sock.send_to(b"ack", remote)?; } return Ok(Peer { sock, remote }); }
            }
        }
        Err(std::io::ErrorKind::TimedOut.into())
    }
    pub fn send(&self, m: &Msg) -> std::io::Result<()> { self.sock.send_to(encode(m).as_bytes(), self.remote).map(|_| ()) }
    /// Next valid non-hello message, or None on timeout.
    pub fn recv(&self) -> Option<Msg> { self.recv_timeout(Duration::from_millis(2000)) }
    /// Next message, giving up after `budget`. `recv` is `recv_timeout(2s)`.
    pub fn recv_timeout(&self, budget: Duration) -> Option<Msg> {
        let deadline = Instant::now() + budget;
        let mut buf = [0u8; 1500];
        while Instant::now() < deadline {
            if let Ok((n, f)) = self.sock.recv_from(&mut buf) {
                let m = decode(&String::from_utf8_lossy(&buf[..n]));
                if f == self.remote && !matches!(m.verb, Verb::Invalid | Verb::Hello) { return Some(m); }
            }
        }
        None
    }
}

// ---- C ABI ---------------------------------------------------------------
// flare/sources/remote/remotetransport.h declares this shape; the C++ remote
// dialog uses it so the shipped app drives the same transport the tests cover.
// A NULL handle means "no peer": wrong code, or the timeout expired. Nothing
// here throws, and no pointer crosses the boundary that the caller does not own.

use std::ffi::CStr;

/// Opaque connected session. Owned by the caller; free with flare_remote_close.
pub struct FlareRemote {
    peer: Peer,
    port: u16,
}

fn cstr(p: *const c_char) -> Option<String> {
    if p.is_null() { return None; }
    unsafe { CStr::from_ptr(p) }.to_str().ok().map(str::to_owned)
}

unsafe fn msg_of(verb: &str, arg: &str) -> Msg {
    Msg { verb: VERBS.iter().find(|v| v.1 == verb).map_or(Verb::Invalid, |v| v.0), arg: arg.into() }
}

/// Announce `code` on `port` (0 = ephemeral) and wait `timeout_ms` for a peer.
/// Returns NULL on timeout or a code mismatch.
#[no_mangle]
pub unsafe extern "C" fn flare_remote_host(
    port: u16, code: *const c_char, timeout_ms: u32,
) -> *mut FlareRemote {
    let Some(code) = cstr(code) else { return std::ptr::null_mut() };
    let Ok(sock) = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port))) else { return std::ptr::null_mut() };
    let bound = sock.local_addr().map(|a| a.port()).unwrap_or(port);
    // Accept from whoever answers the code: learn the peer from the first datagram.
    let Ok(remote) = wait_for_hello(&sock, &code, Duration::from_millis(timeout_ms as u64)) else { return std::ptr::null_mut() };
    match Peer::connect(sock, remote, &code, Duration::from_millis(timeout_ms as u64)) {
        Ok(peer) => Box::into_raw(Box::new(FlareRemote { peer, port: bound })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Dial `host:port` as the joining peer.
#[no_mangle]
pub unsafe extern "C" fn flare_remote_join(
    host: *const c_char, port: u16, code: *const c_char, timeout_ms: u32,
) -> *mut FlareRemote {
    let (Some(host), Some(code)) = (cstr(host), cstr(code)) else { return std::ptr::null_mut() };
    let Ok(ip) = host.parse::<std::net::IpAddr>() else { return std::ptr::null_mut() };
    let Ok(sock) = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))) else { return std::ptr::null_mut() };
    match Peer::connect(sock, SocketAddr::new(ip, port), &code, Duration::from_millis(timeout_ms as u64)) {
        Ok(peer) => Box::into_raw(Box::new(FlareRemote { peer, port })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// The port actually bound, for the UI to advertise. 0 when `s` is NULL.
#[no_mangle]
pub unsafe extern "C" fn flare_remote_port(s: *const FlareRemote) -> u16 {
    if s.is_null() { 0 } else { (*s).port }
}

/// Send one message. 0 on success, -1 on a NULL handle or send failure.
#[no_mangle]
pub unsafe extern "C" fn flare_remote_send(
    s: *mut FlareRemote, verb: *const c_char, arg: *const c_char,
) -> i32 {
    if s.is_null() { return -1; }
    let (Some(v), Some(a)) = (cstr(verb), cstr(arg)) else { return -1 };
    match (*s).peer.send(&msg_of(&v, &a)) { Ok(()) => 0, Err(_) => -1 }
}

/// Wait up to `timeout_ms` for a message; writes the encoded form into `out`.
/// 1 = got one (out holds up to `cap` bytes, NUL-terminated when it fits),
/// 0 = nothing arrived, -1 = bad arguments.
#[no_mangle]
pub unsafe extern "C" fn flare_remote_poll(
    s: *const FlareRemote, timeout_ms: u32, out: *mut c_char, cap: usize,
) -> i32 {
    if s.is_null() || out.is_null() || cap == 0 { return -1; }
    match (*s).peer.recv_timeout(Duration::from_millis(timeout_ms as u64)) {
        Some(m) => {
            let e = encode(&m);
            let bytes = e.as_bytes();
            let n = bytes.len().min(cap - 1);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
            *out.add(n) = 0;
            1
        }
        None => 0,
    }
}

/// Drop the session. Safe with NULL; never called twice on the same handle.
#[no_mangle]
pub unsafe extern "C" fn flare_remote_close(s: *mut FlareRemote) {
    if !s.is_null() { drop(Box::from_raw(s)); }
}

fn wait_for_hello(sock: &UdpSocket, code: &str, budget: Duration) -> std::io::Result<SocketAddr> {
    let end = Instant::now() + budget;
    let mut buf = [0u8; 1500];
    while Instant::now() < end {
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            let m = decode(&String::from_utf8_lossy(&buf[..n]));
            if m.verb == Verb::Hello && m.arg == code { return Ok(from); }
        }
    }
    Err(std::io::ErrorKind::TimedOut.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(v: Verb, a: &str) -> Msg { Msg { verb: v, arg: a.into() } }
    #[test] fn proto() {
        let mut s = 1u64; let c = make_pairing_code(&mut s);
        assert_eq!(c.len(), 6); assert!(!c.contains(['0','1','I','L','O','U']));
        for x in [m(Verb::Play, ""), m(Verb::Frame, "12"), m(Verb::Command, "MI_Undo"), m(Verb::Cell, "3 7")] { assert_eq!(decode(&encode(&x)), x); }
        assert_eq!(decode("frame").verb, Verb::Invalid); assert_eq!(decode("play x").verb, Verb::Invalid); assert_eq!(decode("zz").verb, Verb::Invalid);
    }
    fn pair(c1: &'static str, c2: &'static str) -> (std::io::Result<Peer>, std::io::Result<Peer>) {
        let a = UdpSocket::bind("127.0.0.1:0").unwrap(); let b = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (aa, ba) = (a.local_addr().unwrap(), b.local_addr().unwrap());
        let t = std::thread::spawn(move || Peer::connect(b, aa, c2, Duration::from_secs(2)));
        (Peer::connect(a, ba, c1, Duration::from_secs(2)), t.join().unwrap())
    }
    #[test] fn loopback() {
        let (a, b) = pair("ABC234", "ABC234"); let (a, b) = (a.unwrap(), b.unwrap());
        a.send(&m(Verb::Frame, "12")).unwrap(); assert_eq!(b.recv(), Some(m(Verb::Frame, "12")));
        b.send(&m(Verb::Command, "MI_Undo")).unwrap(); assert_eq!(a.recv(), Some(m(Verb::Command, "MI_Undo")));
    }
    #[test] fn bad_code() { let (a, b) = pair("ABC234", "ZZZ999"); assert!(a.is_err() && b.is_err()); }
}

#[cfg(test)]
mod ffi_tests {
    use super::*;

    fn cs(s: &str) -> Vec<c_char> {
        s.as_bytes().iter().chain(std::iter::once(&0u8)).map(|b| *b as c_char).collect()
    }
    fn text(out: &[c_char]) -> String {
        unsafe { std::ffi::CStr::from_ptr(out.as_ptr()) }.to_str().unwrap().to_owned()
    }

    #[test]
    fn ffi_abi_shapes() {
        // Test that the C ABI function signatures are correct and safe.
        let play = cs("play");
        let empty = cs("");
        let mut out = [0i8; 64];

        // NULL handles must be inert and return errors, never panic.
        assert_eq!(unsafe { flare_remote_send(std::ptr::null_mut(), play.as_ptr(), empty.as_ptr()) }, -1);
        assert_eq!(unsafe { flare_remote_poll(std::ptr::null(), 0, out.as_mut_ptr(), out.len()) }, -1);
        assert_eq!(unsafe { flare_remote_port(std::ptr::null()) }, 0);
        unsafe { flare_remote_close(std::ptr::null_mut()) };

        // Invalid host/join args (NULL strings) must return NULL, not crash.
        let h = unsafe { flare_remote_host(0, std::ptr::null(), 100) };
        assert!(h.is_null());
        let g = unsafe { flare_remote_join(std::ptr::null(), 0, std::ptr::null(), 100) };
        assert!(g.is_null());
    }
}
