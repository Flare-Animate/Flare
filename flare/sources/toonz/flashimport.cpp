#include "toonz/menubarcommandids.h"
#include "toonz/menubar.h"
#include "toonz/ocaio.h"
#include "toonz/projectmanager.h"
#include "toonz/preferences.h"
#include "toonz/tapp.h"
#include "toonz/toonzfolders.h"
#include "tsystem.h"

#include "toonzqt/gutil.h"
#include "toonzqt/dvdialog.h"
#include "toonzqt/filebrowserpopup.h"
#include "toonzqt/gutil.h"

// Flash library includes
#include "flash/XFLReader.h"

#include <QProcess>
#include <QDesktopServices>
#include <QUrl>
#include <QFile>
#include <QDir>
#include <QDateTime>
#include <QMessageBox>

using namespace DVGui;

//-----------------------------------------------------------------------------
// Helper function to check if we can handle file internally
//-----------------------------------------------------------------------------

static bool canHandleInternally(const TFilePath &fp) {
  std::string ext = fp.getType();
  
  // XFL directories can be handled internally
  if (ext.empty() && XFL::isXFLDirectory(fp)) {
    return true;
  }
  
  // Modern FLA files that are ZIP-based could be extracted and read
  if (ext == "fla" && XFL::isFLAZipBased(fp)) {
    // For now, we still use external tools for ZIP extraction
    // Future: implement internal ZIP handling
    return false;
  }
  
  // XFL files need extraction first
  if (ext == "xfl") {
    return false;
  }
  
  return false;
}

//-----------------------------------------------------------------------------
// Import XFL directory internally
//-----------------------------------------------------------------------------

static bool importXFLDirectory(const TFilePath &xflPath) {
  XFL::Reader reader(xflPath);
  
  if (!reader.read()) {
    QString err = QObject::tr("Failed to read XFL: %1")
                    .arg(QString::fromStdString(reader.getError()));
    DVGui::warning(err);
    return false;
  }
  
  const XFL::Document &doc = reader.getDocument();
  
  // Show information about the imported XFL
  QString info = QObject::tr(
    "XFL Project Information:\n"
    "Size: %1 x %2\n"
    "Frame Rate: %3 fps\n"
    "Background: %4\n"
    "Symbols: %5\n\n"
    "XFL structure has been parsed. To import assets, please:\n"
    "1. Navigate to the LIBRARY folder\n"
    "2. Import individual images/SVG files\n"
    "3. Use File → Load Level for sequences")
    .arg(doc.width)
    .arg(doc.height)
    .arg(doc.frameRate)
    .arg(QString::fromStdString(doc.backgroundColor))
    .arg(doc.symbols.size());
  
  std::vector<QString> buttons = {QObject::tr("Open XFL Folder"), QObject::tr("OK")};
  int ret = DVGui::MsgBox(DVGui::INFORMATION, info, buttons);
  
  if (ret == 1) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(xflPath.getQString()));
  }
  
  return true;
}

class ImportFlashVectorCommand final : public MenuItemHandler {
public:
  ImportFlashVectorCommand() : MenuItemHandler(MI_ImportFlashVector) {}
  void execute() override;
} ImportFlashVectorCommand;

