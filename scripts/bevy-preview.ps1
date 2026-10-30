<#
.SYNOPSIS
Build and run the local Bevy history/Simon/name-entry development preview.
.DESCRIPTION
Requires privately imported opening assets and the recovered local scene fixture.
It does not download, extract or distribute original game data.
#>
[CmdletBinding()]
param(
    [string]$Scene = 'work/research/story/mesh-scene-simon.json',
    [string]$OpeningManifest = 'target/name-preview/release/roses-data/opening/manifest.json',
    [ValidateSet('intro', 'title', 'main-menu', 'historical-story', 'simon-arrival', 'name-entry', 'simon-post-name', 'simon-practice-intro', 'practice-choice', 'simon-deck-intro', 'simon-deck-prompt', 'deck-choice', 'simon-deck-chosen', 'seto-arrival', 'seto-side-intro', 'side-choice', 'seto-leaves', 'map-event-100', 'yugi-at-brest', 'map-events-101-102')]
    [string]$StartStage = 'simon-arrival',
    [string]$CloudSource,
    [ValidateRange(0, 10000)]
    [int]$CloudTicks = 0,
    [switch]$FreezeClouds,
    [ValidateRange(0, 1000)]
    [int]$CaptureLiveTicks = 0,
    [string]$Replay,
    [string]$Capture,
    [switch]$WorldOnly,
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    if (-not (Test-Path -LiteralPath $Scene -PathType Leaf)) {
        throw "Local scene fixture is missing: $Scene"
    }
    if (-not $WorldOnly -and -not (Test-Path -LiteralPath $OpeningManifest -PathType Leaf)) {
        throw "Imported opening manifest is missing: $OpeningManifest"
    }
    if ($WorldOnly -and ($Replay -or $CloudSource -or $CloudTicks -or $FreezeClouds -or $StartStage -eq 'historical-story')) {
        throw 'Replay and clouds need the opening presentation; omit -WorldOnly.'
    }
    if ($StartStage -eq 'historical-story' -and -not $CloudSource) {
        $CloudSource = 'work/research/story/clouds-rust-oracle/source-counter437-20260929.json'
    }
    # Title/menu reach the history via New Game; use the checkpoint when present.
    $defaultClouds = 'work/research/story/clouds-rust-oracle/source-counter437-20260929.json'
    if (($StartStage -in @('intro', 'title', 'main-menu')) -and -not $CloudSource -and -not $WorldOnly -and (Test-Path -LiteralPath $defaultClouds -PathType Leaf)) {
        $CloudSource = $defaultClouds
    }
    if (($CloudTicks -or $FreezeClouds) -and -not $CloudSource) {
        throw 'Cloud controls require -CloudSource or -StartStage historical-story.'
    }
    if ($CloudSource -and -not (Test-Path -LiteralPath $CloudSource -PathType Leaf)) {
        throw "Private cloud checkpoint is missing: $CloudSource"
    }
    if (-not $NoBuild) {
        cargo +stable build --manifest-path crates/roses-game/Cargo.toml --features renderer --bin roses-bevy-scene --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'Bevy preview build failed.' }
    }
    $previewExe = Join-Path $projectRoot 'crates/roses-game/target/debug/roses-bevy-scene.exe'
    if (-not (Test-Path -LiteralPath $previewExe -PathType Leaf)) {
        throw 'Bevy executable is missing. Run again without -NoBuild.'
    }
    $previewArgs = @('--scene', $Scene, '--data-root', $projectRoot)
    if (-not $WorldOnly) {
        $previewArgs += @('--opening-manifest', $OpeningManifest, '--start-stage', $StartStage)
    }
    if ($Replay) { $previewArgs += @('--replay', $Replay) }
    if ($CloudSource) { $previewArgs += @('--cloud-source', $CloudSource, '--cloud-ticks', $CloudTicks.ToString()) }
    if ($FreezeClouds) { $previewArgs += '--freeze-clouds' }
    if ($CaptureLiveTicks) { $previewArgs += @('--capture-live-ticks', $CaptureLiveTicks.ToString()) }
    if ($Capture) { $previewArgs += @('--capture', $Capture) }
    & $previewExe @previewArgs
    if ($LASTEXITCODE -ne 0) { throw "Bevy preview exited with code $LASTEXITCODE." }
}
finally {
    Pop-Location
}
