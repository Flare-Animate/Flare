#include "flashimportpopup.h"

#include "iocommand.h"
#include "menubarcommandids.h"
#include "tapp.h"
#include "texception.h"
#include "flare/tproject.h"
#include "flareqt/filefield.h"
#include "flareqt/menubarcommand.h"
#include "../flash/archive.h"
#include "../flash/document.h"
#include "../flarelib/flashimport.h"
#include "../flarelib/flashpreflight.h"

#include <QCheckBox>
#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QEventLoop>
#include <QFileInfo>
#include <QFormLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QMainWindow>
#include <QPlainTextEdit>
#include <QProgressDialog>
#include <QPushButton>

#include <algorithm>
#include <exception>
#include <utility>
#include <vector>

namespace {

void keepWrappedHeight(QLabel *label) {
  // Keep the label's height-for-width calculation, and let the findings area
  // absorb a smaller dialog rather than clipping multi-line document details.
  auto policy = label->sizePolicy();
  policy.setVerticalPolicy(QSizePolicy::Minimum);
  label->setSizePolicy(policy);
  label->setAlignment(Qt::AlignLeft | Qt::AlignTop);
}

// All document-supplied strings are shown as plain, bounded text. Reports keep
// the complete source IDs; the dialog avoids putting entire XML records in UI.
QString displayText(QString text, int maximum = 600) {
  if (text.size() > maximum) text = text.left(maximum) + QChar(0x2026);
  for (QChar &character : text)
    if (character.unicode() < 32 && character != QChar('\n') &&
        character != QChar('\t'))
      character = QChar(' ');
  return text;
}

QString exceptionText(const std::exception &error) {
  const char *message = error.what();
  int length          = 0;
  while (length < 4000 && message[length]) ++length;
  return displayText(QString::fromUtf8(message, length), 4000);
}

QString sourceStamp(const QString &source) {
  QFileInfo info(source);
  if (info.isDir()) info.setFile(QDir(source).filePath("DOMDocument.xml"));
  if (info.suffix().compare("xfl", Qt::CaseInsensitive) == 0)
    info.setFile(info.dir().filePath("DOMDocument.xml"));
  return info.canonicalFilePath() + '\n' + QString::number(info.size()) + '\n' +
         QString::number(info.lastModified().toMSecsSinceEpoch());
}

QString suggestedFolder(const QString &source) {
  const QFileInfo info(source);
  QString name = info.isDir() ? info.fileName() : info.completeBaseName();
  if (name.isEmpty()) name = "Flash";
  for (QChar &character : name)
    if (character.unicode() < 32 || QString("<>:\"/\\|?*").contains(character))
      character = '_';
  while (name.endsWith('.') || name.endsWith(' ')) name.chop(1);
  return name.left(100) + "_imported";
}

QString diagnosticText(const QJsonObject &report) {
  QStringList lines;
  const quint64 displaySlotLayers =
      quint64(report.value("displaySlotLayerCount").toDouble());
  if (displaySlotLayers) {
    QStringList partTracks;
    partTracks.append(
        QObject::tr("Part tracks: %1 multi-element source layer(s) use "
                    "display-slot columns (%2 keyframes, %3 placements).")
            .arg(QString::number(displaySlotLayers))
            .arg(QString::number(
                quint64(report.value("displaySlotKeyframeCount").toDouble())))
            .arg(QString::number(quint64(
                report.value("displaySlotPlacementCount").toDouble()))));
    partTracks.append(QObject::tr(
        "On an eligible import, simultaneous elements get separate native "
        "columns. A display slot may contain a different part at a later "
        "keyframe; "
        "the importer does not infer a part link."));
    const QJsonArray samples =
        report.value("displaySlotLayerSamples").toArray();
    for (int index = 0; index < std::min(samples.size(), 8); ++index) {
      const QJsonObject sample = samples[index].toObject();
      const QString name =
          displayText(sample.value("sourceLayerName").toString(), 100);
      const QString source =
          displayText(sample.value("sourceLayerId").toString(), 350);
      partTracks.append(
          QObject::tr("%1 keyframes, layer %2\n  %3")
              .arg(QString::number(
                  quint64(sample.value("keyframes").toDouble())))
              .arg(name.isEmpty() ? QObject::tr("(unnamed)") : name)
              .arg(source));
    }
    if (report.value("displaySlotLayerSamplesTruncated").toBool() ||
        samples.size() > 8)
      partTracks.append(QObject::tr(
          "Additional display-slot layers are omitted from this view."));
    lines.append(partTracks.join('\n'));
  }
  const QJsonObject codeCounts  = report.value("issueCodeCounts").toObject();
  const QJsonObject errorCounts = report.value("errorCodeCounts").toObject();
  if (!codeCounts.isEmpty()) {
    std::vector<std::pair<QString, quint64>> categories;
    categories.reserve(codeCounts.size());
    for (auto it = codeCounts.constBegin(); it != codeCounts.constEnd(); ++it)
      categories.emplace_back(it.key(), quint64(it.value().toDouble()));
    std::sort(categories.begin(), categories.end(),
              [](const auto &left, const auto &right) {
                return left.second != right.second ? left.second > right.second
                                                   : left.first < right.first;
              });
    QStringList summary;
    summary.append(QObject::tr("Findings by category (%1 total, %2 blocking):")
                       .arg(QString::number(
                           quint64(report.value("totalIssueCount").toDouble())))
                       .arg(QString::number(
                           quint64(report.value("errorCount").toDouble()))));
    constexpr size_t maxCategories = 30;
    for (size_t index = 0; index < std::min(categories.size(), maxCategories);
         ++index) {
      QString code = displayText(categories[index].first, 100);
      code.replace('\n', ' ').replace('\t', ' ');
      summary.append(
          QObject::tr("%1: %2 (blocking: %3)")
              .arg(code)
              .arg(QString::number(categories[index].second))
              .arg(QString::number(quint64(
                  errorCounts.value(categories[index].first).toDouble()))));
    }
    if (categories.size() > maxCategories)
      summary.append(
          QObject::tr("%1 more categories are omitted from this view.")
              .arg(QString::number(categories.size() - maxCategories)));
    lines.append(summary.join('\n'));
  }
  const QJsonArray issues = report.value("issues").toArray();
  const int count         = std::min(issues.size(), 500);
  for (int index = 0; index < count; ++index) {
    const QJsonObject issue = issues[index].toObject();
    const QString handling  = issue.value("handling").toString();
    QString line =
        displayText(
            handling.isEmpty() ? issue.value("severity").toString() : handling,
            30) +
        ": " + displayText(issue.value("message").toString(), 800);
    const QString source = displayText(issue.value("sourceId").toString(), 350);
    if (!source.isEmpty()) line += "\n  " + source;
    lines.append(line);
  }
  if (report.value("truncated").toBool() || issues.size() > count)
    lines.append(
        QObject::tr("Additional findings are omitted from this view."));
  if (lines.isEmpty())
    lines.append(
        QObject::tr("No unsupported features were found in the selected "
                    "import scope."));
  return lines.join("\n\n");
}

class ImportFlashCommand final : public MenuItemHandler {
public:
  ImportFlashCommand() : MenuItemHandler(MI_ImportFlash) {}
  void execute() override { openFlashImportPopup(); }
} importFlashCommand;

}  // namespace

