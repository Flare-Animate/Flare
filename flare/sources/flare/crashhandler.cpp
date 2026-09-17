#include "crashhandler.h"

#include <inttypes.h>
#include <signal.h>

#ifdef _WIN32
#include <windows.h>
#include <winbase.h>
#include <dbghelp.h>
#include <psapi.h>
#else
#include <execinfo.h>
#include <signal.h>
#include <unistd.h>
#include <err.h>
#include <errno.h>
#include <sys/wait.h>
#include <regex>
#ifdef MACOSX
#include <mach-o/dyld.h>  // _NSGetExecutablePath(); /proc does not exist here
#include <cstdint>
#endif
#endif

#include "tgl.h"
#include "tapp.h"
#include "tenv.h"
#include "tconvert.h"
#include "texception.h"
#include "tfilepath_io.h"
#include "flare/toonzfolders.h"
#include "flare/tproject.h"
#include "flare/tscenehandle.h"
#include "flare/toonzscene.h"

#include <QOperatingSystemVersion>
#include <QDesktopServices>
#include <QApplication>
#include <QClipboard>
#include <QThread>
#include <QMainWindow>
#include <QMessageBox>
#include <QCloseEvent>
#include <QDialog>
#include <QLayout>
#include <QLabel>
#include <QTextEdit>
#include <QPushButton>

static QWidget *s_parentWindow = NULL;
static bool s_reportProjInfo   = false;

//-----------------------------------------------------------------------------

static const char *filenameOnly(const char *path) {
  for (int i = strlen(path); i >= 0; --i) {
    if (path[i] == '\\' || path[i] == '/') return path + i + 1;
  }
  return path;
}

//-----------------------------------------------------------------------------
//-----------------------------------------------------------------------------
// Windows platform functions

#ifdef _WIN32

#define HAS_MINIDUMP
static bool generateMinidump(TFilePath dumpFile) {
  HANDLE hDumpFile = CreateFileW(dumpFile.getWideString().c_str(),
                                 GENERIC_WRITE, 0, 0, CREATE_ALWAYS, 0, 0);
  if (hDumpFile == INVALID_HANDLE_VALUE) return false;

  MINIDUMP_EXCEPTION_INFORMATION mdei;
  mdei.ThreadId          = GetCurrentThreadId();
  mdei.ExceptionPointers = NULL;
  mdei.ClientPointers    = FALSE;

  if (MiniDumpWriteDump(GetCurrentProcess(), GetCurrentProcessId(), hDumpFile,
                        MiniDumpNormal, &mdei, 0, NULL)) {
    CloseHandle(hDumpFile);
    return true;
  }

  return false;
}

#define HAS_MODULES
static void printModules(std::string &out) {
  HANDLE hProcess = GetCurrentProcess();

  HMODULE modules[1024];
  DWORD size;
  if (EnumProcessModules(hProcess, modules, sizeof(modules), &size)) {
    for (unsigned int i = 0; i < size / sizeof(HMODULE); i++) {
      char moduleName[512];
      GetModuleFileNameA(modules[i], moduleName, 512);
      out.append(moduleName);
      out.append("\n");
    }
  }
}

