# Try friends online on this PC (loopback only): makes a self-signed
# certificate, an account database and an online.json for the game, then
# runs the hub (open sign-up, as the friends beta; -InviteOnly makes an
# invite code and requires it) until you close this window.
#
#   powershell -File scripts/friends-local.ps1 [-Game <roses-data\game-...>] [-Dir <folder>]
#              [-Port 7979] [-Computer] [-BinDir <folder with roses-server.exe>]
#
# Then start the game with ROSES_ONLINE_CONFIG pointing at <Dir>\online.json
# (or copy that file next to memory-card\ in the game data folder), choose
# Online Play on the title, Connect, Sign Up (or sign up on the launcher's
# Profile page with --data-root pointing at that folder).
# Two game windows on one PC can play each other (use two names).
param(
    [string]$Game = "",
    [string]$Dir = "work\friends-local",
    [int]$Port = 7979,
    [switch]$Computer,
    [switch]$InviteOnly,
    [string]$BinDir = "work\online-bin"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not $Game) {
    $Game = Get-ChildItem "work\play\roses-data" -Directory -Filter "game-*" | Sort-Object Name | Select-Object -Last 1 -ExpandProperty FullName
}
$server = Join-Path $BinDir "roses-server.exe"
if (-not (Test-Path $server)) { throw "No $($server): build it (cargo build -p roses-server) and copy roses-server.exe there, or pass -BinDir" }
New-Item -ItemType Directory -Force $Dir | Out-Null
$db = Join-Path $Dir "accounts.db"
$certDir = Join-Path $Dir "cert"
if (-not (Test-Path (Join-Path $certDir "cert.pem"))) {
    & $server tls self-signed --out $certDir --name 127.0.0.1 --name localhost | Out-Null
}
$pin = & $server tls fingerprint (Join-Path $certDir "cert.pem")
$config = @{ server = "127.0.0.1:$Port"; name = "Local test server"; pin = "$pin" } | ConvertTo-Json
Set-Content -Encoding ascii -Path (Join-Path $Dir "online.json") -Value $config
if ($InviteOnly) {
    $invite = & $server accounts --db $db invite --uses 4 --note "local test"
    Write-Host "Invite code: $invite"
}
Write-Host "online.json: $(Resolve-Path (Join-Path $Dir 'online.json'))"
Write-Host "In the game: set ROSES_ONLINE_CONFIG to that online.json, then Title -> Online Play."
$hubArgs = @("hub", "--duel-import", (Join-Path $Game "duel"), "--db", $db,
    "--listen", "127.0.0.1:$Port", "--tls-cert", (Join-Path $certDir "cert.pem"), "--tls-key", (Join-Path $certDir "key.pem"),
    "--name", "Local test server", "--replay-dir", (Join-Path $Dir "replays"))
if ($Computer) { $hubArgs += "--computer" }
if (-not $InviteOnly) { $hubArgs += "--open-signup" }
& $server @hubArgs