FlashImportPopup::FlashImportPopup()
    : DVGui::Dialog(TApp::instance()->getMainWindow(), true, false,
                    "FlashImport") {
  setWindowTitle(tr("Import FLA/XFL as Editable Scene"));
  setWindowModality(Qt::ApplicationModal);
  resize(720, 620);

  auto explanation = new QLabel(
      tr("Inspect an FLA or uncompressed XFL document, then import editable "
         "Flare drawings and shared sub-xsheets into a new folder. "
         "Adobe Animate is not required."),
      this);
  explanation->setWordWrap(true);
  explanation->setTextFormat(Qt::PlainText);
  keepWrappedHeight(explanation);
  m_topLayout->addWidget(explanation);

  m_inputs  = new QWidget(this);
  auto form = new QFormLayout(m_inputs);
  form->setContentsMargins(0, 0, 0, 0);
  m_source = new DVGui::FileField(m_inputs, QString(), false, false, false);
  m_source->setObjectName("FlashImportSource");
  m_source->setFileMode(QFileDialog::ExistingFile);
  m_source->setFilters({"fla", "xfl", "xml"});
  m_source->setToolTip(
      tr("Select an FLA, XFL marker or DOMDocument.xml. "
         "An uncompressed XFL folder can also be entered."));
  form->addRow(tr("Source document:"), m_source);

  m_destination =
      new DVGui::FileField(m_inputs, QString(), false, false, false);
  m_destination->setObjectName("FlashImportParentFolder");
  m_destination->setFileMode(QFileDialog::Directory);
  form->addRow(tr("Save inside:"), m_destination);
  m_folderName = new QLineEdit(m_inputs);
  m_folderName->setObjectName("FlashImportNewFolder");
  m_folderName->setMaxLength(160);
  form->addRow(tr("New folder name:"), m_folderName);

  m_unusedSymbols =
      new QCheckBox(tr("Include unused library symbols"), m_inputs);
  m_unusedSymbols->setObjectName("FlashImportUnusedSymbols");
  m_unusedSymbols->setChecked(false);
  m_unusedSymbols->setToolTip(
      tr("Also convert library symbols that no timeline uses, as extra "
         "native levels."));
  form->addRow(QString(), m_unusedSymbols);
  m_symbolGuides =
      new QCheckBox(tr("Show guide layers inside symbols (notes)"), m_inputs);
  m_symbolGuides->setObjectName("FlashImportSymbolGuides");
  m_symbolGuides->setChecked(false);
  m_symbolGuides->setToolTip(
      tr("Animate shows a symbol's guide layers only while you edit that "
         "symbol. They are always imported as hidden [guide] columns; check "
         "this to show them wherever the symbol is placed."));
  form->addRow(QString(), m_symbolGuides);
  m_topLayout->addWidget(m_inputs);

  m_destinationStatus = new QLabel(this);
  m_destinationStatus->setObjectName("FlashImportDestinationStatus");
  m_destinationStatus->setWordWrap(true);
  m_destinationStatus->setTextFormat(Qt::PlainText);
  keepWrappedHeight(m_destinationStatus);
  m_topLayout->addWidget(m_destinationStatus);
  m_summary =
      new QLabel(tr("Choose a source document and click Inspect."), this);
  m_summary->setObjectName("FlashImportSummary");
  m_summary->setWordWrap(true);
  m_summary->setTextFormat(Qt::PlainText);
  keepWrappedHeight(m_summary);
  m_topLayout->addWidget(m_summary);

  m_diagnostics = new QPlainTextEdit(this);
  m_diagnostics->setObjectName("FlashImportFindings");
  m_diagnostics->setReadOnly(true);
  m_diagnostics->setMaximumBlockCount(2000);
  m_diagnostics->setPlaceholderText(
      tr("Compatibility findings and their source locations appear here. "
         "Features without a native mapping are approximated or left out, and "
         "every case is listed here and in the import report."));
  m_topLayout->addWidget(m_diagnostics, 1);
  auto limitations = new QLabel(
      tr("This importer is under development. Supported artwork becomes native "
         "Flare data; passing inspection does not guarantee a 1:1 visual "
         "match. The original document is preserved."),
      this);
  limitations->setWordWrap(true);
  limitations->setTextFormat(Qt::PlainText);
  keepWrappedHeight(limitations);
  m_topLayout->addWidget(limitations);

  m_inspect = new QPushButton(tr("Inspect"), this);
  m_import  = new QPushButton(tr("Import and Open"), this);
  m_inspect->setObjectName("FlashImportInspect");
  m_import->setObjectName("FlashImportApply");
  m_import->setDefault(false);
  m_close = new QPushButton(tr("Close"), this);
  m_close->setObjectName("FlashImportClose");
  addButtonBarWidget(m_inspect, m_import, m_close);

  connect(m_source->getField(), &QLineEdit::textChanged, this,
          [this] { sourceChanged(); });
  connect(m_destination->getField(), &QLineEdit::textChanged, this,
          [this] { updateActions(); });
  connect(m_folderName, &QLineEdit::textChanged, this,
          [this] { updateActions(); });
  connect(m_unusedSymbols, &QCheckBox::toggled, this,
          [this] { invalidateInspection(); });
  connect(m_inspect, &QPushButton::clicked, this, [this] { inspectSource(); });
  connect(m_import, &QPushButton::clicked, this, [this] { importSource(); });
  connect(m_close, &QPushButton::clicked, this, [this] { reject(); });
  updateActions();
}

