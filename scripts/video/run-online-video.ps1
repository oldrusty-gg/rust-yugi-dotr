# The YouTube take (video aid): two game clients against one server, both
# driven through their pads at a person's pace by the game's own computer
# opponents (Yugi's module for player 1, Seto's for player 2), from the
# title through Online Play to the end of a full duel:
#
#   player 1 (Jim's account): title -> Online Play -> CONNECT -> LOG IN -> LOBBY -> DECKS ->
#             COPY A DECK (Yugi; the copy opens the game's deck editor: two
#             cards out, Blue-Eyes White Dragon and Dark Magician in) ->
#             back -> CREATE ROOM -> CHOOSE DECK (the copy) -> CHOOSE MAP ->
#             ROOM (its code goes to player 2) -> the duel -> RESULT
#   player 2 (mcduck3000): title -> Online Play -> CONNECT -> LOG IN -> JOIN ROOM ->
#             CHOOSE DECK (Seto) -> the room code -> the duel -> RESULT
#
# Player 1's view is recorded (-Record: the in-engine recorder's --record,
# when the build has it) or, headless, written as key frames (-Frames).
# Every process this starts is stopped before the script returns; the game
# is muted unless -Sound is given.
#
# The same seed gives the same duel only on a server that uses that seed
# for the match (a loopback hub started with `roses-server hub ...
# --match-seed <seed>`; scripts/video/rehearse-local.ps1 does that). A
# server with random match seeds deals other hands, so the duel differs.
# The server's turn clock must also be longer than the default 120 s
# (`--turn-limit-ms 600000`): a paced turn with several battle scenes can
# pass 120 s, and a turn the server ends early changes the rest of the duel.
# So the live take needs the hub restarted with `--match-seed <seed>
# --turn-limit-ms 600000` for its duration (the lead decides, with Jim).
#
# NOT FOR THE LIVE SERVER YET: anything but a loopback address is refused
# unless -LiveServerApproved is given (the lead passes it once Jim agrees).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\video\run-online-video.ps1 `
#       -Server 127.0.0.1:47990 -NoTls -Account1 <Jim's account> -Password1 <pw> [-Account2 mcduck3000] -Password2 <pw> `
#       -Seed 7 -Map "VS MAP 13" -Out work\video\take-1 [-SignUp] [-Pace human|off]
#       [-Record] [-RecordArgs "--record <file.mp4>"] [-Frames] [-Window]
#       [-Game <game.json>] [-Bevy <roses-bevy-scene.exe>] [-Save <memory-card slot folder>]
#       [-BuilderWords <file>] [-Pin sha256:...] [-TimeoutMin 60]
param(
    [Parameter(Mandatory = $true)][string]$Server,
    [Parameter(Mandatory = $true)][string]$Account1,
    [Parameter(Mandatory = $true)][string]$Password1,
    [string]$Account2 = "mcduck3000",
    [Parameter(Mandatory = $true)][string]$Password2,
    [Parameter(Mandatory = $true)][string]$Out,
    [long]$Seed = 1,
    [string]$Map = "VS MAP 13",
    [ValidateSet("human", "off")][string]$Pace = "human",
    [string]$Ai1 = "yugi",
    [string]$Ai2 = "seto",
    [int]$Tier = 8,
    [string]$CopyDeck = "Yugi",
    [string]$Deck2 = "Seto",
    [string]$Game = "work\runs\claude-video\game\game.json",
    [string]$Bevy = "work\runs\claude-video\bin-release2\roses-bevy-scene.exe",
    [string]$Save = "work\runs\claude-2p-game\save\memory-card\slot1",
    [string]$BuilderWords = "work\video\decks\builder-words.txt",
    [string]$Pin = "",
    [switch]$NoTls,
    [switch]$SignUp,
    [switch]$Record,
    [string]$RecordArgs = "",
    [switch]$Frames,
    [int]$FrameEveryS = 20,
    [switch]$Window,
    [switch]$Sound,
    [switch]$LiveServerApproved,
    [int]$TimeoutMin = 60
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root

# --- safety: loopback only until the live take is approved ---------------
$hostPart = ($Server -replace ':\d+$', '').Trim('[', ']')
$loopback = $hostPart -in @("127.0.0.1", "localhost", "::1")
if (-not $loopback -and -not $LiveServerApproved) {
    throw "$Server is not this computer: the live take needs -LiveServerApproved (Jim's OK first)"
}
if ($NoTls -and -not $loopback) { throw "-NoTls is for a loopback server only" }
if ($SignUp -and -not $loopback -and -not $LiveServerApproved) { throw "-SignUp makes new accounts: loopback, or a live take Jim approved" }
if (-not $Sound) { $env:ROSES_MUTE = "1" }

if (Test-Path $Out) { throw "$Out exists; give a new folder" }
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Game = (Resolve-Path $Game).Path
$Bevy = (Resolve-Path $Bevy).Path
$words = (Get-Content (Resolve-Path $BuilderWords) -Raw).Trim()
$roomFile = Join-Path $Out "room-code.txt"
$stopFile = Join-Path $Out "stop-recording"
$log = Join-Path $Out "driver.log"
function Note($text) { $line = "$(Get-Date -Format HH:mm:ss) $text"; Write-Host $line; Add-Content $log $line }
Note "server $Server, seed $Seed, pace $Pace, map $Map, modules $Ai1 / $Ai2 (tier $Tier)"

# --- one data folder per player -------------------------------------------
function New-PlayerData($name, $account) {
    $data = Join-Path $Out $name
    New-Item -ItemType Directory -Force (Join-Path $data "memory-card\slot1"), (Join-Path $data "shots") | Out-Null
    if (Test-Path $Save) { Copy-Item (Join-Path $Save "*") (Join-Path $data "memory-card\slot1") -Recurse }
    $cfg = [ordered]@{ server = $Server; name = "Roses Online" }
    if ($NoTls) { $cfg.tls = $false } elseif ($Pin) { $cfg.pin = $Pin }
    $cfg | ConvertTo-Json | Set-Content -Encoding ascii (Join-Path $data "online.json")
    return $data
}
$data1 = New-PlayerData "player1" $Account1
$data2 = New-PlayerData "player2" $Account2

# --- the menu scripts (online_render.rs: ROSES_ONLINE_KEYS) ---------------
function Q($text) { return '"' + $text + '"' }
function Sel($label) { return "select:" + ($label -replace ' ', '_') }
function SignIn($account, $password) {
    if ($SignUp) {
        return "until:SERVER,shot:01-server,wait*20,select:CONNECT,x,until:ACCOUNT,shot:02-account,select:SIGN_UP,x,until:SIGN_UP," +
            (Q $account) + ",x," + (Q $password) + ",x,select:SIGN_UP,x,until:LOBBY"
    }
    return "until:SERVER,shot:01-server,wait*20,select:CONNECT,x,until:ACCOUNT,shot:02-account,select:LOG_IN,x,until:LOG_IN," +
        (Q $account) + ",x," + (Q $password) + ",x,select:LOG_IN,x,until:LOBBY"
}
$copy = "$CopyDeck (copy)"
$keys1 = (SignIn $Account1 $Password1) + ",shot:03-lobby,wait*25," +
    "select:DECKS,x,until:DECKS,shot:04-decks,wait*15,select:COPY_A_DECK,x,until:COPY_A_DECK,$(Sel $CopyDeck),wait*25,shot:05-copy,x," +
    "wait*5,until:DECK,shot:06-copied-and-edited,wait*20," +
    "shot:08-deck-edited,wait*40,z,until:DECKS,shot:08b-decks,wait*25,z,until:LOBBY,wait*20," +
    "select:CREATE_ROOM,x,until:CHOOSE_DECK,$(Sel $copy),shot:09-choose-deck,wait*15,x,until:CHOOSE_MAP,$(Sel $Map),wait*40,shot:10-choose-map,x," +
    "until:ROOM,roomout,shot:11-room," +
    "until:RESULT,shot:13-result,wait*75,select:OK,x,until:LOBBY,shot:14-lobby-after,wait*50,exit"
$keys2 = (SignIn $Account2 $Password2) + ",wait*25," +
    "select:JOIN_ROOM,x,until:CHOOSE_DECK,$(Sel $Deck2),wait*15,shot:05-choose-deck,x,until:JOIN_ROOM,roomin,shot:06-room-code,x,x," +
    "until:RESULT,shot:13-result,wait*75,select:OK,x,until:LOBBY,wait*50,exit"
Set-Content -Encoding ascii (Join-Path $Out "keys-player1.txt") $keys1
Set-Content -Encoding ascii (Join-Path $Out "keys-player2.txt") $keys2

# --- start a client --------------------------------------------------------
$vars = "ROSES_ONLINE_CONFIG", "ROSES_SAVE_DIR", "ROSES_ONLINE_KEYS", "ROSES_ONLINE_PADBOT", "ROSES_ONLINE_SHOTS",
    "ROSES_ONLINE_ROOM_FILE", "ROSES_BUILDER_REPLAY", "ROSES_EDITOR_CAPTURE", "ROSES_EDITOR_CAPTURE_AT", "ROSES_BOT_PACE",
    "ROSES_BOT_SEED", "ROSES_BOT_AI", "ROSES_BOT_TIER", "ROSES_BOT_LOG", "ROSES_CAPTURE_EXTRA_S", "ROSES_VIDEO_FRAMES",
    "ROSES_VIDEO_FRAME_EVERY_S", "ROSES_RECORD_STOP_FILE", "ROSES_RECORD_WAV"
function Clear-Vars { foreach ($v in $vars) { Remove-Item "env:$v" -ErrorAction SilentlyContinue } }
function Start-Player($name, $data, $keys, $botSeed, $ai, $recordThis) {
    Clear-Vars
    $env:ROSES_ONLINE_CONFIG = Join-Path $data "online.json"
    $env:ROSES_SAVE_DIR = $data
    $env:ROSES_ONLINE_KEYS = $keys
    $env:ROSES_ONLINE_PADBOT = "1"
    $env:ROSES_ONLINE_SHOTS = Join-Path $data "shots"
    $env:ROSES_ONLINE_ROOM_FILE = $roomFile
    $env:ROSES_BOT_PACE = $Pace
    $env:ROSES_BOT_SEED = "$botSeed"
    $env:ROSES_BOT_AI = $ai
    $env:ROSES_BOT_TIER = "$Tier"
    $env:ROSES_BOT_LOG = Join-Path $data "bot.jsonl"
    $env:ROSES_CAPTURE_EXTRA_S = "$($TimeoutMin * 60)"
    if ($name -eq "player1") {
        $env:ROSES_BUILDER_REPLAY = $words
        $env:ROSES_EDITOR_CAPTURE = Join-Path $data "shots\07b-deck-builder.png"
        $env:ROSES_EDITOR_CAPTURE_AT = "900"
        if ($Frames) {
            $env:ROSES_VIDEO_FRAMES = Join-Path $data "frames"
            $env:ROSES_VIDEO_FRAME_EVERY_S = "$FrameEveryS"
        }
    }
    # The title's entries: New Game, Continue, Custom Duel, Trade, Online Play.
    $gameArgs = @("--game", "`"$Game`"", "--start-stage", "main-menu", "--replay", "down,down,down,down,cross")
    $windowed = $recordThis -and $Window
    # The recorder (--record) draws off screen itself: no --capture with it.
    if (-not $windowed -and -not ($recordThis -and $Record)) {
        $gameArgs += @("--capture-game-scene", "online", "--capture", "`"$(Join-Path $data 'last-frame.png')`"")
    }
    if ($recordThis -and $Record) {
        # Online: --record-realtime (the game clock stays 1/50 s per recorded
        # frame and the frames are paced to the wall clock, so the server's
        # turn clock and the other seat see real time). Ends with the menu
        # script's exit, or cleanly with the stop file (never kill it).
        # 50 fps: the game updates at 25 Hz (two PAL fields), so every update
        # is exactly two pictures; at 60 fps they alternate 2 and 3 (judder).
        $rec = if ($RecordArgs) { $RecordArgs } else { "--record `"$(Join-Path $Out 'player1.mp4')`" --record-realtime --record-fps 50 --record-size 1920x1080" }
        $gameArgs += $rec
        $env:ROSES_RECORD_STOP_FILE = $stopFile
        $env:ROSES_RECORD_WAV = Join-Path $Out "player1.wav"
    }
    $p = Start-Process -FilePath $Bevy -PassThru -WorkingDirectory (Split-Path -Parent $Bevy) -ArgumentList $gameArgs `
        -RedirectStandardOutput (Join-Path $data "game.out") -RedirectStandardError (Join-Path $data "game.err")
    $null = $p.Handle
    Note "$name started (pid $($p.Id)): $($gameArgs -join ' ')"
    Clear-Vars
    return $p
}

$started = @()
try {
    $seed2 = $Seed -bxor 0xFFFF
    $p1 = Start-Player "player1" $data1 $keys1 $Seed $Ai1 $true
    $started += $p1
    Start-Sleep -Seconds 5
    $p2 = Start-Player "player2" $data2 $keys2 $seed2 $Ai2 $false
    $started += $p2
    $deadline = (Get-Date).AddMinutes($TimeoutMin)
    while ((Get-Date) -lt $deadline -and -not ($p1.HasExited -and $p2.HasExited)) { Start-Sleep -Seconds 5 }
    if (-not $p1.HasExited -and $Record) {
        # Out of time: let the recorder close its file before anything stops.
        Note "time is up: asking the recorder to finish"
        Set-Content -Encoding ascii $stopFile "stop"
        $null = $p1.WaitForExit(90000)
    }
    Note "player 1 exited: $($p1.HasExited); player 2 exited: $($p2.HasExited)"
}
finally {
    foreach ($q in $started) {
        if ($q -and -not $q.HasExited) { Note "stopping pid $($q.Id)"; Stop-Process -Id $q.Id -Force -ErrorAction SilentlyContinue }
    }
    Clear-Vars
}
foreach ($d in $data1, $data2) {
    "--- $d custom-decks.json"; Get-Content (Join-Path $d "custom-decks.json") -ErrorAction SilentlyContinue
    "--- $d script steps"; Select-String -Path (Join-Path $d "game.err") -Pattern "Online script: .* done|Online: |Video bot" -ErrorAction SilentlyContinue |
        Select-Object -Last 25 | ForEach-Object { $_.Line }
}
