# Screenshot of one process's main window with PrintWindow (works while the
# window is covered), optionally after clicking one of its buttons by caption
# (BM_CLICK, like first-run.ps1).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release\capture-window.ps1 -ProcessId <pid> -Out <file.png> [-Click 'Settings'] [-Wait 1]
param(
    [Parameter(Mandatory)] [int]$ProcessId,
    [Parameter(Mandatory)] [string]$Out,
    [string]$Click = '',
    [double]$Wait = 0.5
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('RosesCapture' -as [type])) {
    Add-Type -ReferencedAssemblies System.Drawing @"
using System; using System.Drawing; using System.Runtime.InteropServices;
public static class RosesCapture {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public static Bitmap Capture(IntPtr h) {
    RECT r; GetWindowRect(h, out r);
    var bmp = new Bitmap(r.R - r.L, r.B - r.T);
    using (var g = Graphics.FromImage(bmp)) {
      IntPtr dc = g.GetHdc();
      PrintWindow(h, dc, 2); // PW_RENDERFULLCONTENT
      g.ReleaseHdc(dc);
    }
    return bmp;
  }
}
"@
}
[RosesCapture]::SetProcessDPIAware() | Out-Null
$process = Get-Process -Id $ProcessId
$main = [IntPtr]::Zero
for ($i = 0; $i -lt 100 -and $main -eq [IntPtr]::Zero; $i++) {
    $process.Refresh(); $main = $process.MainWindowHandle
    if ($main -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 100 }
}
if ($main -eq [IntPtr]::Zero) { throw 'window not found' }
if ($Click) {
    $button = [RosesCapture]::FindWindowExW($main, [IntPtr]::Zero, 'Button', $Click)
    if ($button -eq [IntPtr]::Zero) { throw "no button '$Click'" }
    [RosesCapture]::PostMessageW($button, 0xF5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
}
Start-Sleep -Milliseconds ([int]($Wait * 1000))
New-Item -ItemType Directory -Force (Split-Path -Parent $Out) | Out-Null
$bitmap = [RosesCapture]::Capture($main)
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()
"Saved $Out"