QString FlashImportPopup::sourcePath() const {
  const QString path = m_source->getPath().trimmed();
  return path.isEmpty() ? QString() : QFileInfo(path).absoluteFilePath();
}

QString FlashImportPopup::destinationPath() const {
  const QString parent = m_destination->getPath().trimmed();
  if (parent.isEmpty() || m_folderName->text().isEmpty()) return QString();
  return QDir(QFileInfo(parent).absoluteFilePath())
      .filePath(m_folderName->text());
}

QString FlashImportPopup::destinationProblem() const {
  const QFileInfo parent(m_destination->getPath().trimmed());
  if (m_destination->getPath().trimmed().isEmpty() || !parent.isDir())
    return tr("Choose an existing folder in Save inside.");
  const QString name = m_folderName->text();
  if (name.isEmpty() || name == "." || name == ".." || name.endsWith('.') ||
      name != name.trimmed())
    return tr(
        "Enter a new folder name without leading or trailing spaces or "
        "a trailing period.");
  for (QChar character : name)
    if (character.unicode() < 32 || QString("<>:\"/\\|?*").contains(character))
      return tr("The new folder name contains an invalid character.");
  const QFileInfo target(destinationPath());
  if (target.exists() || target.isSymLink())
    return tr("The destination already exists. Choose a new folder name.");
  const QFileInfo source(sourcePath());
  if (source.isDir() ||
      source.suffix().compare("xfl", Qt::CaseInsensitive) == 0 ||
      source.fileName() == "DOMDocument.xml") {
    const QString root        = source.isDir() ? source.canonicalFilePath()
                                               : source.dir().canonicalPath();
    const QString destination = QDir(parent.canonicalFilePath()).filePath(name);
    const QString relative    = QDir(root).relativeFilePath(destination);
    if (!root.isEmpty() &&
        (relative == "." ||
         (!relative.startsWith("../") && !QDir::isAbsolutePath(relative))))
      return tr("Choose a destination outside the original XFL project.");
  }
  return QString();
}

