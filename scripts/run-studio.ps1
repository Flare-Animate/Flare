param(
    [ValidateSet('gl', 'dx12', 'vulkan')]
    [string]$Backend = 'vulkan',
    [switch]$Release
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoPath)) { throw 'Install the Rust toolchain before starting Flare Studio.' }
$previousBackend = $env:WGPU_BACKEND
try {
    $env:WGPU_BACKEND = $Backend
    Push-Location (Join-Path $repoRoot 'flare/sources')
    try {
        $cargoArgs = @('run', '--locked', '-p', 'flare-studio', '--bin', 'flare-studio')
        if ($Release) { $cargoArgs += '--release' }
        & $cargoPath @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw "Flare Studio exited with code $LASTEXITCODE" }
    } finally { Pop-Location }
} finally { $env:WGPU_BACKEND = $previousBackend }
