; Flare Windows installer. Build: makensis /DVERSION=x.y.z /DSRCDIR=FlarePortable packaging\windows\installer.nsi
!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SRCDIR
  !define SRCDIR "..\..\FlarePortable"
!endif
!define UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall\Flare"

Name "Flare ${VERSION}"
OutFile "Flare-Setup.exe"
InstallDir "$PROGRAMFILES64\Flare"
InstallDirRegKey HKLM "Software\Flare" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
!include "MUI2.nsh"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

!macro ASSOC EXT DESC
  WriteRegStr HKCR ".${EXT}" "" "Flare.${EXT}"
  WriteRegStr HKCR "Flare.${EXT}" "" "${DESC}"
  WriteRegStr HKCR "Flare.${EXT}\DefaultIcon" "" "$INSTDIR\Flare.exe,0"
  WriteRegStr HKCR "Flare.${EXT}\shell\open\command" "" '"$INSTDIR\Flare.exe" "%1"'
!macroend
!macro UNASSOC EXT
  DeleteRegKey HKCR ".${EXT}"
  DeleteRegKey HKCR "Flare.${EXT}"
!macroend

Section "Flare"
  SetOutPath "$INSTDIR"
  File /r "${SRCDIR}\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\Flare" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${UNINST}" "DisplayName" "Flare"
  WriteRegStr HKLM "${UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  !insertmacro ASSOC "fla" "Flare Animation"
  !insertmacro ASSOC "xfl" "Flare Animation (XFL)"
  !insertmacro ASSOC "swf" "Flash Movie"
  !insertmacro ASSOC "moho" "Moho Project"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  CreateDirectory "$SMPROGRAMS\Flare"
  CreateShortcut "$SMPROGRAMS\Flare\Flare.lnk" "$INSTDIR\Flare.exe"
  CreateShortcut "$DESKTOP\Flare.lnk" "$INSTDIR\Flare.exe"
SectionEnd

Section "Uninstall"
  !insertmacro UNASSOC "fla"
  !insertmacro UNASSOC "xfl"
  !insertmacro UNASSOC "swf"
  !insertmacro UNASSOC "moho"
  Delete "$SMPROGRAMS\Flare\Flare.lnk"
  RMDir "$SMPROGRAMS\Flare"
  Delete "$DESKTOP\Flare.lnk"
  DeleteRegKey HKLM "${UNINST}"
  DeleteRegKey HKLM "Software\Flare"
  RMDir /r "$INSTDIR"
SectionEnd