#define HAS_BACKTRACE
static void printBacktrace(std::string &out) {
  int frameStack = 0;
  int frameSkip  = 3;

  HANDLE hProcess = GetCurrentProcess();

  SymSetOptions(SYMOPT_DEFERRED_LOADS | SYMOPT_INCLUDE_32BIT_MODULES |
                SYMOPT_FAIL_CRITICAL_ERRORS | SYMOPT_LOAD_LINES |
                SYMOPT_UNDNAME);

  SymInitialize(hProcess, NULL, TRUE);

  CONTEXT context;
  RtlCaptureContext(&context);

  char sourceSymMem[sizeof(IMAGEHLP_SYMBOL64) + 1025];
  PIMAGEHLP_SYMBOL64 sourceSym = (PIMAGEHLP_SYMBOL64)&sourceSymMem;
  memset(sourceSymMem, 0, sizeof(sourceSymMem));

  IMAGEHLP_LINE64 sourceInfo;
  memset(&sourceInfo, 0, sizeof(IMAGEHLP_LINE64));
  sourceInfo.SizeOfStruct = sizeof(IMAGEHLP_LINE64);

  IMAGEHLP_MODULE64 moduleInfo;
  memset(&moduleInfo, 0, sizeof(moduleInfo));
  moduleInfo.SizeOfStruct = sizeof(IMAGEHLP_MODULE64);

  STACKFRAME64 stackframe;
  memset(&stackframe, 0, sizeof(STACKFRAME64));

#ifdef _WIN64
  int machineType             = IMAGE_FILE_MACHINE_AMD64;
  stackframe.AddrPC.Offset    = context.Rip;
  stackframe.AddrPC.Mode      = AddrModeFlat;
  stackframe.AddrStack.Offset = context.Rsp;
  stackframe.AddrStack.Mode   = AddrModeFlat;
  stackframe.AddrFrame.Offset = context.Rbp;
  stackframe.AddrFrame.Mode   = AddrModeFlat;
#else
  int machineType             = IMAGE_FILE_MACHINE_I386;
  stackframe.AddrPC.Offset    = context.Eip;
  stackframe.AddrPC.Mode      = AddrModeFlat;
  stackframe.AddrStack.Offset = context.Esp;
  stackframe.AddrStack.Mode   = AddrModeFlat;
  stackframe.AddrFrame.Offset = context.Ebp;
  stackframe.AddrFrame.Mode   = AddrModeFlat;
#endif

  HANDLE hThread = GetCurrentThread();
  while (StackWalk64(machineType, hProcess, hThread, &stackframe, &context,
                     NULL, SymFunctionTableAccess64, SymGetModuleBase64,
                     NULL)) {
    // Skip first frames since they point to this function
    if (frameStack++ < frameSkip) continue;
    char numStr[32];
    memset(numStr, 0, sizeof(numStr));
    sprintf(numStr, "%3i> ", frameStack - frameSkip);
    out.append(numStr);

    sourceSym->SizeOfStruct  = sizeof(IMAGEHLP_SYMBOL64);
    sourceSym->MaxNameLength = 1024;

    // Get symbol name
    DWORD64 displacement64;
    if (SymGetSymFromAddr64(hProcess, (ULONG64)stackframe.AddrPC.Offset,
                            &displacement64, sourceSym)) {
      out.append(sourceSym->Name);

      // Get module filename
      char moduleFile[512];
      MEMORY_BASIC_INFORMATION mbi;
      VirtualQuery((LPCVOID)stackframe.AddrPC.Offset, &mbi, sizeof(mbi));
      GetModuleFileNameA((HMODULE)mbi.AllocationBase, moduleFile, 512);

      // Get source filename and line
      DWORD displacement32;
      if (SymGetLineFromAddr64(hProcess, stackframe.AddrPC.Offset,
                               &displacement32, &sourceInfo) != FALSE) {
        out.append(" {");
        out.append(filenameOnly(sourceInfo.FileName));
        out.append(":");
        out.append(std::to_string(sourceInfo.LineNumber));
        out.append("}");
      } else {
        memset(numStr, 0, sizeof(numStr));
        sprintf(numStr, " [0x%" PRIx64 "]", stackframe.AddrPC.Offset);
        out.append(numStr);
      }
      out.append(" <");
      out.append(filenameOnly(moduleFile));
      out.append(">");
    }

    out.append("\n");
  }

  SymCleanup(hProcess);
}

//-----------------------------------------------------------------------------