void FlashImportPopup::sourceChanged() {
  if (m_busy) return;
  const QString path = sourcePath();
  if (!path.isEmpty() && QFileInfo::exists(path)) {
    const QFileInfo info(path);
    if (m_destination->getPath().isEmpty()) {
      QDir parent = info.dir();
      if (!info.isDir() &&
          (info.suffix().compare("xfl", Qt::CaseInsensitive) == 0 ||
           info.fileName() == "DOMDocument.xml"))
        parent.cdUp();
      m_destination->setPath(parent.absolutePath());
    }
    if (m_folderName->text().isEmpty())
      m_folderName->setText(suggestedFolder(path));
  }
  invalidateInspection();
}

void FlashImportPopup::invalidateInspection() {
  if (m_busy) return;
  m_canImport = false;
  m_inspectedSource.clear();
  m_inspectedStamp.clear();
  m_summary->setText(
      tr("Click Inspect to check this source and library scope."));
  m_diagnostics->clear();
  updateActions();
}

void FlashImportPopup::updateActions() {
  const QString problem = destinationProblem();
  m_destinationStatus->setText(
      problem.isEmpty() ? tr("New import folder: %1").arg(destinationPath())
                        : problem);
  m_inspect->setEnabled(!m_busy && QFileInfo::exists(sourcePath()));
  m_import->setEnabled(!m_busy && m_canImport && problem.isEmpty() &&
                       sourcePath() == m_inspectedSource &&
                       m_unusedSymbols->isChecked() == m_inspectedUnused);
  m_close->setEnabled(!m_busy);
}

void FlashImportPopup::reject() {
  if (m_busy) {
    m_cancelRequested = true;
    if (m_progress) m_progress->cancel();
    return;
  }
  DVGui::Dialog::reject();
}

