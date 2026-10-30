$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    cargo test --workspace --offline
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    cargo build --workspace --release --offline
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $exe = Join-Path $projectRoot 'target\release\roses.exe'
    $reference = 'examples\comparison\reference.synthetic.json'
    $equal = & $exe compare --reference $reference --candidate $reference | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $equal.status -ne 'equal' -or $equal.evidence -ne 'synthetic') {
        throw 'Synthetic equality must be labeled synthetic.'
    }
    $different = & $exe compare --reference $reference --candidate 'examples\comparison\different.synthetic.json' | ConvertFrom-Json
    if ($LASTEXITCODE -ne 1 -or $different.first_difference.path -ne '/players/0/life_points' -or $different.first_difference.checkpoint_index -ne 1) {
        throw 'The comparator did not identify the intentional first difference.'
    }
    $incomplete = & $exe compare --reference $reference --candidate 'examples\comparison\incomplete.synthetic.json' | ConvertFrom-Json
    if ($LASTEXITCODE -ne 3 -or $incomplete.status -ne 'incomplete') {
        throw 'Unknown observations must not pass.'
    }
    Write-Output 'Build, unit tests, and CLI comparisons passed. Synthetic tests do not establish game parity.'
}
finally {
    Pop-Location
}