LONG WINAPI exceptionHandler(PEXCEPTION_POINTERS info) {
  static volatile bool handling = false;

  const char *reason = "Unknown";
  switch (info->ExceptionRecord->ExceptionCode) {
  case EXCEPTION_ACCESS_VIOLATION:
    reason = "EXCEPTION_ACCESS_VIOLATION";
    break;
  case EXCEPTION_ARRAY_BOUNDS_EXCEEDED:
    reason = "EXCEPTION_ARRAY_BOUNDS_EXCEEDED";
    break;
  case EXCEPTION_DATATYPE_MISALIGNMENT:
    reason = "EXCEPTION_DATATYPE_MISALIGNMENT";
    break;
  case EXCEPTION_FLT_DENORMAL_OPERAND:
    reason = "EXCEPTION_FLT_DENORMAL_OPERAND";
    break;
  case EXCEPTION_FLT_DIVIDE_BY_ZERO:
    reason = "EXCEPTION_FLT_DIVIDE_BY_ZERO";
    break;
  case EXCEPTION_FLT_INEXACT_RESULT:
    reason = "EXCEPTION_FLT_INEXACT_RESULT";
    break;
  case EXCEPTION_FLT_INVALID_OPERATION:
    reason = "EXCEPTION_FLT_INVALID_OPERATION";
    break;
  case EXCEPTION_FLT_OVERFLOW:
    reason = "EXCEPTION_FLT_OVERFLOW";
    break;
  case EXCEPTION_FLT_STACK_CHECK:
    reason = "EXCEPTION_FLT_STACK_CHECK";
    break;
  case EXCEPTION_FLT_UNDERFLOW:
    reason = "EXCEPTION_FLT_UNDERFLOW";
    break;
  case EXCEPTION_ILLEGAL_INSTRUCTION:
    reason = "EXCEPTION_ILLEGAL_INSTRUCTION";
    break;
  case EXCEPTION_IN_PAGE_ERROR:
    reason = "EXCEPTION_IN_PAGE_ERROR";
    break;
  case EXCEPTION_INT_DIVIDE_BY_ZERO:
    reason = "EXCEPTION_INT_DIVIDE_BY_ZERO";
    break;
  case EXCEPTION_INVALID_DISPOSITION:
    reason = "EXCEPTION_INVALID_DISPOSITION";
    break;
  case EXCEPTION_NONCONTINUABLE_EXCEPTION:
    reason = "EXCEPTION_NONCONTINUABLE_EXCEPTION";
    break;
  case EXCEPTION_PRIV_INSTRUCTION:
    reason = "EXCEPTION_PRIV_INSTRUCTION";
    break;
  case EXCEPTION_STACK_OVERFLOW:
    reason = "EXCEPTION_STACK_OVERFLOW";
    break;
  default:
    return EXCEPTION_CONTINUE_SEARCH;
  }

  // Avoid new exceptions inside the crash handler
  if (handling) return EXCEPTION_CONTINUE_SEARCH;

  handling = true;
  if (CrashHandler::trigger(reason, true)) _Exit(1);
  handling = false;

  return EXCEPTION_CONTINUE_SEARCH;
}

#endif

//-----------------------------------------------------------------------------
//-----------------------------------------------------------------------------
// Linux and Mac OS X platform functions

#ifndef _WIN32

