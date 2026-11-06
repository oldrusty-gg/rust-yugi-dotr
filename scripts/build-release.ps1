# Builds the release package: release exes for the launcher, the importer
# (roses.exe) and the game (roses-bevy-scene.exe), the game's own shaders,
# README.txt and KNOWN-ISSUES.txt, and optionally online.json (an online
# server's address and certificate pin, so players need no setup), in
# work/release/roses-<date>/ plus a zip beside it, and latest.json (version,
# zip name, size, SHA-256) for the launcher's Update button: upload it with the
# zip, and name the GitHub tag exactly -Release, or the button will not appear.
# No game data, BIOS or research captures go in; scripts/release/audit.py
# checks that before the zip is written.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-release.ps1 [-Release 2026-10-06] [-Iso <your.iso>]
#       [-OnlineServer <host:port>] [-OnlinePin <sha256:...>] [-OnlineName <text>]
#
# -Iso lets the audit sample the ISO to prove none of its bytes are inside.
# -OnlineServer/-OnlinePin/-OnlineName: the online server written into the
# package's online.json (beside the exes; the launcher's Profile page and the
# game's Online Play read it when roses-data\online.json does not exist).
# No server address or pin is written in this script. When -OnlineServer
# is not given, the defaults come from the untracked, git-ignored file
# work\release\online.defaults.json:
#   { "server": "host:port", "pin": "sha256:<64 hex>", "name": "Roses" }
# Without that file (and without -OnlineServer) the package ships no
# online.json. -OnlineServer '' ships none even when the file exists.
# -OnlinePin '' means the server has a public (Let's Encrypt) certificate.
#
# -ForbiddenMemories <stage folder> (off by default; Jim decides when FM
# ships): also packages Yu-Gi-Oh! Forbidden Memories as the launcher's
# second game, from the folder forbidden2026's scripts\stage-launcher.ps1
# writes (work\stage there): fm.exe, fm-game.exe, assets\present.wgsl and
# games\forbidden-memories.json (the manifest the launcher's registry reads
# beside its exe; nothing of FM is built into the launcher). -FmDisc
# <your .bin> lets the audit sample that disc too. Without -ForbiddenMemories
# the package is exactly as before.
param(
    [string]$Release = (Get-Date -Format 'yyyy-MM-dd'),
    [string]$Iso = '',
    [string]$TargetDir = '',
    [switch]$SkipBuild,
    [string]$OnlineServer = '',
    [string]$OnlinePin = '',
    [string]$OnlineName = 'Roses',
    [string]$ForbiddenMemories = '',
    [string]$FmDisc = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
# The launcher compares this label with the latest release's tag.
if ($Release -cnotmatch '^\d{4}-\d{2}-\d{2}[a-z]*$') { throw "-Release $Release is not YYYY-MM-DD with optional lower-case letters" }
# The optional second game: check the stage before the long build.
$fmFiles = @('fm.exe', 'fm-game.exe', 'assets\present.wgsl', 'games\forbidden-memories.json')
if ($ForbiddenMemories) {
    $ForbiddenMemories = [IO.Path]::GetFullPath($ForbiddenMemories)
    foreach ($file in $fmFiles) {
        if (-not (Test-Path (Join-Path $ForbiddenMemories $file))) { throw "-ForbiddenMemories ${ForbiddenMemories}: missing $file (run forbidden2026\scripts\stage-launcher.ps1)" }
    }
}
# Online defaults from the local, git-ignored file (see above).
$onlineDefaults = Join-Path $repo 'work\release\online.defaults.json'
if (-not $PSBoundParameters.ContainsKey('OnlineServer') -and (Test-Path $onlineDefaults)) {
    $defaults = Get-Content -Raw $onlineDefaults | ConvertFrom-Json
    if ($defaults.server) { $OnlineServer = [string]$defaults.server }
    if (-not $PSBoundParameters.ContainsKey('OnlinePin') -and $defaults.pin) { $OnlinePin = [string]$defaults.pin }
    if (-not $PSBoundParameters.ContainsKey('OnlineName') -and $defaults.name) { $OnlineName = [string]$defaults.name }
    Write-Host "online defaults from $onlineDefaults"
} elseif (-not $PSBoundParameters.ContainsKey('OnlineServer')) {
    Write-Host 'no -OnlineServer and no work\release\online.defaults.json: the package ships no online.json'
}
# Check the online inputs before the long build.
if ($OnlineServer) {
    if ($OnlineServer -notmatch '^(\[[0-9A-Fa-f:]+\]|[A-Za-z0-9.-]+)(:\d{1,5})?$') { throw "-OnlineServer $OnlineServer is not host:port" }
    if ($OnlinePin) {
        $hex = ($OnlinePin -replace '^(?i)sha256:', '') -replace '[: -]', ''
        if ($hex -notmatch '^[0-9A-Fa-f]{64}$') { throw "-OnlinePin $OnlinePin is not sha256:<64 hex digits>" }
        $OnlinePin = 'sha256:' + $hex.ToLower()
    }
}
if (-not $TargetDir) { $TargetDir = Join-Path $repo 'work\runs\claude-release\target' }
# Absolute: the game is built from crates\roses-game, where a relative
# CARGO_TARGET_DIR would point somewhere else.
if (-not [IO.Path]::IsPathRooted($TargetDir)) { $TargetDir = Join-Path (Get-Location) $TargetDir }
$TargetDir = [IO.Path]::GetFullPath($TargetDir)
$triple = 'x86_64-pc-windows-msvc'
$env:CARGO_TARGET_DIR = $TargetDir
$env:ROSES_RELEASE = $Release
$env:ROSES_MUTE = '1'
# Static C runtime (no VC++ redistributable needed), no absolute paths of
# this PC in panic messages or the PDB reference, and Media Foundation's
# mfplay.dll (only for legacy .mp4 imports) loaded on first use, so the game
# also starts on Windows "N" editions without the Media Feature Pack.
$userHome = [Environment]::GetFolderPath('UserProfile')
$env:RUSTFLAGS = "-C target-feature=+crt-static --remap-path-prefix=$userHome=~ --remap-path-prefix=$repo=roses --remap-path-prefix=$(Split-Path -Parent $repo)=src -C link-arg=/PDBALTPATH:%_PDB% -C link-arg=/DELAYLOAD:mfplay.dll -C link-arg=delayimp.lib"

if (-not $SkipBuild) {
    Push-Location $repo
    try {
        cargo build --release --target $triple -p roses-cli
        if ($LASTEXITCODE -ne 0) { throw 'workspace release build failed' }
    } finally { Pop-Location }
    # The launcher: its own repository, checked out beside this one.
    Push-Location (Join-Path (Split-Path -Parent $repo) 'launcher')
    try {
        cargo build --release --target $triple -p roses-launcher
        if ($LASTEXITCODE -ne 0) { throw 'launcher release build failed' }
    } finally { Pop-Location }
    Push-Location (Join-Path $repo 'crates\roses-game')
    try {
        cargo build --release --target $triple --features renderer --bin roses-bevy-scene
        if ($LASTEXITCODE -ne 0) { throw 'game release build failed' }
    } finally { Pop-Location }
}

$built = Join-Path $TargetDir "$triple\release"
$releaseRoot = Join-Path $repo 'work\release'
$package = Join-Path $releaseRoot "roses-$Release"
if (Test-Path $package) { Remove-Item -Recurse -Force $package }
New-Item -ItemType Directory -Force (Join-Path $package 'assets') | Out-Null
foreach ($exe in 'roses-launcher.exe', 'roses.exe', 'roses-bevy-scene.exe') {
    Copy-Item (Join-Path $built $exe) $package
}
Copy-Item (Join-Path $repo 'crates\roses-game\assets\*.wgsl') (Join-Path $package 'assets')
# The launcher's brand and links (oldrusty; links are filled in when they exist).
Copy-Item (Join-Path $PSScriptRoot 'release\platform.json') $package
foreach ($text in 'README.txt', 'KNOWN-ISSUES.txt') {
    $body = (Get-Content -Raw (Join-Path $PSScriptRoot "release\$text")).Replace('{RELEASE}', $Release)
    # Notepad-friendly line endings.
    $body = $body -replace "(?<!`r)`n", "`r`n"
    [IO.File]::WriteAllText((Join-Path $package $text), $body, (New-Object Text.UTF8Encoding($false)))
}

if ($OnlineServer) {
    $online = [ordered]@{ server = $OnlineServer }
    if ($OnlineName) { $online.name = $OnlineName }
    if ($OnlinePin) { $online.pin = $OnlinePin }
    [IO.File]::WriteAllText((Join-Path $package 'online.json'), ($online | ConvertTo-Json) + "`r`n", (New-Object Text.UTF8Encoding($false)))
    Write-Host "online.json: $OnlineServer $(if ($OnlinePin) { "pin $OnlinePin" } else { '(public certificate)' })"
}

if ($ForbiddenMemories) {
    New-Item -ItemType Directory -Force (Join-Path $package 'games') | Out-Null
    foreach ($file in $fmFiles) {
        Copy-Item (Join-Path $ForbiddenMemories $file) (Join-Path $package $file)
    }
    Write-Host "Forbidden Memories packaged from $ForbiddenMemories"
}

$auditArgs = @((Join-Path $PSScriptRoot 'release\audit.py'), $package)
if ($Iso) { $auditArgs += @('--iso', $Iso) }
if ($ForbiddenMemories) { $auditArgs += '--with-forbidden-memories' }
python @auditArgs
if ($LASTEXITCODE -ne 0) { throw 'package audit failed; no zip written' }
if ($ForbiddenMemories -and $FmDisc) {
    # The FM exes must not hold bytes of the FM disc either.
    python (Join-Path $PSScriptRoot 'release\audit.py') $package --iso $FmDisc --with-forbidden-memories
    if ($LASTEXITCODE -ne 0) { throw 'package audit (FM disc) failed; no zip written' }
}

$zip = Join-Path $releaseRoot "roses-$Release.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $package -DestinationPath $zip -CompressionLevel Optimal
& (Join-Path $PSScriptRoot 'release\write-latest.ps1') -Zip $zip -Release $Release -Out (Join-Path $releaseRoot 'latest.json')
Get-Item $package, $zip | Select-Object FullName, @{n = 'MB'; e = { if ($_.PSIsContainer) { [math]::Round((Get-ChildItem $_ -Recurse -File | Measure-Object Length -Sum).Sum / 1MB, 1) } else { [math]::Round($_.Length / 1MB, 1) } } }
