; Flare Windows installer (NSIS 3).
; Build: makensis -DVERSION=0.1.0 -DBINDIR=<folder with the .exe files> packaging/windows/installer.nsi
; Per-user install (no admin prompt): %LOCALAPPDATA%\Programs\Flare.
; The C++ Flare installs per machine (HKLM, Program Files\Flare); this Rust build
; keeps to HKCU and its own folder, so both can be installed during the conversion.
; Replaces an earlier per-user AnimationCraft install (the app's former name).

; 64-bit installer (the app is 64-bit only)
Target amd64-unicode
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef BINDIR
  !define BINDIR "..\..\target\x86_64-pc-windows-gnu\release"
!endif
!define APP "Flare"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"

Name "${APP} ${VERSION}"
OutFile "${BINDIR}\Flare-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
InstallDirRegKey HKCU "Software\${APP}" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "..\..\assets\icon\flare.ico"
!define MUI_UNICON "..\..\assets\icon\flare.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\flare.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start Flare"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE-MIT"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Flare (required)" SecApp
  SectionIn RO
  ; remove AnimationCraft (this app before the rename): files, shortcuts, registry
  ReadRegStr $0 HKCU "Software\AnimationCraft" "InstallDir"
  StrCmp $0 "" noold
    IfFileExists "$0\uninstall.exe" 0 +2
      ExecWait '"$0\uninstall.exe" /S _?=$0'
    Delete "$0\uninstall.exe"
    RMDir "$0"
  noold:
  SetOutPath "$INSTDIR"
  File "${BINDIR}\flare.exe"
  File "${BINDIR}\flare-cli.exe"
  File "..\..\assets\icon\flare.ico"
  File /oname=LICENSE-MIT.txt "..\..\LICENSE-MIT"
  File /oname=LICENSE-APACHE.txt "..\..\LICENSE-APACHE"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APP}"
  CreateShortcut "$SMPROGRAMS\${APP}\${APP}.lnk" "$INSTDIR\flare.exe" "" "$INSTDIR\flare.ico"
  CreateShortcut "$SMPROGRAMS\${APP}\Uninstall ${APP}.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKCU "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP} (Rust)"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Flare contributors"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\flare.ico"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\flare.exe" "" "$INSTDIR\flare.ico"
SectionEnd

Section "Open .acx files with Flare" SecAssoc
  WriteRegStr HKCU "Software\Classes\.acx" "" "Flare.AcxDocument"
  WriteRegStr HKCU "Software\Classes\Flare.AcxDocument" "" "Flare document"
  WriteRegStr HKCU "Software\Classes\Flare.AcxDocument\DefaultIcon" "" "$INSTDIR\flare.ico"
  WriteRegStr HKCU "Software\Classes\Flare.AcxDocument\shell\open\command" "" '"$INSTDIR\flare.exe" "%1"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\flare.exe"
  Delete "$INSTDIR\flare-cli.exe"
  Delete "$INSTDIR\flare.ico"
  Delete "$INSTDIR\LICENSE-MIT.txt"
  Delete "$INSTDIR\LICENSE-APACHE.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP}\${APP}.lnk"
  Delete "$SMPROGRAMS\${APP}\Uninstall ${APP}.lnk"
  RMDir "$SMPROGRAMS\${APP}"
  Delete "$DESKTOP\${APP}.lnk"
  ReadRegStr $0 HKCU "Software\Classes\.acx" ""
  StrCmp $0 "Flare.AcxDocument" 0 +2
    DeleteRegKey HKCU "Software\Classes\.acx"
  DeleteRegKey HKCU "Software\Classes\Flare.AcxDocument"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\${APP}"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd
