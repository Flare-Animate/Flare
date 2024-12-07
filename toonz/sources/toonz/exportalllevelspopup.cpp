

#include "exportalllevelspopup.h"

// Tnz6 includes
#include "tapp.h"
#include "filebrowser.h"
#include "columnselection.h"
#include "menubarcommandids.h"
#include "iocommand.h"
#include "exportlevelcommand.h"
#include "formatsettingspopups.h"

// TnzQt includes
#include "toonzqt/checkbox.h"
#include "toonzqt/doublefield.h"
#include "toonzqt/colorfield.h"
#include "toonzqt/tselectionhandle.h"
#include "toonzqt/gutil.h"
#include "toonzqt/planeviewer.h"
#include "toonzqt/framenavigator.h"
#include "toonzqt/imageutils.h"

// TnzLib includes
#include "toonz/txsheet.h"
#include "toonz/tcamera.h"
#include "toonz/txshsimplelevel.h"
#include "toonz/txshcell.h"
#include "toonz/tstageobjecttree.h"
#include "toonz/toonzscene.h"
#include "toonz/sceneproperties.h"
#include "toonz/tproject.h"
#include "toonz/stage.h"
#include "toonz/preferences.h"

#include "toonz/txshlevelhandle.h"
#include "toonz/txsheethandle.h"
#include "toonz/tscenehandle.h"
#include "toonz/txshleveltypes.h"
#include "toutputproperties.h"

// TnzCore includes
#include "tiio.h"
#include "tproperty.h"

// Qt includes
#include <QDir>
#include <QLabel>
#include <QPushButton>
#include <QComboBox>
#include <QGroupBox>
#include <QTabBar>
#include <QStackedWidget>
#include <QSplitter>
#include <QToolBar>
#include <QString>
#include <QMessageBox>


//********************************************************************************
//    Export callbacks  definition
//********************************************************************************

namespace {

struct MultiExportOverwriteCB final : public IoCmd::OverwriteCallbacks {
  bool m_yesToAll;
  bool m_stopped;

  MultiExportOverwriteCB() : m_yesToAll(false), m_stopped(false) {}
  bool overwriteRequest(const TFilePath &fp) override {
    if (m_yesToAll) return true;
    if (m_stopped) return false;

    int ret = DVGui::MsgBox(
        QObject::tr("Warning: file %1 already exists.").arg(toQString(fp)),
        QObject::tr("Continue Exporting"), QObject::tr("Continue to All"),
        QObject::tr("Stop Exporting"), 1);

    m_yesToAll = (ret == 2);
    m_stopped  = (ret == 0) || (ret == 3);
    return !m_stopped;
  }
};

//=============================================================================

struct MultiExportProgressCB final : public IoCmd::ProgressCallbacks {
  QString m_processedName;
  DVGui::ProgressDialog m_pb;

public:
  MultiExportProgressCB() : m_pb("", QObject::tr("Cancel"), 0, 0) {
    m_pb.show();
  }
  static QString msg() {
    return QObject::tr("Exporting level of %1 frames in %2");
  }

  void setProcessedName(const QString &name) override {
    m_processedName = name;
  }
  void setRange(int min, int max) override {
    m_pb.setMaximum(max);
    buildString();
  }
  void setValue(int val) override { m_pb.setValue(val); }
  bool canceled() const override { return m_pb.wasCanceled(); }
  void buildString() {
    m_pb.setLabelText(
        msg().arg(QString::number(m_pb.maximum())).arg(m_processedName));
  }
};

}  // namespace

//********************************************************************************
//    ExportAllLevelsPopup  implementation
//********************************************************************************

ExportAllLevelsPopup::ExportAllLevelsPopup(){
  setWindowModality(Qt::ApplicationModal);
  setWindowTitle(QString("Exporting All Levels..."));
  m_browser->setFolder(TApp::instance()->getCurrentScene()->getScene()->getProject()->getProjectFolder());

  // Export Options Tab
  m_exportOptions->m_createlevelfolder->setChecked(true);
  m_exportOptions->m_noAntialias->setChecked(true);

    // Export All checkbox
  m_exportall = new DVGui::CheckBox(tr("Export All"), this);
  m_exportall->setChecked(true);
  m_nameField->setDisabled(true);
  m_nameField->setText("");
  isexport_all = true;

  // Skip Button
  m_skipButton = new QPushButton(tr("Skip"), this);
  m_skipButton->setDisabled(true);

  // Layout
  QLayout *fileFormatLayout =
      layout()
          ->itemAt(2)
          ->layout()
          ->itemAt(2)
          ->widget()
          ->layout();  // Add below checkbox No Antialias
  fileFormatLayout->addWidget(m_exportall);
  fileFormatLayout->addWidget(m_skipButton);

  // Establish connections
  bool ret = true;

  ret = connect(m_skipButton, SIGNAL(clicked()), this, SLOT(skip())) && ret;
  ret = connect(m_exportall, SIGNAL(toggled(bool)), this,
                SLOT(onExportAll(bool))) &&
        ret;
  }

