#include "flashguidedialog.h"

#include "menubarcommandids.h"
#include "flareqt/menubarcommand.h"

#include <QFile>
#include <QFont>
#include <QPushButton>
#include <QTabWidget>
#include <QTextBrowser>
#include <QVBoxLayout>

static QString loadDocFile(const QString &resourcePath,
                           const QString &fallback) {
  QFile f(resourcePath);
  if (f.open(QIODevice::ReadOnly | QIODevice::Text))
    return QString::fromUtf8(f.readAll());
  return fallback;
}

FlashGuideDialog::FlashGuideDialog(QWidget *parent)
    : DVGui::Dialog(parent, true) {
  setWindowTitle(tr("Flash Format Guide"));
  setMinimumSize(700, 500);

  QTabWidget *tabs = new QTabWidget(this);

  QFont mono("Monospace");
  mono.setStyleHint(QFont::TypeWriter);

  const QString importGuide = loadDocFile(
      ":/doc/how_to_import_swf.md",
      tr("Documentation not available.\n\n"
         "To import Flash files, use File > Import > Flash (FLA/XFL/SWF).\n"
         "Save-to-FLA is currently not supported in Flare; use File > Export > SWF or an Adobe Animate workflow for full round-tripping."));

  const QString techRef = loadDocFile(
      ":/doc/FLASH_SUPPORT.md",
      tr("Documentation not available.\n\n"
         "Flare includes built-in native support for Flash file formats."));

  auto makeTab = [&](const QString &content) {
    QTextBrowser *browser = new QTextBrowser(tabs);
    browser->setFont(mono);
    browser->setPlainText(content);
    return browser;
  };

  tabs->addTab(makeTab(importGuide), tr("Flash Import Guide"));
  tabs->addTab(makeTab(techRef), tr("Technical Reference"));

  addWidget(tabs);

  QPushButton *closeBtn = new QPushButton(tr("Close"), this);
  closeBtn->setDefault(true);
  addButtonBarWidget(closeBtn);
  connect(closeBtn, SIGNAL(clicked()), this, SLOT(accept()));
}

//-----------------------------------------------------------------------------

class FlashGuideCommand final : public MenuItemHandler {
public:
  FlashGuideCommand() : MenuItemHandler(MI_FlashGuide) {}
  void execute() override;
} g_flashGuideCommand;

void FlashGuideCommand::execute() {
  FlashGuideDialog *dlg = new FlashGuideDialog();
  dlg->setAttribute(Qt::WA_DeleteOnClose);
  dlg->show();
}

//-----------------------------------------------------------------------------

class FlashExportCommand final : public MenuItemHandler {
public:
  FlashExportCommand() : MenuItemHandler(MI_ExportFlash) {}
  void execute() override;
} g_flashExportCommand;

void FlashExportCommand::execute() {
  DVGui::MsgBox(
      DVGui::INFORMATION,
      QObject::tr(
          "<b>Native SWF / FLA export is not yet implemented.</b><br><br>"
          "To bring your Flare scene into Adobe Animate:<br>"
          "<ol>"
          "<li>Export as <b>OCA</b> (File &gt; Export &gt; Open Cel Animation) — "
          "this preserves layers, timing, and bitmap frames.</li>"
          "<li>Use the <i>OCA Importer</i> script in Adobe Animate (available "
          "from the Animate community exchange) to bring the OCA package in.</li>"
          "<li>From Animate you can then save as FLA or publish as SWF.</li>"
          "</ol>"
          "For a full technical reference, see <b>Help &gt; Flash Format Guide</b>."),
      std::vector<QString>{QObject::tr("OK")}, 0);
}
