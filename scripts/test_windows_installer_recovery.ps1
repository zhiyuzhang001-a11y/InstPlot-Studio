param(
    [string]$FixturePublicRoot = 'https://windows-update.example.test/instplot-studio',
    [switch]$KitOnly
)
# Disposable CI-only installer recovery prototype. Does NOT implement the updater.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Run only on a disposable Windows GitHub Actions runner, never on a user installation.'
}
if ($FixturePublicRoot -ne 'https://windows-update.example.test/instplot-studio') {
    $ExpectedRoot = "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/windows-gui-qa/$($env:GITHUB_RUN_ID)-$($env:GITHUB_RUN_ATTEMPT)"
    if (-not $KitOnly -or $env:GITHUB_RUN_ID -notmatch '^[0-9]+$' -or
        $env:GITHUB_RUN_ATTEMPT -notmatch '^[0-9]+$' -or $FixturePublicRoot -cne $ExpectedRoot) {
        throw 'Public GUI fixture must use this run-specific isolated OSS root in kit-only mode.'
    }
}
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
$MachineRegistration = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
$MachineRegistration32 = 'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
if ((Test-Path $Registration) -or (Test-Path $MachineRegistration) -or (Test-Path $MachineRegistration32)) {
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
    # Match the helper's read-only sharing lease. Prove real Inno installers
    # can execute while writes/deletes are denied for the verified asset.
    $Lease = [System.IO.File]::Open($Installer, [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    try {
        $BeforeHash = (Get-FileHash -Algorithm SHA256 $Installer).Hash
        $Process = Start-Process -FilePath $Installer -ArgumentList $Arguments -Wait -PassThru
        if ((Get-FileHash -Algorithm SHA256 $Installer).Hash -ne $BeforeHash) {
            throw 'Installer asset changed while its read lease was held.'
        }
        if ($ExpectFailure) {
            if ($Process.ExitCode -eq 0) { throw 'Fault installer unexpectedly succeeded.' }
        } elseif ($Process.ExitCode -ne 0) { throw "Installer failed: $($Process.ExitCode)" }
    } finally {
        $Lease.Dispose()
    }
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
    if ((Test-Path $MachineRegistration) -or (Test-Path $MachineRegistration32)) { throw 'Installer changed to machine scope.' }
    # Exercise production read-only native registry/Shell discovery, not a mock.
    $Discovery = & $Binary --check-update-installation | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $Discovery.product -ne 'instplot-studio' -or
        $Discovery.version -ne $Version -or $Discovery.scope -ne 'current_user' -or
        -not $Discovery.running_path_matches -or $Discovery.desktop_shortcut -ne $Desktop) {
        throw 'Native update discovery failed to bind the actual installed executable.'
    }
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

function ExpectDiscoveryFailure([string]$Binary, [string]$Case) {
    $Process = Start-Process -FilePath $Binary -ArgumentList '--check-update-installation' -Wait -PassThru `
        -RedirectStandardOutput (Join-Path $EvidenceRoot "$Case-discovery.stdout.log") `
        -RedirectStandardError (Join-Path $EvidenceRoot "$Case-discovery.stderr.log")
    if ($Process.ExitCode -eq 0) { throw "Unsafe installation discovery succeeded: $Case" }
}

function RegistrationSnapshot {
    if (-not (Test-Path $Registration)) { return 'absent' }
    # Partial installation may have missing fields. Record that state rather
    # than requiring a completed installation before testing its recovery.
    return (Get-ItemProperty $Registration | Select-Object DisplayVersion, InstallLocation, UninstallString |
        ConvertTo-Json -Compress)
}

function NativeRunnerPrototype([string]$Directory, [bool]$Desktop, [string]$Name) {
    $Variables = @{
        INSTPLOT_NATIVE_INNO_PROTOTYPE = '1'
        INSTPLOT_NATIVE_INNO_DIRECTORY = $Directory
        INSTPLOT_NATIVE_INNO_OLD = $OldInstaller
        INSTPLOT_NATIVE_INNO_NEW = $NewInstaller
        INSTPLOT_NATIVE_INNO_NEXT_VERSION = $NextVersion
        INSTPLOT_NATIVE_INNO_DESKTOP = $Desktop.ToString().ToLowerInvariant()
    }
    $PreviousEnvironment = @{}
    try {
        foreach ($Key in $Variables.Keys) {
            $PreviousEnvironment[$Key] = [Environment]::GetEnvironmentVariable($Key, 'Process')
            [Environment]::SetEnvironmentVariable($Key, $Variables[$Key], 'Process')
        }
        Checked 'cargo' @('test', '--locked', '--package', 'instplot-studio', '--lib',
            'update_windows::assets::native_inno_test::real_inno_native_runner_updates_and_restores',
            '--', '--exact', '--ignored', '--nocapture')
        $Receipt = Join-Path $EvidenceRoot "$Name-native-runner.json"
        if (-not (Test-Path $Receipt)) { throw 'Native test did not produce its own evidence.' }
        $Proof = Get-Content -Raw -LiteralPath $Receipt | ConvertFrom-Json
        if ($Proof.product -ne 'instplot-studio' -or $Proof.scope -ne 'real-native-runner-inno-not-GUI-updater' -or
            -not $Proof.applied -or -not $Proof.restored -or -not $Proof.user_data_preserved -or
            $Proof.previous_version -ne $CurrentVersion -or $Proof.candidate_version -ne $NextVersion -or
            $Proof.desktop_shortcut -ne $Desktop) { throw 'Native Inno evidence is incomplete or mismatched.' }
    } finally {
        foreach ($Key in $PreviousEnvironment.Keys) {
            [Environment]::SetEnvironmentVariable($Key, $PreviousEnvironment[$Key], 'Process')
        }
    }
}

Push-Location $RepositoryRoot
try {
    # Two real Studio builds; candidate version exists ONLY in a disposable snapshot.
    $Snapshot = Join-Path $TaskRoot 'snapshot'
    New-Item -ItemType Directory $Snapshot | Out-Null
    $Archive = Join-Path $TaskRoot 'snapshot.tar'
    Checked 'git' @('archive', '--format=tar', "--output=$Archive", 'HEAD')
    Checked 'tar' @('-xf', $Archive, '-C', $Snapshot)
    # Fixture trust is isolated to the snapshot. Never read production keys,
    # change the checkout trust root, or export this private key in artifacts.
    $FixtureKey = Join-Path $TaskRoot 'preview-fixture-key.pem'
    Checked 'openssl' @('genpkey', '-algorithm', 'ED25519', '-out', $FixtureKey)
    $FixturePublic = ((Checked 'cargo' @('run', '--locked', '--quiet', '--package', 'instplot-update-signature', '--', 'public-key-hex', $FixtureKey)) -join '').Trim()
    if ($FixturePublic -notmatch '^[0-9a-f]{64}$') { throw 'Invalid isolated fixture public key.' }
    $FixtureNextKey = Join-Path $TaskRoot 'preview-fixture-next-key.pem'
    Checked 'openssl' @('genpkey', '-algorithm', 'ED25519', '-out', $FixtureNextKey)
    $FixtureNextPublic = ((Checked 'cargo' @('run', '--locked', '--quiet', '--package', 'instplot-update-signature', '--', 'public-key-hex', $FixtureNextKey)) -join '').Trim()
    if ($FixtureNextPublic -notmatch '^[0-9a-f]{64}$' -or $FixtureNextPublic -eq $FixturePublic) { throw 'Invalid next fixture public key.' }
    $FixtureKeys = @(
        @{ id = 'windows-preview-fixture'; public_key_hex = $FixturePublic },
        @{ id = 'windows-preview-fixture-next'; public_key_hex = $FixtureNextPublic }
    )
    [ordered]@{
        public_root = $FixturePublicRoot
        keys = $FixtureKeys
    } | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8NoBOM (Join-Path $Snapshot 'packaging/update/trust.json')
    $Manifest = Join-Path $Snapshot 'Cargo.toml'
    Checked 'cargo' @('build', '--locked', '--manifest-path', $Manifest,
        '--target-dir', (Join-Path $RepositoryRoot 'target'), '--package', 'instplot-studio', '--features', 'in-place-update-preview')
    $OldSource = Join-Path $TaskRoot 'old-source'
    $NewSource = Join-Path $TaskRoot 'new-source'
    New-Item -ItemType Directory $OldSource, $NewSource | Out-Null
    Copy-Item 'target/debug/instplot-studio.exe' $OldSource
    foreach ($Source in @($OldSource, $NewSource)) {
        Copy-Item 'LICENSE' $Source
        Copy-Item 'apps/instplot-studio/assets/InstPlotStudio.ico' $Source
    }
    $Content = [IO.File]::ReadAllText($Manifest)
    $Before = 'version = "' + $CurrentVersion + '"'
    if (([regex]::Matches($Content, [regex]::Escape($Before))).Count -ne 1) { throw 'Ambiguous workspace version.' }
    [IO.File]::WriteAllText($Manifest, $Content.Replace($Before, 'version = "' + $NextVersion + '"'))
    Checked 'cargo' @('build', '--offline', '--manifest-path', $Manifest,
        '--target-dir', (Join-Path $RepositoryRoot 'target'), '--package', 'instplot-studio', '--features', 'in-place-update-preview')
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
    # GUI kit uses only the unmodified product installer (no injected extra file).
    $CandidateDefinition = if ($KitOnly) { $Definition } else { $NewDefinition }
    $NewInstaller = CompileInstaller $NewSource $NextVersion $CandidateDefinition (Join-Path $TaskRoot 'new-setup')
    if (-not $KitOnly) {
        $FaultInstaller = CompileInstaller $NewSource $NextVersion $FaultDefinition (Join-Path $TaskRoot 'fault-setup')
    }
    # Preserve the two compatible preview installers and exact-file evidence.
    # These are disposable test versions, not published/accepted releases.
    $KitRoot = Join-Path $EvidenceRoot 'preview-kit'
    New-Item -ItemType Directory $KitRoot | Out-Null
    [ordered]@{
        scope = 'signed-fixture-metadata-not-GUI-updater'
        public_root = $FixturePublicRoot
        keys = $FixtureKeys
        binaries_use_fixture_trust = $true
    } | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8NoBOM (Join-Path $KitRoot 'fixture-trust.json')
    $env:INSTPLOT_WINDOWS_PREVIEW_FIXTURE_KEY = $FixtureKey
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "INSTPLOT_WINDOWS_PREVIEW_FIXTURE_KEY=$FixtureKey"
    foreach ($Item in @(
        @{ Version = $CurrentVersion; Source = $OldSource; Installer = $OldInstaller },
        @{ Version = $NextVersion; Source = $NewSource; Installer = $NewInstaller }
    )) {
        $Binary = Join-Path $Item.Source 'instplot-studio.exe'
        if ($KitOnly) {
            & (Join-Path $RepositoryRoot 'scripts/verify_windows_icon.ps1') -Executable $Binary
        }
        $Probe = ((& $Binary --windows-update-capabilities) -join "`n") | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0 -or $Probe.product -ne 'instplot-studio' -or
            $Probe.version -ne $Item.Version -or $Probe.platform -ne 'windows-x86_64' -or
            $Probe.scope -ne 'preview-components-not-accepted-updater' -or
            $Probe.public_update_protocol -ne 0 -or $Probe.gui_acceptance_complete -ne $false -or
            $Probe.public_apply_entry_enabled -ne $false) { throw 'Invalid preview kit identity/capability scope.' }
        $Contract = [ordered]@{ schema = 1 }
        foreach ($Name in @('helper_protocol', 'transaction_schema', 'candidate_health_protocol', 'recovery_health_protocol')) {
            if ($Probe.components.$Name -ne 1) { throw "Invalid preview kit protocol: $Name" }
            $Contract[$Name] = 1
        }
        $Contract['executable_sha256'] = (Get-FileHash -Algorithm SHA256 $Binary).Hash.ToLowerInvariant()
        $Contract['license_sha256'] = (Get-FileHash -Algorithm SHA256 (Join-Path $Item.Source 'LICENSE')).Hash.ToLowerInvariant()
        Copy-Item $Item.Installer $KitRoot
        [ordered]@{
            scope = 'preview-components-not-accepted-updater'
            version = $Item.Version
            installer_sha256 = (Get-FileHash -Algorithm SHA256 $Item.Installer).Hash.ToLowerInvariant()
            contract = $Contract
        } | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM (Join-Path $KitRoot ($Item.Version + '-windows-in-place.json'))
    }
    if ($KitOnly) {
        Write-Output 'Windows isolated GUI kit built; no user installation or GUI acceptance performed.'
        return
    }
    # Disposable fault installers only. AfterInstall/Win32 DLL declarations use
    # Inno's documented script interfaces; none enter the product definition.
    # https://jrsoftware.org/ishelp/topic_scriptinstall.htm
    # https://jrsoftware.org/ishelp/topic_scriptdll.htm
    $InterruptDefinition = Join-Path $TaskRoot 'interrupt.iss'
    $AddedEntry = 'Source: "{#SourceDir}\update-added.bin"; DestDir: "{app}"; Flags: ignoreversion'
    $InterruptContent = [IO.File]::ReadAllText($NewDefinition)
    if (([regex]::Matches($InterruptContent, [regex]::Escape($AddedEntry))).Count -ne 1) {
        throw 'Ambiguous interruption point; refuse to inject a different failure.'
    }
    $InterruptCode = @'

[Code]
procedure ExitProcess(ExitCode: Cardinal);
  external 'ExitProcess@kernel32.dll stdcall';
procedure InterruptAfterPayload;
begin
  if not SaveStringToFile(ExpandConstant('{app}\prototype-interrupted.txt'), 'after-payload', False) then
    RaiseException('Cannot record interruption point');
  ExitProcess(91);
end;
'@
    [IO.File]::WriteAllText($InterruptDefinition,
        $InterruptContent.Replace($AddedEntry, $AddedEntry + '; AfterInstall: InterruptAfterPayload') + $InterruptCode)
    $FailedRecoveryDefinition = Join-Path $TaskRoot 'failed-recovery.iss'
    [IO.File]::WriteAllText($FailedRecoveryDefinition, [IO.File]::ReadAllText($Definition) + "`n[Code]`n" +
        "function InitializeSetup(): Boolean;`nbegin`n  Result := False;`nend;`n")
    $InterruptInstaller = CompileInstaller $NewSource $NextVersion $InterruptDefinition (Join-Path $TaskRoot 'interrupt-setup')
    $FailedRecoveryInstaller = CompileInstaller $OldSource $CurrentVersion $FailedRecoveryDefinition (Join-Path $TaskRoot 'failed-recovery-setup')
    # All recovery assets are available BEFORE mutating the installation.
    $OldInstallerHash = (Get-FileHash -Algorithm SHA256 $OldInstaller).Hash
    $Results = @()
    foreach ($Desktop in @($true, $false)) {
        $Name = if ($Desktop) { 'desktop-on' } else { 'desktop-off' }
        $InstallRoot = Join-Path $TaskRoot ("custom path 数据 $Name")
        Install $OldInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-old.log")
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        ExpectDiscoveryFailure $OldBinary "$Name-portable-copy"
        # Only the disposable CI fixture's exact HKCU record is changed/restored.
        try {
            Set-ItemProperty -LiteralPath $Registration -Name DisplayVersion -Value '0.0.0'
            ExpectDiscoveryFailure (Join-Path $InstallRoot 'instplot-studio.exe') "$Name-wrong-version"
        } finally {
            Set-ItemProperty -LiteralPath $Registration -Name DisplayVersion -Value $CurrentVersion
        }
        $OriginalLocation = (Get-ItemProperty $Registration).InstallLocation
        try {
            Set-ItemProperty -LiteralPath $Registration -Name InstallLocation -Value $OldSource
            ExpectDiscoveryFailure (Join-Path $InstallRoot 'instplot-studio.exe') "$Name-wrong-path"
        } finally {
            Set-ItemProperty -LiteralPath $Registration -Name InstallLocation -Value $OriginalLocation
        }
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        # Negative controls: Unicode handling must not weaken exact target/icon checks.
        $BadShortcut = Join-Path $TaskRoot "$Name-wrong-target.lnk"
        $BadLink = (New-Object -ComObject WScript.Shell).CreateShortcut($BadShortcut)
        $BadLink.TargetPath = $OldBinary
        $BadLink.IconLocation = "$OldBinary,0"
        $BadLink.Save()
        $Rejected = $false
        try { [StudioIconResource]::VerifyShortcut($BadShortcut, (Join-Path $InstallRoot 'instplot-studio.exe')) }
        catch {
            if ($_.Exception.ToString() -notmatch 'Shortcut targets the wrong executable') { throw }
            $Rejected = $true
        }
        if (-not $Rejected) { throw 'Wrong shortcut target was accepted.' }
        $Rejected = $false
        try { [StudioIconResource]::VerifyShortcut($MenuShortcut, $OldBinary) }
        catch {
            if ($_.Exception.ToString() -notmatch 'Shortcut targets the wrong executable') { throw }
            $Rejected = $true
        }
        if (-not $Rejected) { throw 'Installed shortcut accepted against a different executable.' }
        $BadLink.IconLocation = "$NewBinary,0"
        $BadLink.Save()
        $Rejected = $false
        try { [StudioIconResource]::VerifyShortcut($BadShortcut, $OldBinary) }
        catch {
            if ($_.Exception.ToString() -notmatch 'Unexpected shortcut icon') { throw }
            $Rejected = $true
        }
        if (-not $Rejected) { throw 'Wrong shortcut icon was accepted.' }
        $UserFile = Join-Path $InstallRoot 'user-project.instplot'
        [IO.File]::WriteAllText($UserFile, 'user data must survive upgrade and recovery')
        $UserHash = (Get-FileHash -Algorithm SHA256 $UserFile).Hash
        NativeRunnerPrototype $InstallRoot $Desktop $Name
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        RemoveAddedFile $InstallRoot $AddedHash
        Install $FaultInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-cancel.log") $true
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        # Abrupt exit AFTER real new payload copy, before the remaining install
        # stages: do not substitute an early InitializeSetup cancellation.
        Install $InterruptInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-interrupt.log") $true
        $InterruptProof = Join-Path $InstallRoot 'prototype-interrupted.txt'
        if (-not (Test-Path $InterruptProof) -or [IO.File]::ReadAllText($InterruptProof) -ne 'after-payload') {
            throw 'Installer did not reach the specified post-payload interruption point.'
        }
        ProductIdentity (Join-Path $InstallRoot 'instplot-studio.exe') $NextVersion
        if ((Get-FileHash -Algorithm SHA256 (Join-Path $InstallRoot 'instplot-studio.exe')).Hash -ne $NewHash -or
            (Get-FileHash -Algorithm SHA256 (Join-Path $InstallRoot 'update-added.bin')).Hash -ne $AddedHash) {
            throw 'Interruption did not leave the recorded new payload.'
        }
        $InterruptedRecord = RegistrationSnapshot
        Install $FailedRecoveryInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-failed-recovery.log") $true
        if ((Get-FileHash -Algorithm SHA256 (Join-Path $InstallRoot 'instplot-studio.exe')).Hash -ne $NewHash -or
            (RegistrationSnapshot) -ne $InterruptedRecord -or
            (Get-FileHash -Algorithm SHA256 $UserFile).Hash -ne $UserHash -or
            (Get-FileHash -Algorithm SHA256 $OldInstaller).Hash -ne $OldInstallerHash) {
            throw 'Failed recovery changed the partial install/user data or lost its hash-verified recovery fixture.'
        }
        # A later explicit fixture recovery, not an automatic retry loop.
        Install $OldInstaller $InstallRoot $Desktop (Join-Path $EvidenceRoot "$Name-interrupted-restore.log")
        VerifyInstallation $InstallRoot $CurrentVersion $OldHash $Desktop
        RemoveAddedFile $InstallRoot $AddedHash
        if ((Get-FileHash -Algorithm SHA256 $UserFile).Hash -ne $UserHash) { throw 'Interrupted recovery modified user data.' }
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
        $AddedFile = Join-Path $InstallRoot 'update-added.bin'
        [IO.File]::WriteAllText($AddedFile, 'modified fixture file must not be deleted')
        $ModifiedHash = (Get-FileHash -Algorithm SHA256 $AddedFile).Hash
        $CleanupRejected = $false
        try { RemoveAddedFile $InstallRoot $AddedHash }
        catch {
            if ($_.Exception.ToString() -notmatch 'New release file changed') { throw }
            $CleanupRejected = $true
        }
        if (-not $CleanupRejected -or -not (Test-Path $AddedFile) -or
            (Get-FileHash -Algorithm SHA256 $AddedFile).Hash -ne $ModifiedHash) {
            throw 'Modified release file was deleted or changed by cleanup.'
        }
        Copy-Item (Join-Path $NewSource 'update-added.bin') $AddedFile
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
            wrong_shortcut_target_and_icon_rejected = $true;
            native_registry_and_shortcut_discovery = $true;
            portable_and_conflicting_registry_rejected = $true;
            installer_cancel_kept_old = $true; rollback_identity_and_hash = $true;
            installer_read_lease_execution = $true;
            rust_native_inno_apply_and_restore = $true;
            interrupted_after_payload_restored = $true;
            failed_recovery_kept_assets_and_user_data = $true;
            modified_added_file_cleanup_rejected = $true;
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
