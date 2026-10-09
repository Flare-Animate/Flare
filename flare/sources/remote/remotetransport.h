// WebRTC data-channel transport over libdatachannel (MPL-2.0), optional.
// Define FLARE_HAVE_LIBDATACHANNEL and link LibDataChannel::LibDataChannel.
// Host: createOffer() -> blob to peer. Peer: acceptOffer(blob) -> answer blob
// back. Host: acceptAnswer(blob). ICE is gathered fully before a blob is
// produced (non-trickle) so one paste each way is enough. NAT traversal via
// public STUN only; no signaling/relay server.
#pragma once
#include "remoteprotocol.h"
#ifdef FLARE_HAVE_LIBDATACHANNEL
#include <rtc/rtc.hpp>
#include <functional>
#include <future>
#include <memory>

namespace flare_remote {

class Transport {
public:
  std::function<void(const Msg &)> onMessage;  // called on libdatachannel thread
  std::function<void()> onOpen;
  Session session;

  explicit Transport(std::string code) {
    session.code = std::move(code);
    rtc::Configuration cfg;
    cfg.iceServers.emplace_back(kStun);
    m_pc = std::make_shared<rtc::PeerConnection>(cfg);
    m_pc->onGatheringStateChange([this](rtc::PeerConnection::GatheringState s) {
      if (s != rtc::PeerConnection::GatheringState::Complete) return;
      if (auto d = m_pc->localDescription())
        m_local.set_value(packSignal(session.code, d->typeString(), *d));
    });
    m_pc->onDataChannel(
        [this](std::shared_ptr<rtc::DataChannel> dc) { bind(dc); });
  }
  std::string createOffer() {
    bind(m_pc->createDataChannel("flare"));
    return m_local.get_future().get();
  }
  std::string acceptOffer(const std::string &blob) {
    std::string c, t, sdp;
    if (!unpackSignal(blob, c, t, sdp) || t != "offer") return {};
    session.code = c, session.paired = true;  // controller trusts the host
    m_pc->setRemoteDescription(rtc::Description(sdp, t));
    return m_local.get_future().get();
  }
  bool acceptAnswer(const std::string &blob) {
    std::string c, t, sdp;
    if (!unpackSignal(blob, c, t, sdp) || t != "answer" || c != session.code)
      return false;
    m_pc->setRemoteDescription(rtc::Description(sdp, t));
    return true;
  }
  void send(const Msg &m) {
    if (m_dc && m_dc->isOpen()) m_dc->send(encode(m));
  }

private:
  void bind(std::shared_ptr<rtc::DataChannel> dc) {
    m_dc = dc;
    dc->onOpen([this] {
      send({Verb::Hello, session.code});
      if (onOpen) onOpen();
    });
    dc->onMessage([this](rtc::message_variant d) {
      if (auto *s = std::get_if<std::string>(&d)) {
        Msg m = decode(*s);
        if (session.accept(m) && onMessage) onMessage(m);
      }
    });
  }
  std::shared_ptr<rtc::PeerConnection> m_pc;
  std::shared_ptr<rtc::DataChannel> m_dc;
  std::promise<std::string> m_local;
};

}  // namespace flare_remote
#endif
