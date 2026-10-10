# Actual GUI input and native-process/window checks, on a disposable VM only.
# The strict loopback TLS snapshot is NOT the exact public QA binary/network.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'Disposable Windows runner only.' }
$Repository = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'invoke_studio_probe.ps1')
$RegistrationName = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
foreach ($Hive in @('HKCU:', 'HKLM:', 'HKLM:\Software\WOW6432Node')) {
    $Suffix = if ($Hive -like '*WOW6432Node') { $RegistrationName.Substring('Software\'.Length) } else { $RegistrationName }
    if (Test-Path (Join-Path $Hive $Suffix)) { throw 'Existing Studio registration; refusing to overwrite.' }
}
$MenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'InstPlot Studio.lnk'
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'InstPlot Studio.lnk'
$Updater = Join-Path $env:LOCALAPPDATA 'InstPlot Studio Updater'
foreach ($Path in @($MenuShortcut, $DesktopShortcut, $Updater)) {
    if (Test-Path -LiteralPath $Path) { throw 'Existing Studio state; refusing to modify it.' }
}
$Evidence = Join-Path $Repository 'target/windows-gui-e2e'
New-Item -ItemType Directory $Evidence | Out-Null
$Task = Join-Path $env:RUNNER_TEMP ('studio-gui-e2e-' + [guid]::NewGuid())
New-Item -ItemType Directory $Task | Out-Null
$Clock = [Diagnostics.Stopwatch]::StartNew()
$Phases = [Collections.Generic.List[object]]::new()
function Phase([string]$Name) {
    $Phases.Add(@{ phase=$Name; elapsed_ms=$Clock.ElapsedMilliseconds })
    $Phases | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM (Join-Path $Evidence 'phases.json')
}
function Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with $LASTEXITCODE" }
}
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
public sealed class GuiWindow {
    public long Handle; public int Pid; public string Title;
    public bool Visible, Minimized, Cloaked, OnMonitor;
}
public static class StudioGuiE2E {
    [DllImport("kernel32.dll", SetLastError=true)] static extern SafeWaitHandle OpenProcess(uint access,bool inherit,int pid);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetProcessTimes(SafeWaitHandle h,out long created,out long exited,out long kernel,out long user);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetExitCodeProcess(SafeWaitHandle h,out uint code);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint WaitForSingleObject(SafeWaitHandle h,uint milliseconds);
    public static SafeWaitHandle BindExitWitness(int pid,long expectedCreated) {
        // Query/synchronize only; retaining the handle prevents PID reuse after exit.
        var h=OpenProcess(0x00101000,false,pid);
        long created,exited,kernel,user;
        if(h.IsInvalid || !GetProcessTimes(h,out created,out exited,out kernel,out user) || created!=expectedCreated) {
            h.Dispose(); throw new InvalidOperationException("Candidate exit witness binding failed.");
        }
        return h;
    }
    public static uint WaitExitCode(SafeWaitHandle h) {
        uint code;
        if(WaitForSingleObject(h,20000)!=0 || !GetExitCodeProcess(h,out code))
            throw new InvalidOperationException("Candidate native exit evidence unavailable.");
        return code;
    }
    delegate bool Callback(IntPtr h, IntPtr p);
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int L,T,R,B; }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback c, IntPtr p);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromRect(ref Rect r, uint f);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h,uint a,out uint v,uint n);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int command);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int width,int height,uint flags);
    [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(uint f,bool inherit,uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr d);
    public static GuiWindow[] Windows(int processId) {
        var list=new List<GuiWindow>();
        EnumWindows((h,p)=>{
            uint id; GetWindowThreadProcessId(h,out id); if(id!=processId)return true;
            Rect r; if(!GetWindowRect(h,out r))return true;
            uint cloaked; int status=DwmGetWindowAttribute(h,14,out cloaked,4);
            var title=new StringBuilder(1024); GetWindowText(h,title,title.Capacity);
            list.Add(new GuiWindow { Handle=h.ToInt64(),Pid=(int)id,Title=title.ToString(),
                Visible=IsWindowVisible(h),Minimized=IsIconic(h),Cloaked=status!=0||cloaked!=0,
                OnMonitor=r.R>r.L&&r.B>r.T&&MonitorFromRect(ref r,0)!=IntPtr.Zero });
            return true;
        },IntPtr.Zero); return list.ToArray();
    }
}
'@
# Exercise native exit evidence on both success and failure before expensive builds.
foreach ($ExpectedExitCode in @(0, 7)) {
    $ExitProbe = Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList @(
        '-NoProfile', '-NonInteractive', '-Command', "Start-Sleep -Seconds 2; exit $ExpectedExitCode"
    ) -PassThru
    $ProbeWitness = [StudioGuiE2E]::BindExitWitness($ExitProbe.Id, $ExitProbe.StartTime.ToUniversalTime().ToFileTimeUtc())
    try {
        if ([StudioGuiE2E]::WaitExitCode($ProbeWitness) -ne $ExpectedExitCode) { throw 'Native exit witness regression failed.' }
    } finally { $ProbeWitness.Dispose() }
}
Phase 'native-exit-zero-and-nonzero-evidence-verified'
$Desktop = [StudioGuiE2E]::OpenInputDesktop(0, $false, 1)
if ($Desktop -eq [IntPtr]::Zero -or -not [Environment]::UserInteractive) { throw 'No interactive desktop; GUI acceptance cannot pass.' }
[void][StudioGuiE2E]::CloseDesktop($Desktop)
function Screenshot([string]$Name) {
    $Bounds = [Windows.Forms.SystemInformation]::VirtualScreen
    $Image = [Drawing.Bitmap]::new($Bounds.Width, $Bounds.Height)
    $Graphics = [Drawing.Graphics]::FromImage($Image)
    try { $Graphics.CopyFromScreen($Bounds.Left,$Bounds.Top,0,0,$Bounds.Size)
        $Image.Save((Join-Path $Evidence "$Name.png"),[Drawing.Imaging.ImageFormat]::Png)
    } finally { $Graphics.Dispose(); $Image.Dispose() }
}
function VisibleWindow([int]$ProcessId, [string]$Title = '') {
    @([StudioGuiE2E]::Windows($ProcessId) | Where-Object {
        $_.Visible -and -not $_.Minimized -and -not $_.Cloaked -and $_.OnMonitor -and
        ($Title -eq '' -or $_.Title -ceq $Title)
    }) | Select-Object -First 1
}
$Certificates = Join-Path $Task 'tls'
Checked 'python3' @('-m','scripts.windows_gui_e2e_fixture','certificates','--output',$Certificates)
Phase 'tls-fixture-created'
& (Join-Path $PSScriptRoot 'test_windows_installer_recovery.ps1') -KitOnly -GuiFixture `
    -FixturePublicRoot 'https://localhost:38443/instplot-studio' -FixtureCaPem (Join-Path $Certificates 'ca.pem')
$Kit = Join-Path $Repository 'target/windows-update-recovery/preview-kit'
$ServiceRoot = Join-Path $Task 'service'
$Source = (& git -C $Repository rev-parse HEAD).Trim()
Checked 'python3' @('-m','scripts.windows_gui_e2e_fixture','stage','--kit',$Kit,'--output',$ServiceRoot,
    '--private-key',$env:INSTPLOT_WINDOWS_PREVIEW_FIXTURE_KEY,'--source-sha',$Source)
$Index = Get-Content -Raw (Join-Path $ServiceRoot 'fixture-index.json') | ConvertFrom-Json
Copy-Item (Join-Path $ServiceRoot 'fixture-index.json') $Evidence
Phase 'release-pair-built-signed-and-validated'
$Token = ([guid]::NewGuid().ToString('N') + [guid]::NewGuid().ToString('N'))
$env:STUDIO_GUI_FIXTURE_TOKEN = $Token
$env:STUDIO_GUI_FIXTURE_CA = Join-Path $Certificates 'ca.pem'
$ServerArguments = @('-m','scripts.windows_gui_e2e_fixture','serve','--root',$ServiceRoot,
    '--certificate',(Join-Path $Certificates 'server.pem'),'--private-key',(Join-Path $Certificates 'server-key.pem'),'--token',$Token)
$Service = Start-Process python3 -ArgumentList $ServerArguments -WorkingDirectory $Repository -PassThru `
    -RedirectStandardOutput (Join-Path $Evidence 'tls-server.log') -RedirectStandardError (Join-Path $Evidence 'tls-server-errors.log')