ExportAllLevelsPopup::~ExportAllLevelsPopup() { outputLevels.clear(); }

//------------------------------------

void ExportAllLevelsPopup::showEvent(QShowEvent *se) {
    
    if (Preferences::instance()->getPixelsOnly()) {
      m_exportOptions->m_widthFld->hide();
      m_exportOptions->m_heightFld->hide();
      m_exportOptions->m_widthLabel->hide();
      m_exportOptions->m_heightLabel->hide();
      m_exportOptions->m_dpiLabel->hide();
    } else {
      m_exportOptions->m_widthFld->show();
      m_exportOptions->m_heightFld->show();
      m_exportOptions->m_widthLabel->show();
      m_exportOptions->m_heightLabel->show();
      m_exportOptions->m_dpiLabel->show();
    }

    GetSelectedSimpLevels();
    
    if (outputLevels.empty()) {
        QTimer::singleShot(1000, this, &ExportAllLevelsPopup::hide);
        DVGui::error(tr("No Level in camera stand or null Level !"));
        return;
    }

    //sort
    std::sort(outputLevels.begin(), outputLevels.end(),
              [](TXshLevel *lhs, TXshLevel *rhs) {
                if (lhs->getType() ==
                    rhs->getType()) {  // vector first,biggest level name second
                  return lhs->getName() < rhs->getName();
                } else
                  return lhs->getType() > rhs->getType();
              });

    onExportAll(isexport_all);
    updateOnSelection();
}

void ExportAllLevelsPopup::updateOnSelection() {
    TXshSimpleLevel *sl = outputLevels.back()->getSimpleLevel();
    assert(sl);

    // Enable tlv output in case all inputs are pli or tlv
    int tlvIdx = m_format->findText("tlv");
        //export all mode
    if (isexport_all) {
      bool allPliTlvs = true;
      for (auto sl : outputLevels) {
        allPliTlvs = (sl && (sl->getType() == PLI_XSHLEVEL ||
                             sl->getType() == TZP_XSHLEVEL)) &&
                     allPliTlvs;
      }
      if (allPliTlvs) {
        if (tlvIdx < 0) m_format->addItem("tlv");
      } else {
        if (tlvIdx > 0) m_format->removeItem(tlvIdx);
      }
      
    } 
        //export one by one mode
    else {
      if (sl->getType() == PLI_XSHLEVEL || sl->getType() == TZP_XSHLEVEL) {
        if (tlvIdx < 0) m_format->addItem("tlv");
      } else {
        if (tlvIdx > 0) m_format->removeItem(tlvIdx);
      }
    }

    //whether be abel to set PliOptions
    if (sl->getType() == PLI_XSHLEVEL) {
        m_exportOptions->pliOtionsVisable = true;  // used when exportOptions be shown
        m_exportOptions->m_pliOptions->setEnabled(true); 
    } else {
        m_exportOptions->pliOtionsVisable = false;
        m_exportOptions->m_pliOptions->setEnabled(false); 
    }
    updatePreview();
    return;
}

void ExportAllLevelsPopup::GetSelectedSimpLevels() {
  // get output Levels
  TXsheet *xsh = TApp::instance()->getCurrentXsheet()->getXsheet();
  outputLevels.clear();
  int col_count = xsh->getColumnCount();
  int c, r0, r1;  // columnindex,cellstartpoint
  TXshSimpleLevel *sl;
  std::vector<int> cols;

  for (int i = 1; i <= col_count; ++i) {
    TXshColumn *col = xsh->getColumn(i - 1);
    assert(col);
    if (!col->isCamstandVisible()) continue;
    if (!col->getColumnType()) {  // eLevelType
      c = col->getIndex();
      if (col->getLevelRange(c, r0, r1)) {  // Not empty
        sl = xsh->getCell(r0, c).getSimpleLevel();
        assert(sl);
        outputLevels.push_back(sl);
      } else
        continue;
    }
  }
}