void FlashImportPopup::withProgress(const QString &title,
                                    const std::function<void()> &operation) {
  m_busy            = true;
  m_cancelRequested = false;
  m_inputs->setEnabled(false);
  updateActions();
  QProgressDialog progress(title, tr("Cancel"), 0, 0, this);
  progress.setWindowTitle(windowTitle());
  progress.setWindowModality(Qt::ApplicationModal);
  progress.setMinimumDuration(0);
  progress.setAutoClose(false);
  progress.setAutoReset(false);
  m_progress = &progress;
  connect(&progress, &QProgressDialog::canceled, this,
          [this] { m_cancelRequested = true; });
  m_eventTimer.start();
  // cancelled() pumps events; keep autosave from saving the open scene
  // while an import is half done.
  const bool wasSaving = TApp::instance()->isSaveInProgress();
  TApp::instance()->setSaveInProgress(true);
  progress.show();
  QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
  const auto cleanup = [this, &progress, wasSaving] {
    TApp::instance()->setSaveInProgress(wasSaving);
    m_progress = nullptr;
    progress.hide();
    m_inputs->setEnabled(true);
    m_busy = false;
    updateActions();
  };
  try {
    operation();
  } catch (...) {
    cleanup();
    throw;
  }
  cleanup();
}

bool FlashImportPopup::cancelled() {
  // The application-modal progress window blocks ordinary editor input. Only
  // bounded event pumping is allowed; the busy guard rejects nested operations.
  if (m_eventTimer.elapsed() >= 35) {
    QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
    m_eventTimer.restart();
  }
  return m_cancelRequested || (m_progress && m_progress->wasCanceled());
}

void FlashImportPopup::showFailure(const QString &message) {
  m_canImport = false;
  m_summary->setText(
      m_cancelRequested
          ? tr("Operation canceled. The current scene is unchanged.")
          : tr("The document could not be imported."));
  m_diagnostics->setPlainText(displayText(message, 4000));
  updateActions();
}

void FlashImportPopup::inspectSource() {
  if (m_busy) return;
  m_canImport = false;
  m_diagnostics->clear();
  const QString source = sourcePath();
  const bool unused    = m_unusedSymbols->isChecked();
  try {
    Flash::DocumentInfo info;
    QJsonObject report;
    withProgress(tr("Inspecting Flash document"), [&] {
      Flash::Limits limits;
      limits.cancelled = [this] { return cancelled(); };
      Flash::Package package(source, limits);
      info = Flash::inspect(package);
      FlashNative::BuildOptions options;
      options.assetDirectory = destinationPath();
      if (options.assetDirectory.isEmpty())
        options.assetDirectory =
            QDir::temp().absoluteFilePath("Flare-Flash-Inspection");
      options.includeUnusedSymbols = unused;
      options.bestEffort           = true;
      // Video and MP3 findings depend on whether the import can decode them.
      FlashNative::attachMediaDecoders(options, info,
                                       FlashNative::ImportOptions());
      report = FlashNative::preflightScene(package, info, options);
    });
    m_inspectedSource = source;
    m_inspectedStamp  = sourceStamp(source);
    m_inspectedUnused = unused;
    m_canImport       = report.value("canImport").toBool(false);
    QString summary =
        tr("Creator: %1\nXFL version: %2\nStage: %3 x %4 pixels; %5 fps\n"
           "Library: %6 symbols\n%7")
            .arg(displayText(info.creator, 180).isEmpty()
                     ? tr("Not specified")
                     : displayText(info.creator, 180))
            .arg(displayText(info.version, 80))
            .arg(info.width)
            .arg(info.height)
            .arg(info.frameRate, 0, 'g', 8)
            .arg(info.symbols.size())
            .arg(!m_canImport ? tr("Import is blocked by the errors below.")
                 : report.value("approximatedCount").toDouble() +
                             report.value("omittedCount").toDouble() ==
                         0
                     ? tr("Everything in this scope maps to editable "
                          "native data.")
                     : tr("Ready to import. %1 detail(s) will be "
                          "approximated and %2 element(s) left out; "
                          "see the findings below.")
                           .arg(QString::number(quint64(
                               report.value("approximatedCount").toDouble())))
                           .arg(QString::number(quint64(
                               report.value("omittedCount").toDouble()))));
    const quint64 displaySlotLayers =
        quint64(report.value("displaySlotLayerCount").toDouble());
    if (displaySlotLayers)
      summary += tr("\nPart tracks: %1 multi-element layer(s) use display-slot "
                    "columns; see the findings below.")
                     .arg(QString::number(displaySlotLayers));
    m_summary->setText(summary);
    m_diagnostics->setPlainText(diagnosticText(report));
  } catch (const TException &error) {
    showFailure(QString::fromStdWString(error.getMessage()));
  } catch (const std::exception &error) {
    showFailure(exceptionText(error));
  } catch (...) {
    showFailure(tr("An unexpected error interrupted inspection."));
  }
  updateActions();
}

