# Rehearse the YouTube take on this computer only (video aid): a loopback
# hub (no TLS, open sign-up, the match seed fixed with --match-seed, the
# server's replay kept) and scripts\video\run-online-video.ps1 against it
# with two new accounts (or, -Mode 2v2 / ffa, scripts\video\run-modes-video.ps1
# with four: a MODES room), headless, player 1's key frames written; then
# the timeline (minute:second -> event) from player 1's bot log and the
# server's replay. Every process this starts is stopped before it returns;
# muted.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\video\rehearse-local.ps1 `
#       -Out work\video\rehearsal\run-1 -Seed 7 [-Map "VS MAP 13"] [-Port 47991] [-Pace human]
#       [-ServerBin work\runs\claude-video-server\debug] [-Record] [-RecordArgs "..."]
#       [-Mode 1v1|2v2|ffa -Decks a,b,c,d -Ais 19,15,1,9 [-Seats S,N,E,W] [-StartLp 4000]]
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [long]$Seed = 1,
    [string]$Map = "VS MAP 13",
    [int]$Port = 47991,
    [ValidateSet("human", "off")][string]$Pace = "human",
    [string]$ServerBin = "work\runs\claude-video-server\debug",
    [string]$Game = "work\runs\claude-video\game\game.json",
    [string]$Bevy = "work\runs\claude-video\bin-release2\roses-bevy-scene.exe",
    [switch]$Record,
    [string]$RecordArgs = "",
    [int]$TimeoutMin = 60,
    # The server's turn clock (ms; 0 = none). The default 120 s is shorter
    # than a paced turn with several battle scenes (their animations alone
    # can take 60-90 s); a timed-out turn ends early and the duel then
    # differs from the seed search. (MODES rooms use their own clock: the
    # modes script sets NO CLOCK.)
    [int]$TurnLimitMs = 600000,
    [ValidateSet("1v1", "2v2", "ffa")][string]$Mode = "1v1",
    [string[]]$Decks = @(),
    [string[]]$Ais = @(),
    [string[]]$Seats = @("S", "N", "E", "W"),
    [int]$StartLp = 4000,
    [int]$ExodiaPlayer = 0,
    [string[]]$Accounts = @("JimTest", "mcduck3000", "Jipps", "Duelist4")
)
$ErrorActionPreference = "Stop"
# `powershell -File` passes "a,b,c,d" as one string: split it.
function Split-List($list) { return , @($list | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' }) }
$Decks = Split-List $Decks; $Ais = Split-List $Ais; $Seats = Split-List $Seats; $Accounts = Split-List $Accounts
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root
$env:ROSES_MUTE = "1"
if (Test-Path $Out) { throw "$Out exists; give a new folder" }
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$server = (Resolve-Path (Join-Path $ServerBin "roses-server.exe")).Path
$import = Split-Path -Parent (Resolve-Path $Game).Path
$hubDir = Join-Path $Out "hub"
New-Item -ItemType Directory -Force $hubDir | Out-Null
$hub = Start-Process -FilePath $server -PassThru -NoNewWindow `
    -ArgumentList @("hub", "--duel-import", "`"$(Join-Path $import 'duel')`"", "--db", "`"$(Join-Path $hubDir 'accounts.db')`"",
        "--local", "127.0.0.1:$Port", "--name", "`"Video rehearsal`"", "--open-signup", "--match-seed", "$Seed",
        "--replay-dir", "`"$(Join-Path $hubDir 'replays')`"", "--turn-limit-ms", "$TurnLimitMs") `
    -RedirectStandardOutput (Join-Path $hubDir "hub.log") -RedirectStandardError (Join-Path $hubDir "hub.err")
$seatNames = $null
try {
    Start-Sleep -Seconds 2
    $take = Join-Path $Out "take"
    $extra = @{}
    if ($Record) { $extra.Record = $true }
    if ($RecordArgs) { $extra.RecordArgs = $RecordArgs }
    if ($Mode -eq "1v1") {
        & (Join-Path $PSScriptRoot "run-online-video.ps1") -Server "127.0.0.1:$Port" -NoTls -SignUp `
            -Account1 "JimTest" -Password1 "rehearsal-pass-1" -Account2 "mcduck3000" -Password2 "rehearsal-pass-2" `
            -Seed $Seed -Map $Map -Pace $Pace -Out $take -Frames -Game $Game -Bevy $Bevy -TimeoutMin $TimeoutMin @extra
    } else {
        $passwords = @(1..4 | ForEach-Object { "rehearsal-pass-$_" })
        & (Join-Path $PSScriptRoot "run-modes-video.ps1") -Server "127.0.0.1:$Port" -NoTls -SignUp -Mode $Mode `
            -Accounts $Accounts -Passwords $passwords -Decks $Decks -Ais $Ais -Seats $Seats -StartLp $StartLp -ExodiaPlayer $ExodiaPlayer `
            -Seed $Seed -Map $Map -Pace $Pace -Out $take -Frames -Game $Game -Bevy $Bevy -TimeoutMin $TimeoutMin @extra
        # The seats' names by side for the timeline (side = seat: S, N, E, W).
        $bySide = @("", "", "", "")
        for ($k = 0; $k -lt 4; $k++) { $bySide[[array]::IndexOf(@("S", "N", "E", "W"), $Seats[$k].ToUpper())] = $Accounts[$k] }
        $seatNames = $bySide -join ","
    }
}
finally {
    if (-not $hub.HasExited) { Stop-Process -Id $hub.Id -Force -ErrorAction SilentlyContinue }
}
$replay = Get-ChildItem (Join-Path $hubDir "replays") -Filter *.json -ErrorAction SilentlyContinue | Select-Object -First 1
python -I (Join-Path $PSScriptRoot "timeline.py") --bot (Join-Path $Out "take\player1\bot.jsonl") `
    --names (Join-Path $import "duel\card-names.json") --out (Join-Path $Out "timeline.md") --game-err (Join-Path $Out "take\player1\game.err") `
    $(if ($replay) { @("--replay", $replay.FullName) }) $(if ($seatNames) { @("--seats", $seatNames) })
Get-Content (Join-Path $hubDir "hub.log") -ErrorAction SilentlyContinue | Select-Object -Last 15