// Run argv[0] with the given arguments and collect its output.
//
// Deliberately fork+exec rather than popen(): popen() runs the command through
// /bin/sh, and the executable path is interpolated into that command line. A
// path is not under our control — a user can keep Flare.app in any directory
// they like — so a directory named with a backtick or $( ) was enough to get
// arbitrary code executed at crash time. Passing an argv array means the shell
// never parses any of this. It also keeps /bin/sh out of an already-fragile
// crash path.
static bool runTool(std::string &out, const char *const argv[]) {
  int fds[2];
  if (pipe(fds) != 0) return false;

  pid_t pid = fork();
  if (pid < 0) {
    close(fds[0]);
    close(fds[1]);
    return false;
  }
  if (pid == 0) {
    // Child: stdout and stderr -> pipe, then exec. No shell involved.
    close(fds[0]);
    dup2(fds[1], STDOUT_FILENO);
    dup2(fds[1], STDERR_FILENO);
    close(fds[1]);
    execvp(argv[0], const_cast<char *const *>(argv));
    _exit(127);  // exec failed; never return into the parent's handler
  }

  close(fds[1]);
  char buffer[128];
  ssize_t n;
  while ((n = read(fds[0], buffer, sizeof(buffer))) != 0) {
    if (n < 0) {
      if (errno == EINTR) continue;  // a crash handler runs amid signals
      break;
    }
    out.append(buffer, n);
  }
  close(fds[0]);

  int status = 0;
  while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {
  }
  return WIFEXITED(status) && WEXITSTATUS(status) == 0;
}

//-----------------------------------------------------------------------------

static bool addr2line(std::string &out, const char *exepath, const char *addr) {
#ifdef MACOSX
  // -l is not optional. backtrace_symbols() reports the runtime PC, which
  // includes the ASLR slide, while atos interprets a bare address relative to
  // the image's *preferred* load address. Without telling it where the image
  // actually landed, every lookup comes back "??" — a well-formed command that
  // still symbolicates nothing. _dyld_get_image_header(0) is the main
  // executable's real load address.
  char loadAddr[32];
  snprintf(loadAddr, sizeof(loadAddr), "%p",
           (const void *)_dyld_get_image_header(0));
  const char *const argv[] = {"atos", "-o", exepath, "-l", loadAddr, addr,
                              nullptr};
#else
  const char *const argv[] = {"addr2line", "-f", "-p", "-e", exepath, addr,
                              nullptr};
#endif
  return runTool(out, argv);
}

//-----------------------------------------------------------------------------

static bool generateMinidump(TFilePath dumpFile) { return false; }

//-----------------------------------------------------------------------------

static void printModules(std::string &out) {}

//-----------------------------------------------------------------------------

#define HAS_BACKTRACE
static void printBacktrace(std::string &out) {
  int frameStack = 0;
  int frameSkip  = 3;

  const int size = 256;
  void *buffer[size];

  // Get executable path. /proc is Linux-only: on macOS the readlink() always
  // failed, exepath stayed empty, and addr2line() below then ran
  // `atos -o "" ...` against no binary at all.
  // 4096, matching tenv.cpp: _NSGetExecutablePath() fails outright when the
  // buffer is too small, so a 512-byte one would have made Flare installed
  // under a long path look unresolvable and silently dropped every frame back
  // to a raw symbol. macOS PATH_MAX is 1024, so this cannot now be too small.
  char exepath[4096];
  memset(exepath, 0, sizeof(exepath));
#ifdef MACOSX
  uint32_t exepathSize = sizeof(exepath);
  if (_NSGetExecutablePath(exepath, &exepathSize) != 0) {
    exepath[0] = '\0';
    fprintf(stderr, "Couldn't get exe path\n");
  }
#else
  // readlink() does not NUL-terminate, so leave room and terminate by hand.
  ssize_t exepathLen = readlink("/proc/self/exe", exepath, sizeof(exepath) - 1);
  if (exepathLen < 0) {
    exepath[0] = '\0';
    fprintf(stderr, "Couldn't get exe path\n");
  } else {
    exepath[exepathLen] = '\0';
  }
#endif

  // Back trace
  int nptrs  = backtrace(buffer, size);
  char **bts = backtrace_symbols(buffer, nptrs);
  // Match ONLY a hex address. backtrace_symbols() formats differ:
  //   Linux: /opt/flare/bin/Flare(+0x1a2b3c) [0x55d1a2b3c4d5]
  //   macOS: 3   Flare   0x000000010a2b3c4d -[NSApplication run] + 72
  // The old pattern was "\[(.+)\]", which on macOS matched the *Objective-C
  // selector* inside the brackets rather than an address — so a frame like
  // -[NSApplication(NSEventRouting) _nextEventMatchingEventMask:...] was
  // pasted unquoted into the atos command line and the shell died on the
  // parenthesis. Capturing a hex address means nothing else can reach sh.
#ifdef MACOSX
  std::regex re("(0x[0-9a-fA-F]+)");
#else
  std::regex re("\\[(0x[0-9a-fA-F]+)\\]");
#endif
  if (bts) {
    for (int i = 0; i < nptrs; ++i) {
      // Skip first frames since they point to this function
      if (frameStack++ < frameSkip) continue;
      char numStr[32];
      memset(numStr, 0, sizeof(numStr));
      sprintf(numStr, "%3i> ", frameStack - frameSkip);
      out.append(numStr);

      std::string sym = bts[i];
      std::string line;
      std::smatch ms;

      bool found = false;
      // No executable path means addr2line/atos has nothing to resolve
      // against; fall straight through to the raw symbol.
      if (exepath[0] && std::regex_search(sym, ms, re)) {
        std::string addr = ms[1];
        if (addr2line(line, exepath, addr.c_str())) {
          found = (line.rfind("??", 0) != 0);
        }
      }

      out.append(found ? line : (sym + "\n"));
    }
  }

  free(bts);
}