void FlashImportPopup::importSource() {
  if (m_busy || !m_import->isEnabled()) return;
  if (sourceStamp(sourcePath()) != m_inspectedStamp) {
    invalidateInspection();
    m_summary->setText(
        tr("The source changed. Inspect it again before importing."));
    return;
  }
  const QString source      = sourcePath();
  const QString destination = destinationPath();
  FlashNative::ImportResult result;
  try {
    withProgress(tr("Importing editable animation"), [&] {
      FlashNative::ImportOptions options;
      options.destinationDirectory = destination;
      options.project = TProjectManager::instance()->getCurrentProject();
      options.includeUnusedSymbols = m_unusedSymbols->isChecked();
      options.showSymbolGuides     = m_symbolGuides->isChecked();
      options.limits.cancelled     = [this] { return cancelled(); };
      QElapsedTimer clock;
      clock.start();
      options.progress = [this, &clock](const QString &stage, int current,
                                        int total) {
        if (!m_progress) return;
        // One overall bar across all import stages, with the time estimate.
        const double fraction =
            FlashNative::importProgressFraction(stage, current, total);
        const auto minutes = [](qint64 ms) {
          const qint64 seconds = ms / 1000;
          return QString("%1:%2")
              .arg(seconds / 60)
              .arg(seconds % 60, 2, 10, QChar('0'));
        };
        const qint64 elapsed = clock.elapsed();
        QString label        = displayText(stage, 180);
        if (total > 0) label += QString(" (%1 / %2)").arg(current).arg(total);
        label += "\n" + tr("%1% done, %2 elapsed")
                            .arg(int(fraction * 100))
                            .arg(minutes(elapsed));
        if (fraction > 0.05 && fraction < 1.0)
          label +=
              tr(", about %1 left")
                  .arg(minutes(qint64(elapsed * (1 - fraction) / fraction)));
        m_progress->setRange(0, 1000);
        m_progress->setValue(int(fraction * 1000));
        m_progress->setLabelText(label);
        cancelled();
      };
      result = FlashNative::importDocument(source, options);
    });
  } catch (const TException &error) {
    showFailure(QString::fromStdWString(error.getMessage()));
    return;
  } catch (const std::exception &error) {
    showFailure(exceptionText(error));
    return;
  } catch (...) {
    showFailure(tr("An unexpected error interrupted import."));
    return;
  }

  // A successful return means the complete bundle has been committed. A late
  // cancellation must not report that the saved result was discarded.
  m_canImport = false;
  // Hiding a QDialog ends its exec() loop even if it is shown again later.
  // Keep this dialog alive while the loader runs its nested progress/save
  // dialogs, and prevent reentrant imports or edits until opening returns.
  m_busy = true;
  m_inputs->setEnabled(false);
  updateActions();
  bool opened = false;
  QString openingError;
  try {
    opened = IoCmd::loadScene(result.scenePath, true, true, true);
  } catch (const TException &error) {
    openingError = QString::fromStdWString(error.getMessage());
  } catch (const std::exception &error) {
    openingError = exceptionText(error);
  } catch (...) {
    openingError =
        tr("An unexpected error interrupted opening the saved scene.");
  }
  m_inputs->setEnabled(true);
  m_busy = false;
  if (opened) {
    accept();
    return;
  }
  m_summary->setText(tr("Import was saved, but the scene was not opened.\n"
                        "Use File > Load Scene to open:\n%1")
                         .arg(result.scenePath.getQString()));
  m_diagnostics->setPlainText(tr("Import report: %1").arg(result.reportPath) +
                              (openingError.isEmpty()
                                   ? QString()
                                   : "\n\n" + displayText(openingError, 4000)));
  updateActions();
}

void openFlashImportPopup() {
  FlashImportPopup popup;
  popup.exec();
}
