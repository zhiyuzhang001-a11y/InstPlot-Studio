param(
    [string]$OutputDirectory = "target/packages/windows",
    [switch]$InPlaceUpdatePreview
)

$ErrorActionPreference = "Stop"
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Set-Location $RepositoryRoot
$Version = (& python3 scripts/read_version.py).Trim()
if (-not $Version) { throw "Unable to read product version" }
if ($InPlaceUpdatePreview -and $Version -notmatch '-') {
    throw "In-place update technical preview requires a prerelease version"
}
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

$BuildArguments = @("build", "--release", "--locked", "--package", "instplot-studio", "--bin", "instplot-studio")
if ($InPlaceUpdatePreview) { $BuildArguments += @("--features", "in-place-update-preview") }
& cargo @BuildArguments
if ($LASTEXITCODE -ne 0) { throw "Release build failed" }
& ./scripts/verify_windows_icon.ps1 -Executable target/release/instplot-studio.exe

$OutputRoot = Join-Path $RepositoryRoot $OutputDirectory
$SourceRoot = Join-Path $OutputRoot "source"
if (Test-Path $OutputRoot) { Remove-Item -Recurse -Force $OutputRoot }
New-Item -ItemType Directory -Force -Path $SourceRoot | Out-Null
Copy-Item target/release/instplot-studio.exe $SourceRoot
Copy-Item LICENSE $SourceRoot
Copy-Item apps/instplot-studio/assets/InstPlotStudio.ico $SourceRoot

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
if ($InPlaceUpdatePreview) {
    # Probe the exact packaged binary, not a separately compiled executable.
    $Binary = Join-Path $SourceRoot "instplot-studio.exe"
    . ./scripts/invoke_studio_probe.ps1
    $Probe = (Invoke-StudioProbe $Binary @('--windows-update-capabilities')) | ConvertFrom-Json
    if ($Probe.product -ne "instplot-studio" -or $Probe.version -ne $Version -or
        $Probe.platform -ne "windows-x86_64" -or
        $Probe.scope -ne "preview-components-not-accepted-updater" -or
        $Probe.public_update_protocol -ne 0 -or
        $Probe.gui_acceptance_complete -ne $false -or
        $Probe.public_apply_entry_enabled -ne $false -or
        $Probe.build_profile -ne 'release' -or
        $Probe.startup_update_check_enabled -ne $true) { throw "Unexpected preview capability scope" }
    $Contract = [ordered]@{ schema = 1 }
    foreach ($Name in @("helper_protocol", "transaction_schema", "candidate_health_protocol", "recovery_health_protocol")) {
        if ($Probe.components.$Name -ne 1) { throw "Unsupported preview protocol: $Name" }
        $Contract[$Name] = 1
    }
    $Contract["executable_sha256"] = (Get-FileHash -Algorithm SHA256 $Binary).Hash.ToLowerInvariant()
    $Contract["license_sha256"] = (Get-FileHash -Algorithm SHA256 (Join-Path $SourceRoot "LICENSE")).Hash.ToLowerInvariant()
    $Evidence = [ordered]@{
        scope = "preview-components-not-accepted-updater"
        version = $Version
        installer_sha256 = (Get-FileHash -Algorithm SHA256 $Installer).Hash.ToLowerInvariant()
        contract = $Contract
    }
    $Evidence | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM (Join-Path $OutputRoot "windows-in-place.json")
}
Write-Output $Installer
