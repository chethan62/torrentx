# Package a Windows release zip: the exe, both licences, and a short README.
# Called by release.yml (on a tag) and windows-native.yml (to prove the package
# step on a dispatch run). Not a build script — it expects target/release/torrentx.exe.
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Exe = "target/release/torrentx.exe",
  [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Exe)) { throw "missing built binary: $Exe" }
foreach ($f in @("LICENSE-MIT", "LICENSE-APACHE")) {
  if (-not (Test-Path $f)) { throw "missing $f next to the script's repo root" }
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$stage = Join-Path $OutDir "stage"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item $Exe $stage
Copy-Item LICENSE-MIT, LICENSE-APACHE $stage

$readme = @"
TorrentX $Version (Windows x64)

Run torrentx.exe. On first start it writes %APPDATA%\torrentx\config.json.

Requires a running Jackett instance: set its URL and API key in Settings
(Jackett's own API key, not an indexer's). Searches fan out over every
configured indexer.

Platform notes — these are Linux-only today:
  * the system tray icon (TorrentX closes to the taskbar instead)
  * "Play" streaming, which starts webtorrent-cli and mpv; both must be on PATH
    for it to work here
Everything else (search, favourites, RSS, export, the indexer window) runs the
same as on Linux.

Licence: MIT OR Apache-2.0 — see LICENSE-MIT and LICENSE-APACHE.
Source: https://github.com/chethan62/torrentx
"@
Set-Content -Path (Join-Path $stage "README.txt") -Value $readme -Encoding utf8

$zip = Join-Path $OutDir "TorrentX-$Version-windows-x64.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip

# Same "<hash>  <name>" shape as sha256sum, so `sha256sum -c` works on the result.
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
$sums = Join-Path $OutDir "sha256sums-windows.txt"
Set-Content -Path $sums -Value "$hash  $(Split-Path $zip -Leaf)" -Encoding ascii

Write-Host "packaged $zip"
Write-Host (Get-Content $sums -Raw)
