# Two-window test of the friends online beta on this PC (loopback only):
# a local TLS hub with an invite, then two real game windows, each with its
# own data folder (online.json + memory-card) driven by scripted keys
# (ROSES_ONLINE_KEYS for the Online Play screens, ROSES_ONLINE_PADBOT=1 for
# the duels): title -> Online Play -> sign up (two accounts) -> lobby ->
# challenge/accept -> duel -> result -> lobby; then a room-code match in
# which window B is killed mid-duel and started again (it rejoins with its
# saved session and the lobby's ticket); then both log out.
# Screenshots: <Out>\shots\{A,B}\*.png (menu frames written by the game)
# and <Out>\win\*.png (the whole windows, PrintWindow). Every process this
# starts is stopped before the script returns; sound is muted.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\friends-two-windows.ps1 -Out <new dir>
#       [-Bevy <roses-bevy-scene.exe>] [-ServerBin <dir with roses-server.exe>]
#       [-Game <game.json>] [-SaveA <slot folder>] [-SaveB <slot folder>] [-Port 47995]
#       [-NoReconnect] [-TimeoutMin 40]
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Bevy = "work\online-bin\roses-bevy-scene.exe",
    [string]$ServerBin = "work\online-bin",
    [string]$Game = "work\play\roses-data\game-20261004-claude-v25\game.json",
    [string]$SaveA = "work\runs\claude-2p-game\save\memory-card\slot1",
    [string]$SaveB = "work\runs\claude-2p-game\save\memory-card\slot2",
    [int]$Port = 47995,
    [switch]$NoReconnect,
    [int]$TimeoutMin = 40
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:ROSES_MUTE = "1"
if (Test-Path $Out) { throw "$Out exists; give a new folder" }
New-Item -ItemType Directory -Force $Out | Out-Null
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
$invite = (& $server accounts --db $db invite --uses 4 --note "two-window test").Trim()
"invite $invite, pin $pin"
$roomFile = Join-Path $Out "room-code.txt"
$capture = Join-Path $root "scripts\release\capture-window.ps1"

function New-DataFolder($name, $save) {
    $dir = Join-Path $Out $name
    New-Item -ItemType Directory -Force (Join-Path $dir "memory-card\slot1"), (Join-Path $Out "shots\$name") | Out-Null
    Copy-Item (Join-Path $save "*") (Join-Path $dir "memory-card\slot1") -Recurse
    @{ server = "127.0.0.1:$Port"; name = "Two-window test"; pin = $pin } | ConvertTo-Json |
        Set-Content -Encoding ascii (Join-Path $dir "online.json")
    $dir
}
$dataA = New-DataFolder "A" $SaveA
$dataB = New-DataFolder "B" $SaveB

$started = @()
$log = Join-Path $Out "driver.log"
function Note($text) { $line = "$(Get-Date -Format HH:mm:ss) $text"; Write-Host $line; Add-Content $log $line }

function Start-Client($name, $data, $keys, $tag) {
    $env:ROSES_ONLINE_CONFIG = Join-Path $data "online.json"
    $env:ROSES_SAVE_DIR = $data
    $env:ROSES_ONLINE_KEYS = $keys
    $env:ROSES_ONLINE_PADBOT = "1"
    $env:ROSES_ONLINE_SHOTS = Join-Path $Out "shots\$name"
    $env:ROSES_ONLINE_ROOM_FILE = $roomFile
    # The title's entries: New Game, Continue, Custom Duel, Trade, Online Play.
    $p = Start-Process -FilePath $Bevy -PassThru -WorkingDirectory (Split-Path -Parent $Bevy) `
        -ArgumentList @("--game", "`"$Game`"", "--start-stage", "main-menu", "--replay", "down,down,down,down,cross") `
        -RedirectStandardOutput (Join-Path $Out "$name-$tag.out") -RedirectStandardError (Join-Path $Out "$name-$tag.err")
    $null = $p.Handle  # keeps the exit code readable
    Note "started $name ($tag) pid $($p.Id)"
    $p
}

function Shot($p, $name) {
    if ($p -and -not $p.HasExited) {
        try { & $capture -ProcessId $p.Id -Out (Join-Path $Out "win\$name.png") -Wait 0 | Out-Null; Note "window shot $name" }
        catch { Note "window shot $name failed: $_" }
    }
}

function Count($file, $pattern) {
    if (-not (Test-Path $file)) { return 0 }
    @(Select-String -Path $file -Pattern $pattern -SimpleMatch).Count
}

$signup = { param($n) "until:SERVER,shot:server,x,until:ACCOUNT,select:SIGN_UP,x,until:SIGN_UP,`"$n`",x,`"secret-pass-$n`",x,`"$invite`",x,shot:signup,x,until:LOBBY" }
# A: sign up, challenge B, duel 1, result; create a room, duel 2, result; log out.
$keysA = (& $signup "Alice") + ",player,wait*25,shot:lobby,select:Bobby,x,until:CHALLENGE,shot:challenge-sent," +
    "until:RESULT,select:OK,shot:result1,x,until:LOBBY,wait*50,select:CREATE_ROOM,x,until:ROOM,roomout,shot:room," +
    "until:RESULT,select:OK,shot:result2,x,until:LOBBY,wait*100,select:LOG_OUT,shot:before-logout,x,until:ACCOUNT,shot:logged-out,wait*25,exit"
