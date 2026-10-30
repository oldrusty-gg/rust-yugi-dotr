# First-run check of a friends package on a "clean machine": unzips the
# package into a fresh folder, starts its launcher with that folder's own
# (empty) roses-data, presses Import ISO and then Play through the real
# window's buttons, and waits for the game log to reach the title.
# The game is muted (ROSES_MUTE=1 is inherited) and closed afterwards.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release\first-run.ps1 -Zip <roses-DATE.zip> -Iso <your.iso> -Work <new folder>
param(
    [Parameter(Mandatory)] [string]$Zip,
    [Parameter(Mandatory)] [string]$Iso,
    [Parameter(Mandatory)] [string]$Work,
    [int]$ImportTimeoutMin = 40,
    [int]$TitleTimeoutSec = 240
)
$ErrorActionPreference = 'Stop'
$env:ROSES_MUTE = '1'
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class RosesWin {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
}
"@
if (Test-Path $Work) { throw "$Work already exists; give a new folder" }
New-Item -ItemType Directory $Work | Out-Null
Expand-Archive -Path $Zip -DestinationPath $Work
$package = Get-ChildItem $Work -Directory | Select-Object -First 1
$launcher = Join-Path $package.FullName 'roses-launcher.exe'
$data = Join-Path $package.FullName 'roses-data'
if (Test-Path $data) { throw 'package already has roses-data' }

$process = Start-Process -FilePath $launcher -ArgumentList @('--iso', "`"$Iso`"") -PassThru
$main = [IntPtr]::Zero
for ($i = 0; $i -lt 100 -and $main -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 200
    # This launcher's own window (not another launcher that may be open).
    $process.Refresh()
    $main = $process.MainWindowHandle
}
if ($main -eq [IntPtr]::Zero) { throw 'launcher window not found' }
$import = [RosesWin]::FindWindowExW($main, [IntPtr]::Zero, 'Button', 'Import ISO')
$play = [RosesWin]::FindWindowExW($main, [IntPtr]::Zero, 'Button', 'Play')
for ($i = 0; $i -lt 100 -and -not [RosesWin]::IsWindowEnabled($import); $i++) { Start-Sleep -Milliseconds 200 }
$BM_CLICK = 0xF5
$started = Get-Date
[RosesWin]::PostMessageW($import, $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Seconds 2
$deadline = $started.AddMinutes($ImportTimeoutMin)
while (-not [RosesWin]::IsWindowEnabled($import)) {
    if ((Get-Date) -gt $deadline) { throw 'import timed out' }
    Start-Sleep -Seconds 2
}
$importSeconds = [math]::Round(((Get-Date) - $started).TotalSeconds)
$launcherLog = Join-Path $data 'logs\launcher.log'
$game = Get-ChildItem $data -Directory -Filter 'game-*' | Where-Object { Test-Path (Join-Path $_.FullName 'game.json') }
"Import: $importSeconds s; game import: $($game.FullName)"
if (-not $game) { Get-Content $launcherLog -Tail 40; throw 'import did not complete' }

[RosesWin]::PostMessageW($play, $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
$gameLog = Join-Path $data 'logs\game.log'
$deadline = (Get-Date).AddSeconds($TitleTimeoutSec)
$reached = $false
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 3
    if ((Test-Path $gameLog) -and (Select-String -Path $gameLog -Pattern 'Movie: ended after frame' -Quiet)) { $reached = $true; break }
    if ((Test-Path $launcherLog) -and (Select-String -Path $launcherLog -Pattern 'The game stopped|The game closed' -Quiet)) { break }
}
Start-Sleep -Seconds 5
$gameProcess = Get-CimInstance Win32_Process -Filter "Name = 'roses-bevy-scene.exe'" | Where-Object { $_.ParentProcessId -eq $process.Id }
foreach ($g in $gameProcess) { Stop-Process -Id $g.ProcessId }
Start-Sleep -Seconds 2
Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
"Title reached after the intro movie: $reached"
"--- launcher.log"; Get-Content $launcherLog
"--- game.log (filtered)"; Get-Content $gameLog | Select-String -Pattern 'Movie|Game|stopped|error|panick|AdapterInfo|WARN' | Select-Object -First 40
