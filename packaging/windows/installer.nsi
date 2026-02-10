; NSIS script for Flare installer (placeholder)
; This script is a starting point for creating an installer with NSIS

OutFile "Flare-Installer.exe"
InstallDir "$PROGRAMFILES\\Flare"
RequestExecutionLevel user

Section "Install"
  SetOutPath "$INSTDIR"
  ; Copy files here (use packaging steps in CI to populate "FlarePortable")
  ; File /r "FlarePortable\\*"
SectionEnd

Section "Create Shortcuts"
  ; Create desktop and start menu shortcuts
SectionEnd

Section "Uninstall"
  ; Remove files
SectionEnd
