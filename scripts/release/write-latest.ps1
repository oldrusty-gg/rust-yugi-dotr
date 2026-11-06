# Writes latest.json, the small release asset the launcher's Update button
# reads: the version (it must equal the GitHub tag and the -Release the
# package was built with), the zip's asset name, its size and its SHA-256.
# build-release.ps1 calls this; upload the file with the zip, named latest.json.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release\write-latest.ps1 -Zip <roses-DATE.zip> -Release <DATE> -Out <latest.json>
param(
    [Parameter(Mandatory)] [string]$Zip,
    [Parameter(Mandatory)] [string]$Release,
    [Parameter(Mandatory)] [string]$Out
)
$ErrorActionPreference = 'Stop'
if ($Release -cnotmatch '^\d{4}-\d{2}-\d{2}[a-z]*$') { throw "-Release $Release is not YYYY-MM-DD with optional lower-case letters; the launcher could not compare it" }
$file = Get-Item -LiteralPath $Zip
$latest = [ordered]@{
    version = $Release
    zip     = $file.Name
    size    = $file.Length
    sha256  = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLower()
}
# No byte order mark: the launcher reads this as plain JSON.
[IO.File]::WriteAllText($Out, ($latest | ConvertTo-Json) + "`r`n", (New-Object Text.UTF8Encoding($false)))
Write-Host "latest.json: $Release, $($file.Name), $($latest.size) bytes, sha256 $($latest.sha256)"