try {
    Start-Sleep -Seconds 2
    if ($Service.HasExited) { throw 'Fixture TLS server exited.' }
    Checked 'python3' @('-m','scripts.windows_gui_e2e_fixture','verify-service','--ca',(Join-Path $Certificates 'ca.pem'))
    Phase 'tls-positive-negative-and-private-file-guards-passed'
    $InstallRoot = Join-Path $Task 'installation'
    $BaselineInstaller = Join-Path $Kit "InstPlot-Studio-$($Index.baseline)-windows-x86_64-setup.exe"
    $Setup = Start-Process $BaselineInstaller -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-',
        '/NOCLOSEAPPLICATIONS','/NORESTARTAPPLICATIONS','/TASKS=desktopicon',('/DIR="'+$InstallRoot+'"'),
        ('/LOG="'+(Join-Path $Evidence 'baseline-install.log')+'"')) -PassThru
    if (-not $Setup.WaitForExit(120000) -or $Setup.ExitCode -ne 0) { throw 'Baseline setup failed.' }
    $MesaArchive = Join-Path $Task 'mesa.7z'
    $MesaRoot = Join-Path $Task 'mesa'
    Invoke-WebRequest 'https://github.com/pal1000/mesa-dist-win/releases/download/26.2.4/mesa3d-26.2.4-release-msvc.7z' -OutFile $MesaArchive
    $MesaSha = '351fc8c8b695878ffb3eaa044b3ead08672a48b1a045e3c3e3975811df0f6695'
    if ((Get-Item $MesaArchive).Length -ne 70257286 -or (Get-FileHash $MesaArchive).Hash.ToLowerInvariant() -cne $MesaSha) { throw 'Pinned Mesa hash/size mismatch.' }
    Checked '7z.exe' @('x',$MesaArchive,"-o$MesaRoot",'x64/opengl32.dll','x64/libgallium_wgl.dll','-y')
    foreach ($Dll in @('opengl32.dll','libgallium_wgl.dll')) {
        $Destination = Join-Path $InstallRoot $Dll
        if (Test-Path $Destination) { throw 'Foreign renderer DLL exists.' }
        Copy-Item (Join-Path $MesaRoot "x64/$Dll") $Destination
    }
    $env:GALLIUM_DRIVER = 'llvmpipe'
    @{ scope='temporary-VM-software-GPU-not-user-GPU'; sha256=$MesaSha } | ConvertTo-Json |
        Set-Content -Encoding utf8NoBOM (Join-Path $Evidence 'graphics-environment.json')
    $Binary = Join-Path $InstallRoot 'instplot-studio.exe'
    $Project = Join-Path $Task 'GUI-sentinel.instplot'
    $Created = Invoke-StudioProbe $Binary @('--create-project',$Project)
    $ProjectHash = (Get-FileHash $Project).Hash
    $Baseline = Start-Process $Binary -ArgumentList ('"'+$Project+'"') -PassThru -RedirectStandardError (Join-Path $Evidence 'baseline-startup.log')
    $Deadline = [datetime]::UtcNow.AddSeconds(60)
    do {
        if ($Baseline.HasExited) { throw 'Baseline exited before trusted recovery bootstrap.' }
        $Recovery = @(Get-ChildItem (Join-Path $Updater 'downloads/recovery-metadata') -Filter current.json -Recurse -ErrorAction SilentlyContinue)
        if ($Recovery.Count -gt 0 -and $null -ne (VisibleWindow $Baseline.Id) -and
            (Test-Path (Join-Path $Updater 'downloads/background-check.json'))) { break }
        Start-Sleep -Milliseconds 200
    } while ([datetime]::UtcNow -lt $Deadline)
    if ($Recovery.Count -ne 1 -or $Baseline.HasExited -or $null -eq (VisibleWindow $Baseline.Id) -or
        -not (Test-Path (Join-Path $Updater 'downloads/background-check.json'))) { throw 'Visible baseline/trusted recovery metadata/successful startup check missing or ambiguous.' }
    Screenshot '01-baseline-trusted-recovery'
    Phase 'baseline-visible-and-recovery-metadata-retained'
    if (-not $Baseline.CloseMainWindow() -or -not $Baseline.WaitForExit(20000) -or $Baseline.ExitCode -ne 0) { throw 'Baseline did not close normally.' }
    # The real product throttle is preserved. No deleting state or shortening it.
    $Timestamp = [long](Get-Content -Raw (Join-Path $Updater 'downloads/background-check.json') | ConvertFrom-Json)
    $Due = [DateTimeOffset]::FromUnixTimeSeconds($Timestamp + 305)
    while ([DateTimeOffset]::UtcNow -lt $Due) { Start-Sleep -Seconds 5 }
    Checked 'python3' @('-c', 'import os,ssl,urllib.request; c=ssl.create_default_context(cafile=os.environ["STUDIO_GUI_FIXTURE_CA"]); r=urllib.request.Request("https://localhost:38443/_fixture/activate",data=b"",headers={"X-Fixture-Token":os.environ["STUDIO_GUI_FIXTURE_TOKEN"]}); urllib.request.urlopen(r,context=c).close()')
    Phase 'local-candidate-activated-after-real-throttle'
    $Parent = Start-Process $Binary -ArgumentList ('"'+$Project+'"') -PassThru -RedirectStandardError (Join-Path $Evidence 'update-parent-startup.log')
    $Deadline = [datetime]::UtcNow.AddSeconds(60)
    $Popup = $null
    $SizedParent = $false
    do {
        if ($Parent.HasExited) { throw 'Old GUI exited before update click.' }
        if (-not $SizedParent) {
            $RootWindow = VisibleWindow $Parent.Id
            if ($null -ne $RootWindow -and $RootWindow.Title -like 'InstPlot Studio*') {
                # The VM's 1024x768 desktop otherwise makes the main canvas
                # fill the monitor and intentionally embeds tool windows.
                # Resize our own real window, not the product/popup state.
                [void][StudioGuiE2E]::ShowWindow([IntPtr]$RootWindow.Handle,9)
                if (-not [StudioGuiE2E]::SetWindowPos([IntPtr]$RootWindow.Handle,[IntPtr]::Zero,40,40,800,600,20)) { throw 'Cannot resize owned GUI for detached-tool acceptance.' }
                $SizedParent = $true
                Phase 'owned-main-window-sized-for-detached-tool-mode'
            }
        }
        $Popup = VisibleWindow $Parent.Id '检查更新'
        if ($null -ne $Popup) { break }
        Start-Sleep -Milliseconds 200
    } while ([datetime]::UtcNow -lt $Deadline)
    if ($null -eq $Popup) { throw 'No automatic visible update popup on startup.' }
    Screenshot '02-automatic-update-popup'
    Phase 'automatic-update-popup-visible'
    [void][StudioGuiE2E]::SetForegroundWindow([IntPtr]$Popup.Handle)
    Start-Sleep -Milliseconds 500
    if ([StudioGuiE2E]::GetForegroundWindow().ToInt64() -ne $Popup.Handle) { throw 'Cannot safely direct GUI input to owned update popup.' }
    # Real keyboard input to the first enabled action, not a test-only apply CLI.
    [Windows.Forms.SendKeys]::SendWait('{TAB}')
    Screenshot '03-update-action-focused'
    if ($Parent.HasExited -or [StudioGuiE2E]::GetForegroundWindow().ToInt64() -ne $Popup.Handle -or
        $null -eq ([StudioGuiE2E]::Windows($Parent.Id) | Where-Object { $_.Handle -eq $Popup.Handle -and $_.Visible -and -not $_.Minimized -and -not $_.Cloaked -and $_.OnMonitor })) { throw 'Owned update popup lost foreground before Enter; no input sent.' }
    [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Phase 'real-update-action-input-sent'
    $Deadline = [datetime]::UtcNow.AddSeconds(240)
    $Transaction = $null
    $OldExitRecorded = $false
    $LastStage = ''
    do {
        if (-not $OldExitRecorded -and $Parent.HasExited) { Phase 'old-GUI-native-exit-observed'; $OldExitRecorded = $true }
        $Files = @(Get-ChildItem (Join-Path $Updater 'transactions') -Filter transaction.json -Recurse -ErrorAction SilentlyContinue)
        if ($Files.Count -gt 1) { throw 'More than one update transaction started.' }
        if ($Files.Count -eq 1) {
            $Transaction = Get-Content -Raw $Files[0].FullName | ConvertFrom-Json
            if ($Transaction.stage -cne $LastStage) { $LastStage = $Transaction.stage; Phase ("transaction-" + $LastStage) }
            if ($Transaction.stage -eq 'completed') { break }
            if ($Transaction.stage -in @('failed_before_apply','rolled_back','inspection_required')) { throw "Update ended as $($Transaction.stage)." }
        }
        Start-Sleep -Milliseconds 500
    } while ([datetime]::UtcNow -lt $Deadline)
    if ($null -eq $Transaction -or $Transaction.stage -ne 'completed' -or $null -ne $Transaction.last_error) { throw 'Update did not reach healthy completed state.' }
    if (-not $Parent.WaitForExit(1000) -or $Parent.ExitCode -ne 0) { throw 'Old GUI did not exit normally.' }
    $Candidate = Get-Process -Id ([int]$Transaction.candidate_process[0])
    if ($Candidate.Id -eq $Parent.Id -or $Candidate.Path -cne $Binary -or
        $Candidate.StartTime.ToUniversalTime().ToFileTimeUtc().ToString() -cne [string]$Transaction.candidate_process[1]) { throw 'Candidate native process binding differs.' }
    $Request = Get-Content -Raw (Join-Path $Files[0].DirectoryName 'request.json') | ConvertFrom-Json
    if ($Request.resume_project.sha256 -cne $ProjectHash.ToLowerInvariant()) { throw 'Saved project resume binding differs.' }
    $WindowEvidence = Join-Path $Files[0].DirectoryName 'candidate-window.json'
    $Deadline = [datetime]::UtcNow.AddSeconds(5)
    while (-not (Test-Path $WindowEvidence) -and [datetime]::UtcNow -lt $Deadline) { Start-Sleep -Milliseconds 100 }
    $Witness = Get-Content -Raw $WindowEvidence | ConvertFrom-Json
    if ($Witness.process_id -ne $Candidate.Id -or -not $Witness.visible -or $Witness.minimized -or $Witness.cloaked -or -not $Witness.on_monitor) { throw 'Native candidate window diagnostic missing.' }
    Phase 'old-exited-and-new-visible-health-completed'
    for ($Count=0; $Count -lt 60; $Count++) {
        $Owned = @(Get-Process -Name instplot-studio -ErrorAction SilentlyContinue | Where-Object { $_.Path -ceq $Binary })
        if ($Owned.Count -ne 1 -or $Candidate.HasExited -or $null -eq (VisibleWindow $Candidate.Id)) { throw 'New GUI not uniquely visible/alive after helper completion.' }
        Start-Sleep -Milliseconds 500
    }
    Screenshot '04-updated-visible-survived-helper'
    if ((VisibleWindow $Candidate.Id).Title -notlike '*GUI-sentinel*') { throw 'Visible candidate did not reopen the saved project.' }
    $Registration = Get-ItemProperty "HKCU:\$RegistrationName"
    if ($Registration.DisplayVersion -cne $Index.candidate -or $Registration.InstallLocation.TrimEnd('\') -cne $InstallRoot) { throw 'Wrong updated registration/path.' }
    $Shell = New-Object -ComObject WScript.Shell
    foreach ($Shortcut in @($MenuShortcut,$DesktopShortcut)) {
        if (-not (Test-Path $Shortcut) -or $Shell.CreateShortcut($Shortcut).TargetPath -cne $Binary) { throw 'Updated shortcut target differs.' }
    }
    if ((Get-FileHash $Project).Hash -cne $ProjectHash) { throw 'Project changed during update.' }
    $Identity = Invoke-StudioProbe $Binary @('--product-info')
    if ($Identity -cne "InstPlot Studio`tinstplot-studio`t$($Index.candidate)") { throw 'Installed candidate identity differs.' }
    $CheckedProject = Invoke-StudioProbe $Binary @('--check-project',$Project)
    $SuccessResult = @{ scope=$Index.scope; source_sha=$Source; old_pid=$Parent.Id; new_pid=$Candidate.Id;
       version=$Index.candidate; native_windows=[StudioGuiE2E]::Windows($Candidate.Id);
       project_sha256=$ProjectHash.ToLowerInvariant(); single_gui=$true; completed=$true; overall_success=$true }
    Phase 'project-shortcuts-version-and-single-visible-GUI-verified'
    # Get-Process observes a helper-owned process, not our Start-Process child.
    # Retain a bound native handle before requesting normal closure; do not infer
    # an exit code from a lazily opened Process handle after the process is gone.
    $ExitWitness = [StudioGuiE2E]::BindExitWitness($Candidate.Id, [long]$Transaction.candidate_process[1])
    try {
        if (-not $Candidate.CloseMainWindow()) { throw 'Candidate normal close request rejected.' }
        $NativeExitCode = [StudioGuiE2E]::WaitExitCode($ExitWitness)
        @{ process_id=$Candidate.Id; process_created=[string]$Transaction.candidate_process[1]; exit_code=$NativeExitCode } |
            ConvertTo-Json | Set-Content -Encoding utf8NoBOM (Join-Path $Evidence 'candidate-exit.json')
        if ($NativeExitCode -ne 0) { throw "Candidate did not close normally: native exit code $NativeExitCode." }
        Phase 'candidate-native-normal-exit-confirmed'
    } finally { $ExitWitness.Dispose() }
} finally {
    Screenshot 'final-desktop'
    if (Test-Path $Updater) {
        New-Item -ItemType Directory (Join-Path $Evidence 'updater-evidence') | Out-Null
        Get-ChildItem $Updater -Recurse -File | Where-Object { $_.Extension -in @('.json','.log') } | ForEach-Object {
            $Relative = [IO.Path]::GetRelativePath($Updater, $_.FullName)
            $Destination = Join-Path $Evidence (Join-Path 'updater-evidence' $Relative)
            New-Item -ItemType Directory -Force (Split-Path $Destination) | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $Destination
        }
    }
    # Graceful stop of only our fixture service; never kill GUI/helper/installer.
    if (-not $Service.HasExited) {
        & python3 -c 'import os,ssl,urllib.request; c=ssl.create_default_context(cafile=os.environ["STUDIO_GUI_FIXTURE_CA"]); r=urllib.request.Request("https://localhost:38443/_fixture/shutdown",data=b"",headers={"X-Fixture-Token":os.environ["STUDIO_GUI_FIXTURE_TOKEN"]}); urllib.request.urlopen(r,context=c).close()'
        if ($LASTEXITCODE -ne 0 -or -not $Service.WaitForExit(5000)) { throw 'Fixture service did not stop normally.' }
    }
    if ($Service.ExitCode -ne 0) { throw 'Fixture service failed.' }
}
$SuccessResult | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8NoBOM (Join-Path $Evidence 'result.json')
Write-Output 'Isolated snapshot actual GUI updater end-to-end passed; public exact-binary network validation remains separate.'
