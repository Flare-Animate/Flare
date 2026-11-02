$ErrorActionPreference = 'Stop'
$toolsDir   = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
$url = "https://github.com/Flare-Animate/Flare/releases/download/$env:PackageVersion/Flare-Windows-$env:PackageVersion.zip"

$packageArgs = @{
  packageName   = 'flare'
  unzipLocation = "$env:ProgramFiles\Flare"
  url           = $url
}

Install-ChocolateyZipPackage @packageArgs

# Optionally add to PATH
$targetPath = "$env:ProgramFiles\Flare"
if ((Test-Path $targetPath) -and -not ($env:Path -split ';' | Where-Object { $_ -eq $targetPath })) {
  [Environment]::SetEnvironmentVariable('Path', $env:Path + ';' + $targetPath, [System.EnvironmentVariableTarget]::Machine)
}
