# The MODES videos (video aid; MODES is a port addition, docs/ADDITIONS.md):
# four game clients against one server in one N-player room (2v2 or a
# 4-player free-for-all), each seat driven through its pad at a person's
# pace by one of the game's own computer opponents (video_bot.rs plans on
# the server computers' 2-side projection), from the title through Online
# Play to the end of the match:
#
#   player 1 (host): title -> Online Play -> CONNECT -> LOG IN -> LOBBY -> MODES ->
#             MODE (2v2) / START LP / TURN TIMER: NO CLOCK / BOARD (the map) ->
#             CREATE ROOM -> CHOOSE DECK -> ROOM (its code goes to player 2)
#             -> (its seat) -> all four seated -> START -> the match -> RESULT
#   players 2-4: title -> Online Play -> CONNECT -> LOG IN -> MODES -> JOIN ROOM ->
#             CHOOSE DECK -> the room code -> ROOM -> (its seat) -> the match -> RESULT
#
# The joins are one at a time: player k+1 gets the room code once player k
# sits at its seat (its shot 07-seated.png), so every player takes the seat
# given (2v2: S + N against E + W). Player 1's view is recorded (-Record)
# or written as key frames (-Frames). Every process this starts is stopped
# before the script returns; the game is muted unless -Sound is given.
#
# The same seed gives the same match only on a server that uses it for the
# room's match (`roses-server hub ... --match-seed <seed>`, which MODES
# rooms use too; scripts\video\rehearse-local.ps1 -Mode 2v2|ffa). The room
# has no turn clock (TURN TIMER: NO CLOCK), so the hub's --turn-limit-ms
# does not matter here.
#
# NOT FOR THE LIVE SERVER YET: anything but a loopback address is refused
# unless -LiveServerApproved is given (the lead passes it once Jim agrees).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\video\run-modes-video.ps1 `
#       -Server 127.0.0.1:47992 -NoTls -SignUp -Mode ffa `
#       -Accounts JimTest,mcduck3000,Jipps,Duelist4 -Passwords p1,p2,p3,p4 `
#       -Decks "Jasper Dice Tudor",Seto,Yugi,Joey -Ais 17,1,19,15 [-Seats S,N,E,W] `
#       -Seed 7 -Map "VS MAP 13" [-StartLp 4000] [-ExodiaPlayer 1] -Out work\video\take-ffa [-Record] [-Frames]
param(
    [Parameter(Mandatory = $true)][string]$Server,
    [Parameter(Mandatory = $true)][string[]]$Accounts,
    [Parameter(Mandatory = $true)][string[]]$Passwords,
    [Parameter(Mandatory = $true)][ValidateSet("2v2", "ffa")][string]$Mode,
    [Parameter(Mandatory = $true)][string[]]$Decks,
    [Parameter(Mandatory = $true)][string[]]$Ais,
    [string[]]$Seats = @("S", "N", "E", "W"),
    [Parameter(Mandatory = $true)][string]$Out,
    [long]$Seed = 1,
    [string]$Map = "VS MAP 13",
    [ValidateSet(1000, 2000, 4000, 6000, 8000, 9999)][int]$StartLp = 4000,
    [ValidateSet("human", "off")][string]$Pace = "human",
    [int]$Tier = 8,
    # The player (1-4) whose bot uses the Exodia aid (video_bot.rs
    # exodia_assist, ROSES_BOT_EXODIA=1; 0: none).
    [int]$ExodiaPlayer = 0,
    [string]$Game = "work\runs\claude-video\game\game.json",
    [string]$Bevy = "work\runs\claude-video\bin-modes3\roses-bevy-scene.exe",
    [string]$Save = "work\runs\claude-2p-game\save\memory-card\slot1",
    [string]$Pin = "",
    [switch]$NoTls,
    [switch]$SignUp,
    [switch]$Record,
    [string]$RecordArgs = "",
    [switch]$Frames,
    [int]$FrameEveryS = 20,
    [switch]$Sound,
    [switch]$LiveServerApproved,
    [int]$TimeoutMin = 120
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root
$n = 4
# `powershell -File` passes "a,b,c,d" as one string: split it.
function Split-List($list) { return , @($list | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' }) }
$Accounts = Split-List $Accounts; $Passwords = Split-List $Passwords; $Decks = Split-List $Decks
$Ais = Split-List $Ais; $Seats = Split-List $Seats
foreach ($list in @($Accounts, $Passwords, $Decks, $Ais, $Seats)) {
    if ($list.Count -ne $n) { throw "-Accounts, -Passwords, -Decks, -Ais and -Seats need four entries each" }
}
$letters = @("S", "N", "E", "W")
$seatOf = @($Seats | ForEach-Object { $i = [array]::IndexOf($letters, $_.ToUpper()); if ($i -lt 0) { throw "seat $_ is not S, N, E or W" }; $i })
if (($seatOf | Sort-Object -Unique).Count -ne $n) { throw "-Seats must name four different seats" }

# --- safety: loopback only until the live take is approved ---------------
$hostPart = ($Server -replace ':\d+$', '').Trim('[', ']')
$loopback = $hostPart -in @("127.0.0.1", "localhost", "::1")
if (-not $loopback -and -not $LiveServerApproved) {
    throw "$Server is not this computer: the live take needs -LiveServerApproved (Jim's OK first)"
}
if ($NoTls -and -not $loopback) { throw "-NoTls is for a loopback server only" }
if (-not $Sound) { $env:ROSES_MUTE = "1" }

if (Test-Path $Out) { throw "$Out exists; give a new folder" }
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Game = (Resolve-Path $Game).Path
$Bevy = (Resolve-Path $Bevy).Path
$stopFile = Join-Path $Out "stop-recording"
$log = Join-Path $Out "driver.log"
function Note($text) { $line = "$(Get-Date -Format HH:mm:ss) $text"; Write-Host $line; Add-Content $log $line }
Note "server $Server, mode $Mode, seed $Seed, pace $Pace, map $Map, start LP $StartLp"
for ($k = 0; $k -lt $n; $k++) { Note "player $($k + 1): $($Accounts[$k]) at $($letters[$seatOf[$k]]), deck $($Decks[$k]), module $($Ais[$k])" }

function Q($text) { return '"' + $text + '"' }
function Sel($label) { return "select:" + ($label -replace ' ', '_') }
# Cross on that row, checked at the press (online_render.rs `ok:`).
function Ok($label) { return "ok:" + ($label -replace ' ', '_') }
function SignIn($account, $password) {
    if ($SignUp) {
        return "until:SERVER,shot:01-server,wait*20,select:CONNECT,ok:CONNECT,until:ACCOUNT,shot:02-account,select:SIGN_UP,ok:SIGN_UP,until:SIGN_UP," +
            (Q $account) + ",x," + (Q $password) + ",x,select:SIGN_UP,ok:SIGN_UP,until:LOBBY"
    }
    return "until:SERVER,shot:01-server,wait*20,select:CONNECT,ok:CONNECT,until:ACCOUNT,shot:02-account,select:LOG_IN,ok:LOG_IN,until:LOG_IN," +
        (Q $account) + ",x," + (Q $password) + ",x,select:LOG_IN,ok:LOG_IN,until:LOBBY"
}

# Where each player lands when it joins (the first free seat), and the
# move to its own seat (Cross on that seat's row).
$taken = @()
$moves = @()
for ($k = 0; $k -lt $n; $k++) {
    $land = if ($k -eq 0) { 0 } else { (0..3 | Where-Object { $taken -notcontains $_ } | Select-Object -First 1) }
    $want = $seatOf[$k]
    if ($taken -contains $want) { throw "player $($k + 1)'s seat $($letters[$want]) is taken by then" }
    $moves += $(if ($land -ne $want) { "$(Sel $letters[$want]),wait*10,$(Ok $letters[$want]),wait*25," } else { "wait*25," })
    $taken += $want
}

# --- the menu scripts (online_render.rs: ROSES_ONLINE_KEYS) ---------------
$lpSteps = [array]::IndexOf(@(1000, 2000, 4000, 6000, 8000, 9999), $StartLp) - 2
$lpKeys = if ($lpSteps -gt 0) { (@("right") * $lpSteps) -join "," } elseif ($lpSteps -lt 0) { (@("left") * (-$lpSteps)) -join "," } else { "" }
$setup = "select:MODES,ok:MODES,until:MODES,shot:03b-modes,wait*20,"
if ($Mode -eq "2v2") { $setup += "select:MODE,wait*10,right,wait*20," }
if ($lpKeys) { $setup += "select:START_LP,wait*10,$lpKeys,wait*15," }
# 90 S -> 120 S -> 180 S -> NO CLOCK.
$setup += "select:TURN_TIMER,wait*10,right,right,right,wait*20,shot:03c-settings,"
$setup += "select:BOARD,ok:BOARD,until:CHOOSE_MAP,$(Sel $Map),wait*40,shot:03d-map,$(Ok $Map),until:MODES,wait*20,"
$keys = @()
$keys += (SignIn $Accounts[0] $Passwords[0]) + ",shot:03-lobby,wait*25," + $setup +
    "select:CREATE_ROOM,ok:CREATE_ROOM,until:CHOOSE_DECK,$(Sel $Decks[0]),shot:04-choose-deck,wait*15,$(Ok $Decks[0])," +
    "until:ROOM,wait*10,roomout,shot:05-room," + $moves[0] + "shot:07-seated," +
    "seated:4,wait*40,shot:08-room-full,select:START,wait*10,ok:START," +
    "until:RESULT,shot:13-result,wait*75,select:OK,ok:OK,until:LOBBY,shot:14-lobby-after,wait*50,exit"
for ($k = 1; $k -lt $n; $k++) {
    $keys += (SignIn $Accounts[$k] $Passwords[$k]) + ",wait*25," +
        "select:MODES,ok:MODES,until:MODES,wait*15,select:JOIN_ROOM,ok:JOIN_ROOM,until:CHOOSE_DECK,$(Sel $Decks[$k]),wait*15,shot:04-choose-deck,$(Ok $Decks[$k])," +
        "until:JOIN_ROOM,roomin,shot:05-room-code,x,x,until:ROOM,wait*10," + $moves[$k] + "shot:07-seated," +
        "until:RESULT,shot:13-result,wait*75,select:OK,ok:OK,until:LOBBY,wait*50,exit"
}
for ($k = 0; $k -lt $n; $k++) { Set-Content -Encoding ascii (Join-Path $Out "keys-player$($k + 1).txt") $keys[$k] }

# --- one data folder per player -------------------------------------------
function New-PlayerData($name) {
    $data = Join-Path $Out $name
    New-Item -ItemType Directory -Force (Join-Path $data "memory-card\slot1"), (Join-Path $data "shots") | Out-Null
    if (Test-Path $Save) { Copy-Item (Join-Path $Save "*") (Join-Path $data "memory-card\slot1") -Recurse }
    $cfg = [ordered]@{ server = $Server; name = "Roses Online" }
    if ($NoTls) { $cfg.tls = $false } elseif ($Pin) { $cfg.pin = $Pin }
    $cfg | ConvertTo-Json | Set-Content -Encoding ascii (Join-Path $data "online.json")
    return $data
}

# --- start a client --------------------------------------------------------
$vars = "ROSES_ONLINE_CONFIG", "ROSES_SAVE_DIR", "ROSES_ONLINE_KEYS", "ROSES_ONLINE_PADBOT", "ROSES_ONLINE_SHOTS",
    "ROSES_ONLINE_ROOM_FILE", "ROSES_BOT_PACE", "ROSES_BOT_SEED", "ROSES_BOT_AI", "ROSES_BOT_TIER", "ROSES_BOT_LOG",
    "ROSES_CAPTURE_EXTRA_S", "ROSES_BOT_EXODIA", "ROSES_VIDEO_FRAMES", "ROSES_VIDEO_FRAME_EVERY_S", "ROSES_RECORD_STOP_FILE", "ROSES_RECORD_WAV"
function Clear-Vars { foreach ($v in $vars) { Remove-Item "env:$v" -ErrorAction SilentlyContinue } }
function Start-Player($k, $data) {
    Clear-Vars
    $seat = $seatOf[$k]
    $env:ROSES_ONLINE_CONFIG = Join-Path $data "online.json"
    $env:ROSES_SAVE_DIR = $data
    $env:ROSES_ONLINE_KEYS = $keys[$k]
    $env:ROSES_ONLINE_PADBOT = "1"
    $env:ROSES_ONLINE_SHOTS = Join-Path $data "shots"
    $env:ROSES_ONLINE_ROOM_FILE = Join-Path $Out "room-$($k + 1).txt"
    $env:ROSES_BOT_PACE = $Pace
    # video_tests.rs multi_bot_seed: the seed ^ (seat * 0x1111).
    $env:ROSES_BOT_SEED = "$($Seed -bxor ($seat * 0x1111))"
    $env:ROSES_BOT_AI = $Ais[$k]
    $env:ROSES_BOT_TIER = "$Tier"
    $env:ROSES_BOT_LOG = Join-Path $data "bot.jsonl"
    if ($k + 1 -eq $ExodiaPlayer) { $env:ROSES_BOT_EXODIA = "1" }
    $env:ROSES_CAPTURE_EXTRA_S = "$($TimeoutMin * 60)"
    $recordThis = ($k -eq 0) -and $Record
    if ($k -eq 0 -and $Frames) {
        $env:ROSES_VIDEO_FRAMES = Join-Path $data "frames"
        $env:ROSES_VIDEO_FRAME_EVERY_S = "$FrameEveryS"
    }
    # The title's entries: New Game, Continue, Custom Duel, Trade, Online Play.
    $gameArgs = @("--game", "`"$Game`"", "--start-stage", "main-menu", "--replay", "down,down,down,down,cross")
    if ($recordThis) {
        # Real time at 50 fps (two pictures per 25 Hz update; see
        # run-online-video.ps1); ends with the script's exit or the stop file.
        $rec = if ($RecordArgs) { $RecordArgs } else { "--record `"$(Join-Path $Out 'player1.mp4')`" --record-realtime --record-fps 50 --record-size 1920x1080" }
        $gameArgs += $rec
        $env:ROSES_RECORD_STOP_FILE = $stopFile
        $env:ROSES_RECORD_WAV = Join-Path $Out "player1.wav"
    } else {
        $gameArgs += @("--capture-game-scene", "online", "--capture", "`"$(Join-Path $data 'last-frame.png')`"")
    }
    $p = Start-Process -FilePath $Bevy -PassThru -WorkingDirectory (Split-Path -Parent $Bevy) -ArgumentList $gameArgs `
        -RedirectStandardOutput (Join-Path $data "game.out") -RedirectStandardError (Join-Path $data "game.err")
    $null = $p.Handle
    Note "player $($k + 1) started (pid $($p.Id), seat $($letters[$seat]), bot seed $env:ROSES_BOT_SEED): $($gameArgs -join ' ')"
    Clear-Vars
    return $p
}

$data = @()
for ($k = 0; $k -lt $n; $k++) { $data += (New-PlayerData "player$($k + 1)") }
$started = @()
try {
    $started += (Start-Player 0 $data[0])
    for ($k = 1; $k -lt $n; $k++) {
        Start-Sleep -Seconds 4
        $started += (Start-Player $k $data[$k])
    }
    # The joins, one at a time: the code goes to player k+1 once player k
    # sits at its seat.
    $next = 1
    $deadline = (Get-Date).AddMinutes($TimeoutMin)
    while ((Get-Date) -lt $deadline -and ($started | Where-Object { -not $_.HasExited }).Count -gt 0) {
        $code = Join-Path $Out "room-1.txt"
        if ($next -lt $n -and (Test-Path $code) -and (Test-Path (Join-Path $data[$next - 1] "shots\07-seated.png"))) {
            Copy-Item $code (Join-Path $Out "room-$($next + 1).txt")
            Note "room code $((Get-Content $code -Raw).Trim()) to player $($next + 1)"
            $next += 1
        }
        if ($started[0].HasExited -and $next -lt $n) { Note "player 1 exited before the room was full"; break }
        Start-Sleep -Seconds 2
    }
    if (-not $started[0].HasExited -and $Record) {
        Note "time is up: asking the recorder to finish"
        Set-Content -Encoding ascii $stopFile "stop"
        $null = $started[0].WaitForExit(90000)
    }
    for ($k = 0; $k -lt $n; $k++) {
        $code = if ($started[$k].HasExited) { " (exit code $($started[$k].ExitCode))" } else { "" }
        Note "player $($k + 1) exited: $($started[$k].HasExited)$code"
    }
}
finally {
    foreach ($q in $started) {
        if ($q -and -not $q.HasExited) { Note "stopping pid $($q.Id)"; Stop-Process -Id $q.Id -Force -ErrorAction SilentlyContinue }
    }
    Clear-Vars
}
foreach ($d in $data) {
    "--- $d script steps"; Select-String -Path (Join-Path $d "game.err") -Pattern "Online script: .* done|Online: |Video bot" -ErrorAction SilentlyContinue |
        Select-Object -Last 12 | ForEach-Object { $_.Line }
}
