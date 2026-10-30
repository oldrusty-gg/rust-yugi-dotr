# End-to-end test of online decks in the real game on this PC (loopback
# only): a local TLS hub with the server's computer opponent, then one game
# window driven by scripted keys: title -> Online Play -> DECKS -> NEW DECK
# (named, leader rank chosen) -> EDIT CARDS (the game's deck editor on every
# card, its pad scripted: 40 cards and a leader) -> back -> sign up ->
# PLAY COMPUTER -> CHOOSE DECK (the custom deck) -> COMPUTER DECK (RANDOM) ->
# CHOOSE MAP (RANDOM) -> duel (pad bot) -> result -> PLAY COMPUTER -> CHOOSE
# DECK (Pegasus Crawford's deck) -> COMPUTER DECK (Pegasus Crawford's) ->
# CHOOSE MAP (VS MAP 05) -> duel -> result.
# Menu frames: <Out>\shots\*.png; the deck builder's frame: <Out>\builder.png.
# Every process this starts is stopped before the script returns; muted.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\online-decks-e2e.ps1 -Out <new dir>
#       -Bevy <roses-bevy-scene.exe> -ServerBin <dir with roses-server.exe>
#       [-Game <game.json>] [-Save <slot folder>] [-Port 47996] [-TimeoutMin 30]
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [Parameter(Mandatory = $true)][string]$Bevy,
    [Parameter(Mandatory = $true)][string]$ServerBin,
    [string]$Game = "work\play\roses-data\game-20261007-claude-v27\game.json",
    [string]$Save = "work\runs\claude-2p-game\save\memory-card\slot1",
    [int]$Port = 47996,
    [int]$TimeoutMin = 30
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:ROSES_MUTE = "1"
if (Test-Path $Out) { throw "$Out exists; give a new folder" }
New-Item -ItemType Directory -Force $Out, (Join-Path $Out "shots") | Out-Null
$Out = (Resolve-Path $Out).Path
$Game = (Resolve-Path $Game).Path
$Bevy = (Resolve-Path $Bevy).Path
$server = (Resolve-Path (Join-Path $ServerBin "roses-server.exe")).Path
$import = Split-Path -Parent $Game
$hubDir = Join-Path $Out "hub"
New-Item -ItemType Directory -Force $hubDir | Out-Null
& $server tls self-signed --out (Join-Path $hubDir "cert") --name 127.0.0.1 --name localhost | Out-Null
$pin = (& $server tls fingerprint (Join-Path $hubDir "cert\cert.pem")).Trim()
$db = Join-Path $hubDir "accounts.db"
$data = Join-Path $Out "player"
New-Item -ItemType Directory -Force (Join-Path $data "memory-card\slot1") | Out-Null
Copy-Item (Join-Path $Save "*") (Join-Path $data "memory-card\slot1") -Recurse
@{ server = "127.0.0.1:$Port"; name = "Decks test"; pin = $pin } | ConvertTo-Json |
    Set-Content -Encoding ascii (Join-Path $data "online.json")
$log = Join-Path $Out "driver.log"
function Note($text) { $line = "$(Get-Date -Format HH:mm:ss) $text"; Write-Host $line; Add-Content $log $line }

# The deck builder's pad (one word per update, hex): Cross then Down 40
# times (40 different cards from the top of the trunk list), Up to the
# first row, Start (that card leads), a pause, Circle (done).
$words = @()
for ($i = 0; $i -lt 40; $i++) { $words += "40", "0", "4000", "0" }
$words += "800", "0"
for ($i = 0; $i -lt 12; $i++) { $words += "0" }
$words += "20", "0"
$env:ROSES_BUILDER_REPLAY = ($words -join ",")
$env:ROSES_EDITOR_CAPTURE = Join-Path $Out "builder.png"
$env:ROSES_EDITOR_CAPTURE_AT = "170"