void signalHandler(int sig) {
  static volatile bool handling = false;

  const char *reason = "Unknown";
  switch (sig) {
  case SIGABRT:
    reason = "(SIGABRT) Usually caused by an abort() or assert()";
    break;
  case SIGFPE:
    reason = "(SIGFPE) Arithmetic exception, such as divide by zero";
    break;
  case SIGILL:
    reason = "(SIGILL) Illegal instruction";
    break;
  case SIGINT:
    reason = "(SIGINT) Interactive attention signal, (usually ctrl+c)";
    break;
  case SIGSEGV:
    reason = "(SIGSEGV) Segmentation Fault";
    break;
  case SIGTERM:
    reason = "(SIGTERM) A termination request was sent to the program";
    break;
  }

  // Avoid new signals inside the crash handler
  if (handling) return;

  handling = true;
  if (CrashHandler::trigger(reason, true)) _Exit(1);
  handling = false;
}

#endif

//-----------------------------------------------------------------------------
//-----------------------------------------------------------------------------

static void printSysInfo(std::string &out) {
  out.append("Build ABI: " + QSysInfo::buildAbi().toStdString() + "\n");
  out.append("Operating System: " + QSysInfo::prettyProductName().toStdString() + "\n");
  out.append("OS Kernel: " + QSysInfo::kernelVersion().toStdString() + "\n");
  out.append("CPU Threads: " + std::to_string(QThread::idealThreadCount()) + "\n");
}

//-----------------------------------------------------------------------------

static void printGPUInfo(std::string &out) {
  const char *gpuVendorName = (const char *)glGetString(GL_VENDOR);
  const char *gpuModelName  = (const char *)glGetString(GL_RENDERER);
  const char *gpuVersion    = (const char *)glGetString(GL_VERSION);
  if (gpuVendorName)
    out.append("GPU Vendor: " + std::string(gpuVendorName) + "\n");
  if (gpuModelName)
    out.append("GPU Model: " + std::string(gpuModelName) + "\n");
  if (gpuVersion)
    out.append("GPU Version: " + std::string(gpuVersion) + "\n");
}

//-----------------------------------------------------------------------------

