// Std-only test of flare/sources/remote/remoteprotocol.h.
// clang++ -std=c++17 tests/native/remote_protocol_tests.cpp -o rp && ./rp
#include "../../flare/sources/remote/remoteprotocol.h"
#include <cstdio>
#include <set>
using namespace flare_remote;
static int fails = 0;
#define CHECK(x) \
  if (!(x)) std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #x), ++fails

int main() {
  std::mt19937 rng(1);
  std::set<std::string> seen;
  for (int i = 0; i < 1000; ++i) {
    std::string c = makePairingCode(rng);
    CHECK(c.size() == 6);
    CHECK(c.find_first_of("01ILOU") == std::string::npos);
    seen.insert(c);
  }
  CHECK(seen.size() > 990);

  for (std::string s : {"", "a", "ab", "abc", "v=0\r\na=x\n\xff\x00z"}) {
    CHECK(b64dec(b64enc(s)) == s);
  }
  CHECK(b64enc("Man") == "TWFu");
  CHECK(b64enc("M") == "TQ==");

  std::string c, t, sdp, raw = "v=0\r\no=- 1 2 IN IP4 0.0.0.0\r\n";
  CHECK(unpackSignal(packSignal("ABC234", "offer", raw), c, t, sdp));
  CHECK(c == "ABC234" && t == "offer" && sdp == raw);
  CHECK(!unpackSignal("junk", c, t, sdp));
  CHECK(!unpackSignal(packSignal("X", "bogus", raw), c, t, sdp));

  for (Msg m : {Msg{Verb::Play, ""}, Msg{Verb::Frame, "12"},
                Msg{Verb::Command, "MI_Undo"}, Msg{Verb::Cell, "3 7"}}) {
    Msg d = decode(encode(m));
    CHECK(d.verb == m.verb && d.arg == m.arg);
  }
  CHECK(decode("frame").verb == Verb::Invalid);
  CHECK(decode("rm -rf").verb == Verb::Invalid);

  Session s{"ABC234"};
  CHECK(!s.accept({Verb::Play, ""}));         // unpaired: dropped
  CHECK(!s.accept({Verb::Hello, "WRONG1"}));  // bad code
  CHECK(!s.paired);
  CHECK(!s.accept({Verb::Hello, "ABC234"}));  // pairs, not dispatched
  CHECK(s.paired);
  CHECK(s.accept({Verb::Play, ""}));
  CHECK(!s.accept({Verb::Invalid, ""}));

  std::printf(fails ? "remote_protocol_tests: %d FAIL\n"
                    : "remote_protocol_tests: OK\n", fails);
  return fails != 0;
}
