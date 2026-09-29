param(
    [string]$OutputDirectory = "target/packages/windows"
)

$ErrorActionPreference = "Stop"
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Set-Location $RepositoryRoot
$Version = (& python3 scripts/read_version.py).Trim()
if (-not $Version) { throw "Unable to read product version" }
$VersionComponents = @(($Version -split '-', 2)[0] -split '\.')
if ($VersionComponents.Count -gt 4 -or $VersionComponents.Count -lt 1) {
    throw "Product version cannot be represented as a Windows file version: $Version"
}
foreach ($Component in $VersionComponents) {
    if ($Component -notmatch '^\d+$') {
        throw "Product version cannot be represented as a Windows file version: $Version"
    }
}
while ($VersionComponents.Count -lt 4) { $VersionComponents += '0' }
$VersionInfoVersion = $VersionComponents -join '.'

& cargo build --release --locked --package instplot-studio --bin instplot-studio
if ($LASTEXITCODE -ne 0) { throw "Release build failed" }

$OutputRoot = Join-Path $RepositoryRoot $OutputDirectory
$SourceRoot = Join-Path $OutputRoot "source"
if (Test-Path $OutputRoot) { Remove-Item -Recurse -Force $OutputRoot }
New-Item -ItemType Directory -Force -Path $SourceRoot | Out-Null
Copy-Item target/release/instplot-studio.exe $SourceRoot
Copy-Item LICENSE $SourceRoot

$Iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if (-not $Iscc) {
    $Candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    )
    $IsccPath = $Candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $IsccPath) { throw "Inno Setup 6 ISCC.exe is required" }
} else {
    $IsccPath = $Iscc.Source
}

$Iss = Join-Path $RepositoryRoot "packaging/windows/InstPlotStudio.iss"
& $IsccPath "/DMyVersion=$Version" "/DMyVersionInfoVersion=$VersionInfoVersion" "/DSourceDir=$SourceRoot" "/DOutputDir=$OutputRoot" $Iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup build failed" }

$Installer = Join-Path $OutputRoot "InstPlot-Studio-$Version-windows-x86_64-setup.exe"
if (-not (Test-Path $Installer)) { throw "Installer was not created: $Installer" }
Write-Output $Installer
