<#
.SYNOPSIS
Try the online duel on this PC: a local server and two game windows (or one
window and a computer player, or a window that joins a given server).

.DESCRIPTION
The original's title has no online entry, so this script is the way in. It
starts `roses-server` on a loopback port with the 2-player Custom Duel
switches (Leader Ability, Card Display) and opens the duel windows. Each
window submits a deck from your memory-card saves (MEMORY CARD slot 1 for
1P, slot 2 for 2P: the deck record your save has selected, or -Deck1 /
-Deck2 0..2); without a save it uses the import's first-duel deck (the
starter deck for 1P, Weevil's for 2P).

Both windows read the keyboard: click the window whose turn it is. Keys:
arrows = d-pad, X Cross, A Square, Z Circle, D Triangle, S Start, Tab
Select, Q L1, E R1, 1 L2, W R2 (with Card Display HIDE, hold W to see your
own hidden cards, as R2 does on the original's shared screen). A window
closes by itself after the 1P WIN / 2P WIN / DRAW result (press X), or
with Escape. Everything this script starts is stopped when it returns.

Everything stays on this computer (127.0.0.1); no port is opened to the
network.

.EXAMPLE
.\scripts\play-online.ps1                       # server + two windows (1P and 2P)
.\scripts\play-online.ps1 -Mode bot             # you (1P) against a computer player that plays through its pad
.\scripts\play-online.ps1 -Mode host            # server + your window; a second player runs -Mode join
.\scripts\play-online.ps1 -Mode join -Address 127.0.0.1:47979
.\scripts\play-online.ps1 -CardDisplay hide -LeaderAbility not-use
#>
param(
    [ValidateSet('two-windows', 'bot', 'host', 'join')]
    [string]$Mode = 'two-windows',
    [string]$Address = '127.0.0.1:47979',
    # The original Custom Duel's defaults: USE and OPEN. "a,b" sets each seat.
    [string]$LeaderAbility = 'use',
    [string]$CardDisplay = 'open',
    # Memory-card slot folders (default: <game imports folder>\memory-card\slot1 / slot2).
    [string]$Save1 = '',
    [string]$Save2 = '',
    [int]$Deck1 = -1,
    [int]$Deck2 = -1,
    # The game import (default: the newest complete one under work\play\roses-data).
    [string]$Game = '',
    # Where roses-bevy-scene.exe and roses-server.exe are (a copy, so no
    # running build is locked).
    [string]$BinDir = 'work\online-bin',
    [int]$Seed = 0
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Push-Location $root
$started = @()
try {
    $bevy = Join-Path $BinDir 'roses-bevy-scene.exe'
    $serverExe = Join-Path $BinDir 'roses-server.exe'
    foreach ($exe in @($bevy, $serverExe)) {
        if (-not (Test-Path $exe)) { throw "$exe is missing (build roses-server and roses-bevy-scene --features renderer, copy them to $BinDir or pass -BinDir)" }
    }
    if (-not $Game) {
        $g = Get-ChildItem 'work\play\roses-data' -Directory -Filter 'game-*' |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'game.json') } |
            Sort-Object Name | Select-Object -Last 1
        if (-not $g) { throw 'No completed game import under work\play\roses-data (use the launcher first).' }
        $Game = Join-Path $g.FullName 'game.json'
    }
    $import = Split-Path -Parent $Game
    $cards = Split-Path -Parent $import
    if (-not $Save1) { $Save1 = Join-Path $cards 'memory-card\slot1' }
    if (-not $Save2) { $Save2 = Join-Path $cards 'memory-card\slot2' }
    if ($Address -notmatch '^(127\.\d+\.\d+\.\d+|\[::1\]):\d+$') { throw "-Address must be a loopback address (127.0.0.1:port)" }

    function Deck-Args([string]$save, [int]$deck, [int]$side) {
        if (Test-Path $save) {
            $a = @('--deck-save', $save)
            if ($deck -ge 0) { $a += @('--deck', "$deck") }
            Write-Host "  $($side + 1)P deck: your save $save$(if ($deck -ge 0) { " (deck $deck)" })"
            return $a
        }
        Write-Host "  $($side + 1)P deck: no save at $save; the import's first-duel deck (side $side)"
        return @('--import-side', "$side")
    }

    function Start-Window([int]$seat, [string[]]$deckArgs, [string]$extra = '') {
        $a = @('--game', $Game, '--online', $Address) + $deckArgs
        if ($seat -ge 0) { $a += @('--seat', "$seat") }
        if ($extra) { $a += $extra.Split(' ') }
        return Start-Process -FilePath $bevy -ArgumentList $a -PassThru
    }

    if ($Mode -ne 'join') {
        $serverArgs = @('--duel-import', $import, '--addr', $Address,
            '--leader-ability', $LeaderAbility, '--card-display', $CardDisplay)
        if ($Seed -ne 0) { $serverArgs += @('--seed', "$Seed") }
        # Playing on one PC (two windows, or against the computer player):
        # no turn clock, as the original has none.
        if ($Mode -in @('two-windows', 'bot')) { $serverArgs += @('--turn-limit-ms', '0') }
        Write-Host "Starting the server on $Address (Leader Ability $LeaderAbility, Card Display $CardDisplay)"
        $server = Start-Process -FilePath $serverExe -ArgumentList $serverArgs -PassThru -NoNewWindow
        $started += $server
        Start-Sleep -Milliseconds 800
        if ($server.HasExited) { throw "the server stopped at once (is $Address in use?)" }
    }
    $windows = @()
    switch ($Mode) {
        'two-windows' {
            $windows += Start-Window 0 (Deck-Args $Save1 $Deck1 0)
            Start-Sleep -Milliseconds 500
            $windows += Start-Window 1 (Deck-Args $Save2 $Deck2 1)
        }
        'bot' {
            $windows += Start-Window 0 (Deck-Args $Save1 $Deck1 0)
            Start-Sleep -Milliseconds 500
            $botArgs = @('--game', $Game, '--online', $Address, '--seat', '1', '--online-bot', '--online-until', 'end', '--validate-only') + (Deck-Args $Save2 $Deck2 1)
            Write-Host '  2P: the computer player (plays through its pad; its presses show in your window)'
            $bot = Start-Process -FilePath $bevy -ArgumentList $botArgs -PassThru -WindowStyle Hidden `
                -RedirectStandardOutput (Join-Path $env:TEMP 'roses-online-bot.log') -RedirectStandardError (Join-Path $env:TEMP 'roses-online-bot.err')
            $started += $bot
        }
        'host' {
            $windows += Start-Window 0 (Deck-Args $Save1 $Deck1 0)
            Write-Host "The second player runs: .\scripts\play-online.ps1 -Mode join -Address $Address"
        }
        'join' {
            $windows += Start-Window -1 (Deck-Args $Save2 $Deck2 1)
        }
    }
    $started += $windows
    Write-Host 'Waiting for the window(s) to close...'
    foreach ($w in $windows) { $w.WaitForExit() }
}
finally {
    foreach ($p in $started) {
        if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue }
    }
    Pop-Location
}