# B: sign up, accept, duel 1, result; join the room by code, duel 2 (killed in it).
$keysB = (& $signup "Bobby") + ",player,wait*25,shot:lobby,until:CHALLENGE,shot:challenged,select:ACCEPT,x," +
    "until:RESULT,select:OK,shot:result1,x,until:LOBBY,wait*50,select:JOIN_ROOM,x,until:JOIN_ROOM,roomin,shot:room-code,x,x," +
    "until:RESULT,select:OK,shot:result2,x,until:LOBBY,wait*50,select:LOG_OUT,x,until:ACCOUNT,shot:logged-out,wait*25,exit"
# B after the restart: Connect signs in with the saved session; the lobby
# sends the match's ticket again and the game rejoins the duel.
$keysB2 = "until:SERVER,shot:restart-server,x,until:RESULT,select:OK,shot:result2,x,until:LOBBY,wait*50,select:LOG_OUT,shot:before-logout,x,until:ACCOUNT,shot:logged-out,wait*25,exit"

try {
    if (Test-Path $roomFile) { Remove-Item $roomFile }
    $hub = Start-Process -FilePath $server -PassThru -NoNewWindow `
        -ArgumentList @("hub", "--duel-import", "`"$(Join-Path $import 'duel')`"", "--db", "`"$db`"", "--listen", "127.0.0.1:$Port",
            "--tls-cert", "`"$(Join-Path $hubDir 'cert\cert.pem')`"", "--tls-key", "`"$(Join-Path $hubDir 'cert\key.pem')`"",
            "--name", "`"Two-window test`"", "--replay-dir", "`"$(Join-Path $hubDir 'replays')`"") `
        -RedirectStandardOutput (Join-Path $Out "hub.log") -RedirectStandardError (Join-Path $Out "hub.err")
    $started += $hub
    Start-Sleep -Seconds 2
    $a = Start-Client "A" $dataA $keysA "run1"; $started += $a
    Start-Sleep -Seconds 3
    $b = Start-Client "B" $dataB $keysB "run1"; $started += $b
    $errA = Join-Path $Out "A-run1.err"; $errB = Join-Path $Out "B-run1.err"
    $deadline = (Get-Date).AddMinutes($TimeoutMin)
    $done = @{}
    $restarted = $false
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        $matchesA = Count $errA "the match began"
        $resultsA = Count $errA "back from the duel"
        if (-not $done.lobby -and (Count $errA "shots\A\lobby.png") -gt 0 -and (Count $errB "shots\B\lobby.png") -gt 0) {
            $done.lobby = 1; Shot $a "A-lobby"; Shot $b "B-lobby"
        }
        if (-not $done.duel1 -and $matchesA -ge 1) { $done.duel1 = 1; Start-Sleep -Seconds 25; Shot $a "A-duel1"; Shot $b "B-duel1" }
        if (-not $done.duel1b -and $done.duel1 -and $matchesA -ge 1) { $done.duel1b = 1; Start-Sleep -Seconds 60; Shot $a "A-duel1-later"; Shot $b "B-duel1-later" }
        if (-not $done.result1 -and $resultsA -ge 1) { $done.result1 = 1; Start-Sleep -Seconds 2; Shot $a "A-result1"; Shot $b "B-result1" }
        if (-not $done.duel2 -and $matchesA -ge 2) {
            $done.duel2 = 1
            Start-Sleep -Seconds 30
            Shot $a "A-duel2"; Shot $b "B-duel2"
            if (-not $NoReconnect -and -not $b.HasExited) {
                Note "killing B (pid $($b.Id)) mid-duel"
                Stop-Process -Id $b.Id -Force
                Start-Sleep -Seconds 3
                Shot $a "A-duel2-paused"
                $b = Start-Client "B" $dataB $keysB2 "run2"; $started += $b
                $errB = Join-Path $Out "B-run2.err"
                $restarted = $true
            }
        }
        if ($restarted -and -not $done.rejoined -and (Count $errB "the match began") -ge 1) {
            $done.rejoined = 1; Note "B rejoined the match"; Start-Sleep -Seconds 15; Shot $a "A-duel2-rejoined"; Shot $b "B-duel2-rejoined"
        }
        if ($a.HasExited -and $b.HasExited) { break }
    }
    Note "A exited: $($a.HasExited) ($(if ($a.HasExited) { $a.ExitCode })), B exited: $($b.HasExited) ($(if ($b.HasExited) { $b.ExitCode }))"
}
finally {
    foreach ($p in $started) {
        if ($p -and -not $p.HasExited) { Note "stopping pid $($p.Id)"; Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
    }
    foreach ($v in "ROSES_ONLINE_CONFIG", "ROSES_SAVE_DIR", "ROSES_ONLINE_KEYS", "ROSES_ONLINE_PADBOT", "ROSES_ONLINE_SHOTS", "ROSES_ONLINE_ROOM_FILE") {
        Remove-Item "env:$v" -ErrorAction SilentlyContinue
    }
}
"--- hub.log"; Get-Content (Join-Path $Out "hub.log")
