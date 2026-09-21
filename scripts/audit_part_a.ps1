$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

$Python = Get-Command py -ErrorAction SilentlyContinue
if ($null -ne $Python) {
    & py -3 scripts/audit_part_a.py --profile windows @args
} else {
    & python scripts/audit_part_a.py --profile windows @args
}
exit $LASTEXITCODE
