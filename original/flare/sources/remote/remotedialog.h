// Minimal Qt UI for P2P remote: Host / Join, a paste box for signal blobs,
// and play/stop/frame buttons. dispatch is supplied by the app (e.g. map Play
// to the "MI_Play" action, Frame to TApp frame handle). Needs
// FLARE_HAVE_LIBDATACHANNEL; not yet added to the flare target's sources.
#pragma once
#include "remotetransport.h"
#ifdef FLARE_HAVE_LIBDATACHANNEL
#include <QDialog>
#include <QHBoxLayout>
#include <QLabel>
#include <QMetaObject>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSpinBox>
#include <QVBoxLayout>
#include <functional>
#include <memory>

class FlareRemoteDialog : public QDialog {
public:
  std::function<void(const flare_remote::Msg &)> dispatch;

  explicit FlareRemoteDialog(QWidget *parent = nullptr) : QDialog(parent) {
    using namespace flare_remote;
    setWindowTitle(tr("Remote Control (P2P)"));
    auto *v = new QVBoxLayout(this);
    auto *code = new QLabel(tr("Not linked"));
    auto *box  = new QPlainTextEdit;
    box->setPlaceholderText(tr("Paste the other device's FLR1: blob here"));
    auto *host = new QPushButton(tr("Host")), *join = new QPushButton(tr("Join")),
         *fin = new QPushButton(tr("Finish (host)"));
    auto *row = new QHBoxLayout;
    row->addWidget(host), row->addWidget(join), row->addWidget(fin);
    auto *play = new QPushButton(tr("Play")), *stop = new QPushButton(tr("Stop"));
    auto *frame = new QSpinBox;
    frame->setRange(1, 99999);
    auto *ctl = new QHBoxLayout;
    ctl->addWidget(play), ctl->addWidget(stop), ctl->addWidget(frame);
    v->addWidget(code), v->addLayout(row), v->addWidget(box), v->addLayout(ctl);

    auto make = [this, code](std::string c) {
      m_t = std::make_unique<Transport>(c);
      m_t->onMessage = [this](const Msg &m) {
        QMetaObject::invokeMethod(this, [this, m] { if (dispatch) dispatch(m); });
      };
      m_t->onOpen = [this, code] {
        QMetaObject::invokeMethod(this, [code] { code->setText(tr("Linked")); });
      };
    };
    connect(host, &QPushButton::clicked, [=] {
      std::mt19937 rng(std::random_device{}());
      std::string c = makePairingCode(rng);
      make(c);
      code->setText(tr("Code: %1 - send the blob below").arg(c.c_str()));
      box->setPlainText(QString::fromStdString(m_t->createOffer()));
    });
    connect(join, &QPushButton::clicked, [=] {
      make("");
      box->setPlainText(QString::fromStdString(
          m_t->acceptOffer(box->toPlainText().trimmed().toStdString())));
      code->setText(tr("Send the answer blob back to the host"));
    });
    connect(fin, &QPushButton::clicked, [=] {
      if (m_t && !m_t->acceptAnswer(box->toPlainText().trimmed().toStdString()))
        code->setText(tr("Bad answer blob"));
    });
    auto send = [this](Verb v, std::string a = {}) {
      if (m_t) m_t->send({v, a});
    };
    connect(play, &QPushButton::clicked, [=] { send(Verb::Play); });
    connect(stop, &QPushButton::clicked, [=] { send(Verb::Stop); });
    connect(frame, QOverload<int>::of(&QSpinBox::valueChanged),
            [=](int f) { send(Verb::Frame, std::to_string(f)); });
  }

private:
  std::unique_ptr<flare_remote::Transport> m_t;
};
#endif