void ExportAllLevelsPopup::onExportAll(bool toggled) {
  if (toggled) {
    this->setWindowTitle(QString("Exporting All Levels"));
    m_skipButton->setDisabled(true);
    m_nameField->setDisabled(true);
    m_nameField->setText("");
    isexport_all = true;
  } 
  
  else {
    this->setWindowTitle(QString("Exporting ") +
                         getLevelTypeName(outputLevels.back()->getType()) +
                         QString(" Level ") +
                         QString::fromStdWString(outputLevels.back()->getName()));
    m_skipButton->setEnabled(true);
    m_nameField->setEnabled(true);
    isexport_all = false;
    m_nameField->setText(QString::fromStdWString(outputLevels.back()->getName()));
  }
}

bool ExportAllLevelsPopup::execute() {
  // Get Folder 
    TFilePath fp = TFilePath(m_browser->getFolder().getQString());

  // Do Path Checks
  if (fp.isEmpty()) {
    DVGui::error(tr("Please select a Folder!"));
    return false;
  }

  // ReBuild export options
  const std::string &ext                = m_format->currentText().toStdString();
  const IoCmd::ExportLevelOptions &opts = getOptions(ext);

  // Start to export
  bool ret = true;

  if (isexport_all) {
    TXshSimpleLevel *sl;
    MultiExportProgressCB progressCB;
    MultiExportOverwriteCB overwriteCB;
    while (!outputLevels.empty()) {
      sl  = outputLevels.back()->getSimpleLevel();
      ret = IoCmd::exportLevel(TFilePath(fp.getWideString()+to_wstring("\\") + sl->getName() + to_wstring("."+ext)), sl,
                               opts, &overwriteCB, &progressCB,m_exportOptions->m_createlevelfolder->isChecked()) &&
            ret;
      if (ret) {
        ++level_exported;  // count exported levels
      };
      outputLevels.pop_back();
    }
  }

  // Export one by one
  else {
    // File Name Check
    if (m_nameField->text().isEmpty()) {
      DVGui::error(tr("Please input the File name!"));
      return false;
    }
    if (!isValidFileName(QString::fromStdString(fp.getName()))) {
      DVGui::error(
          tr("The file name cannot be empty or contain any of the following "
             "characters:(new line)  \\ / : * ? \"  |"));
      return false;
    }
    if (isReservedFileName_message(QString::fromStdString(fp.getName())))
      return false;

    TXshSimpleLevel *sl = outputLevels.back()->getSimpleLevel();
    ret                 = IoCmd::exportLevel(
              fp.withType(ext).withName(m_nameField->text().toStdWString()), sl,
              opts, 0, 0, m_exportOptions->m_createlevelfolder->isChecked()) &&
          ret;

    // count levels exported
    if (ret) {
      ++level_exported;
      outputLevels.pop_back();
    } else
      return false;
  }

  // The Export Action Done
  if (outputLevels.empty()) {  // no more level to export
    QMessageBox::information(
        nullptr,
        "Export All Levels",                                   // title
        QString::number(level_exported) + " Levels Exported",  // content
        QMessageBox::Ok);
    level_exported = 0;
    return true;
  } else {  // move to next level
    m_nameField->setText(QString::fromStdWString(outputLevels.back()->getName()));
    this->setWindowTitle(QString("Exporting ") +
                         getLevelTypeName(outputLevels.back()->getType()) +
                         QString(" Level ") +
                         QString::fromStdWString(outputLevels.back()->getName()));
  }

  updateOnSelection();
  return false;
}

void ExportAllLevelsPopup::skip() {
  outputLevels.pop_back();
  if (outputLevels.empty()) {
      QMessageBox::information(nullptr,
                               tr("Exporting All Levels..."),  // title
                               QString::number(level_exported) + tr("%1 Levels Exported").arg(level_exported),  // content
                               QMessageBox::Ok);
    level_exported = 0;
    close();
    return;
  } else {
    this->setWindowTitle(QString("Exporting ") +
                         getLevelTypeName(outputLevels.back()->getType()) +
                         QString(" Level ") +
                         QString::fromStdWString(outputLevels.back()->getName()));
    m_nameField->setText(QString::fromStdWString(outputLevels.back()->getName()));
  }
  updateOnSelection();
}

QString ExportAllLevelsPopup::getLevelTypeName(int type) { 
    switch (type){
  case PLI_XSHLEVEL:
     return QString("ToonzVector");
  case TZP_XSHLEVEL:
     return QString("ToonzRaster");
  case OVL_XSHLEVEL:
     return QString("Raster");
  case LEVELCOLUMN_XSHLEVEL:
     return QString("Picture");
  default:
      return QString("");
    }
}
    
//********************************************************************************
//    Export All Levels Command  instantiation
//********************************************************************************

OpenPopupCommandHandler<ExportAllLevelsPopup> exportAllLevelsPopupCommand(MI_ExportAllLevels);