void ImportFlashVectorCommand::execute() {
  ToonzScene *scene = TApp::instance()->getCurrentScene()->getScene();
  static GenericLoadFilePopup *loadPopup = 0;
  if (!loadPopup) {
    loadPopup = new GenericLoadFilePopup(
        QObject::tr("Import Flash/XFL Content"));
    loadPopup->addFilterType("swf");
    loadPopup->addFilterType("fla");
    loadPopup->addFilterType("xfl");
  }
  if (!scene->isUntitled())
    loadPopup->setFolder(scene->getScenePath().getParentDir());
  else
    loadPopup->setFolder(
        TProjectManager::instance()->getCurrentProject()->getScenesPath());

  TFilePath fp = loadPopup->getPath();
  if (fp.isEmpty()) {
    DVGui::info(QObject::tr("Flash import cancelled: empty filepath."));
    return;
  }

  // Try internal handling first
  if (canHandleInternally(fp)) {
    importXFLDirectory(fp);
    return;
  }

  // Fall back to external decompiler for SWF and ZIP-based FLA
  QString msg = QObject::tr(
      "This file requires external decompiler support.\n\n"
      "For best results:\n"
      "1. Install JPEXS Free Flash Decompiler (ffdec)\n"
      "2. Set the path in Preferences → Import/Export\n"
      "3. JPEXS will extract vector content to SVG\n\n"
      "For XFL/FLA: Extract or export to XFL format first,\n"
      "then import the extracted XFL directory.\n\n"
      "Continue with external decompiler?");
  
  std::vector<QString> buttons = {QObject::tr("Continue"), QObject::tr("Cancel")};
  int ret = DVGui::MsgBox(DVGui::QUESTION, msg, buttons);
  
  if (ret == 2) {
    return;  // User cancelled
  }

  // Create temporary output directory
  QString tmpDirName = QString("flare_flash_import_%1")
                           .arg(QDateTime::currentMSecsSinceEpoch());
  TFilePath outDir = TSystem::getTempDir() + TFilePath(tmpDirName.toStdString());
  if (!outDir.mkpath(".")) {
    DVGui::error(QObject::tr("Unable to create temporary directory for import."));
    return;
  }

  // Try to invoke external decompiler directly
  QString decompilerPath = Preferences::instance()->getFlashDecompilerPath();
  
  if (decompilerPath.isEmpty()) {
    // Try common locations
    QStringList tryPaths = {"ffdec", "jpexs", 
                            "/usr/bin/ffdec", "/usr/local/bin/ffdec",
                            "C:\\Program Files\\FFDec\\ffdec.exe"};
    for (const QString &path : tryPaths) {
      QProcess testProc;
      testProc.start(path, QStringList() << "-help");
      if (testProc.waitForStarted(1000)) {
        testProc.kill();
        testProc.waitForFinished();
        decompilerPath = path;
        break;
      }
    }
  }

  if (decompilerPath.isEmpty()) {
    QString err = QObject::tr(
        "Flash decompiler not found.\n\n"
        "Please install JPEXS Free Flash Decompiler:\n"
        "https://github.com/jindrapetrik/jpexs-decompiler\n\n"
        "Then set the path in:\n"
        "File → Preferences → Import/Export → Flash Decompiler Path");
    DVGui::warning(err);
    return;
  }

  // Run decompiler to export FLA/XFL
  QStringList args;
  args << "-export" << "fla" << outDir.getQString() << fp.getQString();
  
  QProcess proc;
  proc.setProgram(decompilerPath);
  proc.setArguments(args);
  proc.start();
  
  bool started = proc.waitForStarted(10000);
  if (!started) {
    QString err = QObject::tr("Unable to start Flash decompiler: %1").arg(decompilerPath);
    DVGui::warning(err);
    return;
  }

  // Wait for completion
  proc.waitForFinished(-1);
  int exitCode = proc.exitCode();

  if (exitCode != 0) {
    QString stdErr = proc.readAllStandardError();
    QString err = QObject::tr("Flash decompilation failed:\n%1").arg(stdErr);
    DVGui::warning(err);
    return;
  }

  QString successMsg = QObject::tr(
      "Flash content has been extracted to:\n%1\n\n"
      "You can now import the extracted assets:\n"
      "- SVG files for vector content\n"
      "- Image sequences for animations\n"
      "- XFL structure if exported\n\n"
      "Open containing folder?")
      .arg(outDir.getQString());
  
  std::vector<QString> resultButtons = {QObject::tr("Open Folder"), QObject::tr("OK")};
  int result = DVGui::MsgBox(DVGui::INFORMATION, successMsg, resultButtons);

  if (result == 1) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(outDir.getQString()));
  }
}