$keys = "until:SERVER,shot:server,select:DECKS,x,until:DECKS,shot:decks-empty,select:NEW_DECK,x,until:DECK," +
    "`"Dragons`",down,right*4,shot:deck-menu,select:EDIT_CARDS,x,wait*5,until:DECK,shot:deck-built,z,until:DECKS,shot:decks-list,z," +
    "until:SERVER,select:CONNECT,x,until:ACCOUNT,select:SIGN_UP,x,until:SIGN_UP,`"Decker`",x,`"secret-pass-decker`",x,select:SIGN_UP,x," +
    "until:LOBBY,shot:lobby,select:PLAY_COMPUTER,x,until:CHOOSE_DECK,shot:choose-remembered,select:Dragons,shot:choose-dragons,x," +
    "until:COMPUTER_DECK,shot:computer-deck,x,until:CHOOSE_MAP,shot:choose-map,x," +
    "until:RESULT,shot:result-dragons,x,until:LOBBY,wait*25,select:PLAY_COMPUTER,x,until:CHOOSE_DECK,select:Pegasus_Crawford,shot:choose-pegasus,x," +
    "until:COMPUTER_DECK,select:Pegasus_Crawford,x,until:CHOOSE_MAP,select:VS_MAP_05,shot:choose-map-vs05,x," +
    "until:RESULT,shot:result-pegasus,x,until:LOBBY,shot:lobby-after,wait*25,exit"

$started = @()
try {
    $hub = Start-Process -FilePath $server -PassThru -NoNewWindow `
        -ArgumentList @("hub", "--duel-import", "`"$(Join-Path $import 'duel')`"", "--db", "`"$db`"", "--listen", "127.0.0.1:$Port",
            "--tls-cert", "`"$(Join-Path $hubDir 'cert\cert.pem')`"", "--tls-key", "`"$(Join-Path $hubDir 'cert\key.pem')`"",
            "--name", "`"Decks test`"", "--open-signup", "--computer", "--replay-dir", "`"$(Join-Path $hubDir 'replays')`"") `
        -RedirectStandardOutput (Join-Path $Out "hub.log") -RedirectStandardError (Join-Path $Out "hub.err")
    $started += $hub
    Start-Sleep -Seconds 2
    $env:ROSES_ONLINE_CONFIG = Join-Path $data "online.json"
    $env:ROSES_SAVE_DIR = $data
    $env:ROSES_ONLINE_KEYS = $keys
    $env:ROSES_ONLINE_PADBOT = "1"
    $env:ROSES_ONLINE_SHOTS = Join-Path $Out "shots"
    # The title's entries: New Game, Continue, Custom Duel, Trade, Online Play.
    $p = Start-Process -FilePath $Bevy -PassThru -WorkingDirectory (Split-Path -Parent $Bevy) `
        -ArgumentList @("--game", "`"$Game`"", "--start-stage", "main-menu", "--replay", "down,down,down,down,cross") `
        -RedirectStandardOutput (Join-Path $Out "game.out") -RedirectStandardError (Join-Path $Out "game.err")
    $null = $p.Handle
    $started += $p
    Note "started the game pid $($p.Id)"
    $deadline = (Get-Date).AddMinutes($TimeoutMin)
    while ((Get-Date) -lt $deadline -and -not $p.HasExited) { Start-Sleep -Seconds 2 }
    Note "game exited: $($p.HasExited) ($(if ($p.HasExited) { $p.ExitCode }))"
}
finally {
    foreach ($q in $started) {
        if ($q -and -not $q.HasExited) { Note "stopping pid $($q.Id)"; Stop-Process -Id $q.Id -Force -ErrorAction SilentlyContinue }
    }
    foreach ($v in "ROSES_ONLINE_CONFIG", "ROSES_SAVE_DIR", "ROSES_ONLINE_KEYS", "ROSES_ONLINE_PADBOT", "ROSES_ONLINE_SHOTS",
        "ROSES_BUILDER_REPLAY", "ROSES_EDITOR_CAPTURE", "ROSES_EDITOR_CAPTURE_AT") {
        Remove-Item "env:$v" -ErrorAction SilentlyContinue
    }
}
"--- custom-decks.json"; Get-Content (Join-Path $data "custom-decks.json") -ErrorAction SilentlyContinue
"--- online-deck.json"; Get-Content (Join-Path $data "online-deck.json") -ErrorAction SilentlyContinue
"--- hub.log"; Get-Content (Join-Path $Out "hub.log")
