# Capture the Bevy client in an online duel (headless --capture) over
# loopback. Every process this starts is stopped before the script returns;
# sound is muted (ROSES_MUTE=1).
#
#   powershell -File scripts/online-capture.ps1 -Out <dir> [-Port 47990]
#     [-ServerBin <dir with roses-server.exe, roses-online-client.exe>]
#     [-Bevy <roses-bevy-scene.exe>] [-Game <game.json>]
#     [-Save0 <slot folder>] [-Save1 <slot folder>] [-Pad "<replay pad>"] [-Name png]
#     [-StreamingOpponent] [-Bot] [-Until remote-hand|remote-pick|remote-flight|result|end]
#     [-LeaderAbility use|not-use] [-CardDisplay hide|open]
#
# Default: seat 1 is the terminal client's computer player (it does not
# stream its presses). -StreamingOpponent makes seat 1 a headless Bevy client
# played by the pad-level bot, which streams its presses as a person's
# window does. -Bot plays the captured seat 0 by the same bot; -Until keeps
# the captured client running until that moment (the other seat's hand row
# open / its picked cards / its summon flight / the result banner / the end).
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$Port = 47990,
    [string]$ServerBin = "work/runs/claude-online3/debug",
    [string]$Bevy = "work/runs/claude-online3-bevy/debug/roses-bevy-scene.exe",
    [string]$Game = "work/play/roses-data/game-20261004-claude-v25/game.json",
    [string]$Save0 = "",
    [string]$Save1 = "",
    [string]$Pad = "square,up,cross,up,cross,cross,none*200",
    [string]$Name = "online.png",
    [int]$Seed = 7,
    [switch]$StreamingOpponent,
    [switch]$Bot,
    [string]$Until = "",
    [string]$LeaderAbility = "use",
    [string]$CardDisplay = "hide"
)
$ErrorActionPreference = "Stop"
$env:ROSES_MUTE = "1"
New-Item -ItemType Directory -Force $Out | Out-Null
$import = Split-Path -Parent $Game
$addr = "127.0.0.1:$Port"
$started = @()
try {
    $server = Start-Process -FilePath (Join-Path $ServerBin "roses-server.exe") -PassThru -NoNewWindow `
        -ArgumentList @("--duel-import", $import, "--addr", $addr, "--seed", "$Seed",
            "--leader-ability", $LeaderAbility, "--card-display", $CardDisplay) `
        -RedirectStandardOutput (Join-Path $Out "server.log") -RedirectStandardError (Join-Path $Out "server.err")
    $started += $server
    Start-Sleep -Milliseconds 700
    if ($StreamingOpponent) {
        $botArgs = @("--game", $Game, "--online", $addr, "--seat", "1", "--online-bot", "--online-until", "end", "--validate-only")
        if ($Save1) { $botArgs += @("--deck-save", $Save1) }
        $opponent = Start-Process -FilePath $Bevy -PassThru -NoNewWindow -ArgumentList $botArgs `
            -RedirectStandardOutput (Join-Path $Out "bot.log") -RedirectStandardError (Join-Path $Out "bot.err")
    }
    else {
        $botArgs = @("--duel-import", $import, "--addr", $addr, "--seat", "1")
        if ($Save1) { $botArgs += @("--deck-save", $Save1) }
        $opponent = Start-Process -FilePath (Join-Path $ServerBin "roses-online-client.exe") -PassThru -NoNewWindow `
            -ArgumentList $botArgs `
            -RedirectStandardOutput (Join-Path $Out "bot.log") -RedirectStandardError (Join-Path $Out "bot.err")
    }
    $started += $opponent
    $png = Join-Path $Out $Name
    if (Test-Path $png) { Remove-Item $png }
    $bevyArgs = @("--game", $Game, "--online", $addr, "--seat", "0", "--capture", $png)
    if ($Pad) { $bevyArgs += @("--replay-pad", $Pad) }
    if ($Bot) { $bevyArgs += "--online-bot" }
    if ($Until) { $bevyArgs += @("--online-until", $Until) }
    if ($Save0) { $bevyArgs += @("--deck-save", $Save0) }
    $client = Start-Process -FilePath $Bevy -PassThru -NoNewWindow -ArgumentList $bevyArgs `
        -RedirectStandardOutput (Join-Path $Out "bevy.log") -RedirectStandardError (Join-Path $Out "bevy.err")
    $started += $client
    if (-not $client.WaitForExit(900000)) { Write-Output "the Bevy client did not finish in 900 s" }
    Write-Output "bevy exit code: $($client.ExitCode); capture: $(Test-Path $png)"
}
finally {
    foreach ($p in $started) {
        if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue }
    }
}
