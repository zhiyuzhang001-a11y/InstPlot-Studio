param()
# Disposable CI-only installer recovery prototype. Does NOT implement the updater.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Run only on a disposable Windows GitHub Actions runner, never on a user installation.'
}
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
$MachineRegistration = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
if ((Test-Path $Registration) -or (Test-Path $MachineRegistration)) {
    throw 'Refusing to modify an existing Studio registration.'
}
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'InstPlot Studio.lnk'
$MenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'InstPlot Studio.lnk'
if ((Test-Path $DesktopShortcut) -or (Test-Path $MenuShortcut)) {
    throw 'Refusing to replace existing Studio shortcuts.'
}
$TaskRoot = Join-Path $env:RUNNER_TEMP ('instplot-recovery-prototype-' + [guid]::NewGuid())
New-Item -ItemType Directory $TaskRoot | Out-Null
$EvidenceRoot = Join-Path $RepositoryRoot 'target/windows-update-recovery'
New-Item -ItemType Directory -Force $EvidenceRoot | Out-Null
$CurrentVersion = (& python3 (Join-Path $RepositoryRoot 'scripts/read_version.py')).Trim()
if ($CurrentVersion -notmatch '^([0-9]+\.[0-9]+\.[0-9]+)-rc\.([0-9]+)$') {
    throw 'Prototype requires a prerelease baseline; does not change a stable version.'
}
$NextVersion = $Matches[1] + '-rc.' + ([int]$Matches[2] + 1)
$FileVersion = ($CurrentVersion -split '-')[0] + '.0'
$Iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
$IsccPath = if ($Iscc) { $Iscc.Source } else { "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" }
if (-not (Test-Path $IsccPath)) { throw 'Pinned Inno Setup is required.' }

function Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program exited with $LASTEXITCODE" }
}

function ProductIdentity([string]$Binary, [string]$Version) {
    $Actual = & $Binary --product-info
    if ($LASTEXITCODE -ne 0 -or $Actual -ne "InstPlot Studio`tinstplot-studio`t$Version") {
        throw "Wrong product identity: $Actual"
    }
}

function CompileInstaller([string]$Source, [string]$Version, [string]$Definition, [string]$Output) {
    New-Item -ItemType Directory -Force $Output | Out-Null
    Checked $IsccPath @("/DMyVersion=$Version", "/DMyVersionInfoVersion=$FileVersion",
        "/DSourceDir=$Source", "/DOutputDir=$Output", $Definition) | Out-Host
    return Join-Path $Output "InstPlot-Studio-$Version-windows-x86_64-setup.exe"
}

function Install([string]$Installer, [string]$Directory, [bool]$Desktop, [string]$Log, [bool]$ExpectFailure = $false) {
    $Tasks = if ($Desktop) { '/TASKS=desktopicon' } else { '/TASKS=!desktopicon' }
    $Arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-',
        '/NOCLOSEAPPLICATIONS', '/NORESTARTAPPLICATIONS', $Tasks,
        ('/DIR="' + $Directory + '"'), ('/LOG="' + $Log + '"'))
    $Process = Start-Process -FilePath $Installer -ArgumentList $Arguments -Wait -PassThru
    if ($ExpectFailure) {
        if ($Process.ExitCode -eq 0) { throw 'Fault installer unexpectedly succeeded.' }
    } elseif ($Process.ExitCode -ne 0) { throw "Installer failed: $($Process.ExitCode)" }
}