CrashHandler::CrashHandler(QWidget *parent, TFilePath crashFile, QString crashReport)
    : QDialog(parent), m_crashFile(crashFile), m_crashReport(crashReport) {
  setWindowFlag(Qt::WindowContextHelpButtonHint, false);

  QStringList sl;
  sl.append(tr("<b>Flare crashed unexpectedly.</b>"));
  sl.append("");
  sl.append(tr("A crash report has been generated."));
  sl.append(
      tr("To report, click 'Open Issue Webpage' to access Flare's Issues "
         "page on GitHub."));
  sl.append(tr("Click on the 'New issue' button and fill out the form."));
  sl.append("");
  sl.append(tr("System Configuration and Problem Details:"));
  
  QLabel *headtext = new QLabel(sl.join("<br>"));
  headtext->setTextFormat(Qt::RichText);

  QTextEdit *reportTxt = new QTextEdit();
  reportTxt->setText(crashReport);
  reportTxt->setReadOnly(true);
  reportTxt->setLineWrapMode(QTextEdit::LineWrapMode::NoWrap);
  reportTxt->setStyleSheet(
      "background:white;\ncolor:black;\nborder:1 solid black;");

  QVBoxLayout *mainLayout = new QVBoxLayout();
  QHBoxLayout *buttonsLay = new QHBoxLayout();

  QPushButton *copyBtn   = new QPushButton(tr("Copy to Clipboard"));
  QPushButton *webBtn    = new QPushButton(tr("Open Issue Webpage"));
  QPushButton *folderBtn = new QPushButton(tr("Open Reports Folder"));
  QPushButton *closeBtn  = new QPushButton(tr("Close Application"));
  buttonsLay->addWidget(copyBtn);
  buttonsLay->addWidget(webBtn);
  buttonsLay->addWidget(folderBtn);
  buttonsLay->addWidget(closeBtn);

  mainLayout->addWidget(headtext);
  mainLayout->addWidget(reportTxt);
  mainLayout->addLayout(buttonsLay);

  bool ret = connect(copyBtn, SIGNAL(clicked()), this, SLOT(copyClipboard()));
  ret = ret && connect(webBtn, SIGNAL(clicked()), this, SLOT(openWebpage()));
  ret = ret && connect(folderBtn, SIGNAL(clicked()), this, SLOT(openFolder()));
  ret = ret && connect(closeBtn, SIGNAL(clicked()), this, SLOT(accept()));
  if (!ret) throw TException();

  setWindowTitle(tr("Flare crashed!"));
  setLayout(mainLayout);
}

void CrashHandler::reject() {
  QStringList sl;
  sl.append(tr("Application is in unstable state and must be restarted."));
  sl.append(tr("Resuming is not recommended and may lead to an unrecoverable crash."));
  sl.append(tr("Ignore advice and try to resume program?"));

  QMessageBox::StandardButton reply =
      QMessageBox::question(this, tr("Ignore crash?"), sl.join("\n"),
                            QMessageBox::Yes | QMessageBox::No);
  if (reply == QMessageBox::Yes) {
    QDialog::reject();
  }
}

//-----------------------------------------------------------------------------

void CrashHandler::copyClipboard() {
  QClipboard *clipboard = QApplication::clipboard();
  clipboard->setText(m_crashReport);
}

//-----------------------------------------------------------------------------

void CrashHandler::openWebpage() {
  QDesktopServices::openUrl(QUrl("https://github.com/Flare-Animate/Flare/issues"));
}  

//-----------------------------------------------------------------------------

void CrashHandler::openFolder() {
  TFilePath fp = FlareFolder::getCrashReportFolder();
  QDesktopServices::openUrl(QUrl("file:///" + fp.getQString()));
}

//-----------------------------------------------------------------------------

void CrashHandler::install() {
#ifdef _WIN32
  // std library seems to override this
  //SetUnhandledExceptionFilter(exceptionHandler);

  void *handler = AddVectoredExceptionHandler(0, exceptionHandler);
  assert(handler != NULL);
  //RemoveVectoredExceptionHandler(handler);
#else
  signal(SIGABRT, signalHandler);
  signal(SIGFPE, signalHandler);
  signal(SIGILL, signalHandler);
  signal(SIGINT, signalHandler);
  signal(SIGSEGV, signalHandler);
  signal(SIGTERM, signalHandler);
#endif
}

