// Flare serverless P2P remote control: protocol core (std-only, header-only).
// Pairing: host makes a 6-char code; peers exchange SDP offer/answer as
// copy-paste "signal blobs" (no signaling server; only public STUN is used
// by the WebRTC transport in remotetransport.h). Control messages are
// one-line "verb arg" text frames on the data channel.
#pragma once
#include <random>
#include <string>

namespace flare_remote {

static const char *const kStun  = "stun:stun.l.google.com:19302";
static const char *const kAlpha = "23456789ABCDEFGHJKMNPQRSTVWXYZ";  // 30, no 0/1/I/L/O/U

inline std::string makePairingCode(std::mt19937 &rng, int n = 6) {
  std::uniform_int_distribution<int> d(0, 29);
  std::string s;
  for (int i = 0; i < n; ++i) s += kAlpha[d(rng)];
  return s;
}

inline std::string b64enc(const std::string &in) {
  static const char *t =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  std::string o;
  unsigned v = 0;
  int b      = -6;
  for (unsigned char c : in) {
    v = (v << 8) | c, b += 8;
    while (b >= 0) o += t[(v >> b) & 63], b -= 6;
  }
  if (b > -6) o += t[((v << 8) >> (b + 8)) & 63];
  while (o.size() % 4) o += '=';
  return o;
}
inline std::string b64dec(const std::string &in) {
  std::string o;
  unsigned v = 0;
  int b      = -8;
  for (char c : in) {
    int x;
    if (c >= 'A' && c <= 'Z') x = c - 'A';
    else if (c >= 'a' && c <= 'z') x = c - 'a' + 26;
    else if (c >= '0' && c <= '9') x = c - '0' + 52;
    else if (c == '+') x = 62;
    else if (c == '/') x = 63;
    else continue;
    v = (v << 6) | x, b += 6;
    if (b >= 0) o += char((v >> b) & 0xFF), b -= 8;
  }
  return o;
}

// Signal blob = "FLR1:" + base64(code \n type \n sdp). User pastes it to peer.
inline std::string packSignal(const std::string &code, const std::string &type,
                              const std::string &sdp) {
  return "FLR1:" + b64enc(code + "\n" + type + "\n" + sdp);
}
inline bool unpackSignal(const std::string &blob, std::string &code,
                         std::string &type, std::string &sdp) {
  if (blob.compare(0, 5, "FLR1:")) return false;
  std::string r = b64dec(blob.substr(5));
  size_t a      = r.find('\n');
  if (a == std::string::npos) return false;
  size_t b = r.find('\n', a + 1);
  if (b == std::string::npos) return false;
  code = r.substr(0, a), type = r.substr(a + 1, b - a - 1),
  sdp = r.substr(b + 1);
  return type == "offer" || type == "answer";
}

enum class Verb { Invalid, Hello, Play, Stop, Frame, Command, Cell };
struct Msg {
  Verb verb = Verb::Invalid;
  std::string arg;  // Hello: code, Frame: n, Command: action id, Cell: "col row"
};
inline const char *verbName(Verb v) {
  switch (v) {
  case Verb::Hello: return "hello";
  case Verb::Play: return "play";
  case Verb::Stop: return "stop";
  case Verb::Frame: return "frame";
  case Verb::Command: return "cmd";
  case Verb::Cell: return "cell";
  default: return "";
  }
}
inline std::string encode(const Msg &m) {
  std::string s = verbName(m.verb);
  return m.arg.empty() ? s : s + " " + m.arg;
}
inline Msg decode(const std::string &s) {
  Msg m;
  size_t sp = s.find(' ');
  std::string v = s.substr(0, sp);
  if (sp != std::string::npos) m.arg = s.substr(sp + 1);
  if (m.arg.find('\n') != std::string::npos) return Msg();
  for (Verb x : {Verb::Hello, Verb::Play, Verb::Stop, Verb::Frame,
                 Verb::Command, Verb::Cell})
    if (v == verbName(x)) m.verb = x;
  bool needsArg = m.verb == Verb::Hello || m.verb == Verb::Frame ||
                  m.verb == Verb::Command || m.verb == Verb::Cell;
  if (needsArg && m.arg.empty()) return Msg();
  return m;
}

// Gate: nothing is dispatched until a Hello carrying the pairing code arrives.
struct Session {
  std::string code;
  bool paired = false;
  bool accept(const Msg &m) {
    if (!paired) {
      paired = m.verb == Verb::Hello && m.arg == code;
      return false;
    }
    return m.verb != Verb::Invalid && m.verb != Verb::Hello;
  }
};

}  // namespace flare_remote