function VerifyInstallation([string]$Directory, [string]$Version, [string]$Hash, [bool]$Desktop) {
    $Binary = Join-Path $Directory 'instplot-studio.exe'
    ProductIdentity $Binary $Version
    if ((Get-FileHash -Algorithm SHA256 $Binary).Hash -ne $Hash) { throw 'Installed exe differs from prepared payload.' }
    $Record = Get-ItemProperty $Registration
    if ($Record.DisplayVersion -ne $Version -or
        $Record.InstallLocation.TrimEnd('\') -ne $Directory.TrimEnd('\') -or
        $Record.UninstallString -notlike "*$Directory*unins000.exe*") {
        throw 'Uninstall registration did not retain version/path/identity.'
    }
    if (Test-Path $MachineRegistration) { throw 'Installer changed to machine scope.' }
    if ($Desktop) {
        & (Join-Path $RepositoryRoot 'scripts/verify_windows_icon.ps1') -Executable $Binary -Shortcut $DesktopShortcut
    } elseif (Test-Path $DesktopShortcut) { throw 'Disabled desktop task was re-enabled.' }
    & (Join-Path $RepositoryRoot 'scripts/verify_windows_icon.ps1') -Executable $Binary -Shortcut $MenuShortcut
}

function RemoveAddedFile([string]$Directory, [string]$Hash) {
    # Only this fixture's recorded new release file can be removed; never user data.
    $Added = Join-Path $Directory 'update-added.bin'
    if ((Get-FileHash -Algorithm SHA256 $Added).Hash -ne $Hash) {
        throw 'New release file changed; refuse automatic deletion.'
    }
    Remove-Item -LiteralPath $Added
}

Push-Location $RepositoryRoot
try {
    # Two real Studio builds; candidate version exists ONLY in a disposable snapshot.
    Checked 'cargo' @('build', '--locked', '--package', 'instplot-studio')
    $OldSource = Join-Path $TaskRoot 'old-source'
    $NewSource = Join-Path $TaskRoot 'new-source'
    New-Item -ItemType Directory $OldSource, $NewSource | Out-Null
    Copy-Item 'target/debug/instplot-studio.exe' $OldSource
    foreach ($Source in @($OldSource, $NewSource)) {
        Copy-Item 'LICENSE' $Source
        Copy-Item 'apps/instplot-studio/assets/InstPlotStudio.ico' $Source
    }
    $Snapshot = Join-Path $TaskRoot 'snapshot'
    New-Item -ItemType Directory $Snapshot | Out-Null
    $Archive = Join-Path $TaskRoot 'snapshot.tar'
    Checked 'git' @('archive', '--format=tar', "--output=$Archive", 'HEAD')
    Checked 'tar' @('-xf', $Archive, '-C', $Snapshot)
    $Manifest = Join-Path $Snapshot 'Cargo.toml'
    $Content = [IO.File]::ReadAllText($Manifest)
    $Before = 'version = "' + $CurrentVersion + '"'
    if (([regex]::Matches($Content, [regex]::Escape($Before))).Count -ne 1) { throw 'Ambiguous workspace version.' }
    [IO.File]::WriteAllText($Manifest, $Content.Replace($Before, 'version = "' + $NextVersion + '"'))
    Checked 'cargo' @('build', '--offline', '--manifest-path', $Manifest,
        '--target-dir', (Join-Path $RepositoryRoot 'target'), '--package', 'instplot-studio')
    Copy-Item 'target/debug/instplot-studio.exe' $NewSource
    $OldBinary = Join-Path $OldSource 'instplot-studio.exe'
    $NewBinary = Join-Path $NewSource 'instplot-studio.exe'
    ProductIdentity $OldBinary $CurrentVersion
    ProductIdentity $NewBinary $NextVersion
    $OldHash = (Get-FileHash -Algorithm SHA256 $OldBinary).Hash
    $NewHash = (Get-FileHash -Algorithm SHA256 $NewBinary).Hash
    if ($OldHash -eq $NewHash) { throw 'Must test distinct real binaries.' }
    [IO.File]::WriteAllText((Join-Path $NewSource 'update-added.bin'), 'new-version-only fixture file')
    $AddedHash = (Get-FileHash -Algorithm SHA256 (Join-Path $NewSource 'update-added.bin')).Hash
    $Definition = Join-Path $RepositoryRoot 'packaging/windows/InstPlotStudio.iss'
    $NewDefinition = Join-Path $TaskRoot 'new.iss'
    [IO.File]::WriteAllText($NewDefinition, [IO.File]::ReadAllText($Definition) + "`n[Files]`n" +
        'Source: "{#SourceDir}\update-added.bin"; DestDir: "{app}"; Flags: ignoreversion' + "`n")
    $FaultDefinition = Join-Path $TaskRoot 'fault.iss'
    [IO.File]::WriteAllText($FaultDefinition, [IO.File]::ReadAllText($NewDefinition) + "`n[Code]`n" +
        "function InitializeSetup(): Boolean;`nbegin`n  Result := False;`nend;`n")
    $OldInstaller = CompileInstaller $OldSource $CurrentVersion $Definition (Join-Path $TaskRoot 'old-setup')
    $NewInstaller = CompileInstaller $NewSource $NextVersion $NewDefinition (Join-Path $TaskRoot 'new-setup')
    $FaultInstaller = CompileInstaller $NewSource $NextVersion $FaultDefinition (Join-Path $TaskRoot 'fault-setup')
    # All recovery assets are available BEFORE mutating the installation.
    $OldInstallerHash = (Get-FileHash -Algorithm SHA256 $OldInstaller).Hash
    $Results = @()
    foreach ($Desktop in @($true, $false)) {
        $Name = if ($Desktop) { 'desktop-on' } else { 'desktop-off' }
        $InstallRoot = Join-Path $TaskRoot ("custom path 数据 $Name")
        Install $OldInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-old.log")
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        $UserFile = Join-Path $InstallRoot 'user-project.instplot'
        [IO.File]::WriteAllText($UserFile, 'user data must survive upgrade and recovery')
        $UserHash = (Get-FileHash -Algorithm SHA256 $UserFile).Hash
        Install $FaultInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-cancel.log") $true
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        Install $NewInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-new.log")
        VerifyInstallation $InstallRoot $NextVersion $NewHash $Desktop
        if (-not (Test-Path (Join-Path $InstallRoot 'update-added.bin'))) { throw 'New release file missing.' }
        if ((Get-FileHash -Algorithm SHA256 $OldInstaller).Hash -ne $OldInstallerHash) { throw 'Recovery asset changed.' }
        Install $OldInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-restore.log")
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        # Inno downgrade alone can leave new release files: prove constrained cleanup.
        if (-not (Test-Path (Join-Path $InstallRoot 'update-added.bin'))) {
            throw 'Expected leftover release file not found; review recovery assumption.'
        }
        RemoveAddedFile $InstallRoot $AddedHash
        if ((Get-FileHash -Algorithm SHA256 $UserFile).Hash -ne $UserHash) { throw 'User file was modified.' }
        Checked (Join-Path $InstallRoot 'instplot-studio.exe') @('--export-fixed-png', (Join-Path $EvidenceRoot "$Name-restored.png"))
        $Uninstaller = Start-Process (Join-Path $InstallRoot 'unins000.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
        if ($Uninstaller.ExitCode -ne 0) { throw 'Uninstall failed after recovery.' }
        if ((Test-Path $Registration) -or (Test-Path $DesktopShortcut) -or (Test-Path $MenuShortcut) -or
            (Test-Path (Join-Path $InstallRoot 'instplot-studio.exe'))) { throw 'Installation identity/shortcuts remained after uninstall.' }
        if ((Get-FileHash -Algorithm SHA256 $UserFile).Hash -ne $UserHash) { throw 'Uninstall removed user data.' }
        $Results += @{ case = $Name; versions = @($CurrentVersion, $NextVersion, $CurrentVersion);
            original_path = $true; user_scope = $true; shortcuts_preserved = $true;
            installer_cancel_kept_old = $true; rollback_identity_and_hash = $true;
            added_file_hash_cleanup = $true; user_data_preserved = $true; uninstall_verified = $true }
    }
    @{ product = 'instplot-studio'; scope = 'installer-recovery-prototype-not-GUI-updater';
       production_version_unchanged = $true; results = $Results } | ConvertTo-Json -Depth 5 |
       Set-Content -Encoding utf8 (Join-Path $EvidenceRoot 'results.json')
    Write-Output 'Windows installer recovery prototype: PASS (real GUI updater remains pending)'
} finally {
    Pop-Location
    # Retain exact evidence and disposable fixtures on failure; do not broadly delete.
}