//-----------------------------------------------------------------------------

void CrashHandler::reportProjectInfo(bool enableReport) {
  s_reportProjInfo = enableReport;
}

//-----------------------------------------------------------------------------

void CrashHandler::attachParentWindow(QWidget *parent) {
  s_parentWindow = parent;
}

//-----------------------------------------------------------------------------

bool CrashHandler::trigger(const QString reason, bool showDialog) {
  char fileName[128];
  char dumpName[128];
  char dateName[128];
  std::string out;

  // Get time and build filename
  time_t acc_time;
  time(&acc_time);
  struct tm *tm = localtime(&acc_time);
  strftime(dateName, 128, "%Y-%m-%d %H:%M:%S", tm);
  strftime(fileName, 128, "Crash-%Y%m%d-%H%M%S.log", tm);
  strftime(dumpName, 128, "Crash-%Y%m%d-%H%M%S.dmp", tm);
  TFilePath fpCrsh = FlareFolder::getCrashReportFolder() + fileName;
  TFilePath fpDump = FlareFolder::getCrashReportFolder() + dumpName;

  // Generate minidump
  bool minidump = generateMinidump(fpDump);

  // Generate report
  try {
    out.append(TEnv::getApplicationFullName() + "  (Build " + __DATE__ ")\n");
    out.append("\nReport Date: ");
    out.append(dateName);
    out.append("\nCrash Reason: ");
    out.append(reason.toStdString());
    out.append("\n\n");
    printSysInfo(out);
    out.append("\n");
    printGPUInfo(out);
    out.append("\nCrash File: ");
    out.append(fpCrsh.getQString().toStdString());
#ifdef HAS_MINIDUMP
    out.append("\nMini Dump File: ");
    if (minidump)
      out.append(fpDump.getQString().toStdString());
    else
      out.append("Failed");
#endif
    out.append("\n");
  } catch (...) {
  }
  try {
    if (s_reportProjInfo) {
      auto currentProject = TProjectManager::instance()->getCurrentProject();
      TFilePath projectPath    = currentProject->getProjectPath();

      ToonzScene *currentScene = TApp::instance()->getCurrentScene()->getScene();
      std::wstring sceneName   = currentScene->getSceneName();

      out.append("\nApplication Dir: ");
      out.append(QCoreApplication::applicationDirPath().toStdString());
      out.append("\nStuff Dir: ");
      out.append(TEnv::getStuffDir().getQString().toStdString());
      out.append("\n");
      out.append("\nProject Name: ");
      out.append(currentProject->getName().getQString().toStdString());
      out.append("\nScene Name: ");
      out.append(QString::fromStdWString(sceneName).toStdString());
      out.append("\nProject Path: ");
      out.append(projectPath.getQString().toStdString());
      out.append("\nScene Path: ");
      out.append(currentScene->getScenePath().getQString().toStdString());
      out.append("\n");
    }
  } catch (...) {
  }
#ifdef HAS_MODULES
  try {
    out.append("\n==== Modules ====\n");
    printModules(out);
    out.append("==== End ====\n");
  } catch (...) {
  }
#endif
#ifdef HAS_BACKTRACE
  try {
    out.append("\n==== Backtrace ====\n");
    printBacktrace(out);
    out.append("==== End ====\n");
  } catch (...) {
  }
#endif

  // Save to crash information to file
  FILE *fw = fopen(fpCrsh, "w");
  if (fw != NULL) {
    fwrite(out.c_str(), 1, out.size(), fw);
    fclose(fw);
  }

  if (showDialog) {
    // Show crash handler dialog
    CrashHandler crashdialog(s_parentWindow, fpCrsh, QString::fromStdString(out));
    return crashdialog.exec() != QDialog::Rejected;
  }

  return true;
}

//-----------------------------------------------------------------------------

