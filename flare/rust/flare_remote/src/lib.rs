//! Pure-std P2P remote transport (port of flare/sources/remote/remoteprotocol.h).
//! UDP hole-punch: both peers fire at each other's public addr until one
//! datagram lands; first frame must be "hello <code>". No libdatachannel.
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

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
    pub fn recv(&self) -> Option<Msg> {
        let mut buf = [0u8; 1500];
        for _ in 0..40 {
            if let Ok((n, f)) = self.sock.recv_from(&mut buf) {
                let m = decode(&String::from_utf8_lossy(&buf[..n]));
                if f == self.remote && !matches!(m.verb, Verb::Invalid | Verb::Hello) { return Some(m); }
            }
        }
        None
    }
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
