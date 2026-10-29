# Builds dist\flare-<version>-windows-x86_64.zip holding the app, the command line tool and the MCP server.
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot)
$meta = cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json
$version = ($meta.packages | Where-Object name -eq "flare-desktop").version
cargo build --release --locked -p flare-desktop -p flare-cli -p flare-mcp
$name = "flare-$version-windows-x86_64"
Remove-Item -Recurse -Force "dist\$name" -ErrorAction SilentlyContinue
New-Item -ItemType Directory "dist\$name" | Out-Null
foreach ($f in "flare-desktop.exe", "flare-cli.exe", "flare-mcp.exe") { Copy-Item "target\release\$f" "dist\$name\" }
Copy-Item LICENSE, README.md, CHANGELOG.md "dist\$name\"
Compress-Archive -Path "dist\$name" -DestinationPath "dist\$name.zip" -Force
Write-Output "dist\$name.zip"
