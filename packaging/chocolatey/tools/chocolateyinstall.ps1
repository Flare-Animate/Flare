# Installs the portable Windows archive of the release being packed. The version and the
# SHA-256 are placeholders that the publish workflow fills from the release asset itself, so
# an edited or truncated download is refused instead of installed.
$ErrorActionPreference = 'Stop'

$installDir = Join-Path $env:ProgramFiles 'Flare'
$url = "https://github.com/Flare-Animate/Flare/releases/download/v__VERSION__/flare-__VERSION__-windows-x64-portable.zip"

$packageArgs = @{
  packageName   = 'flare'
  unzipLocation = $installDir
  url           = $url
  checksum      = '__CHECKSUM__'
  checksumType  = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

# On the machine PATH so the command line and the editor start from any terminal. The
# installer records the directory too, so uninstall takes it back out.
Install-ChocolateyPath -PathToInstall $installDir -PathType Machine

$desktop = Join-Path $installDir 'flare-desktop.exe'
if (Test-Path $desktop) {
  Install-ChocolateyShortcut -ShortcutFilePath (Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) 'Flare.lnk') -TargetPath $desktop
}
