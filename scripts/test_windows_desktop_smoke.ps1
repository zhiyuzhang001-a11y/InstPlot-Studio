# Establish that the disposable CI VM can observe actual product windows.
# This is NOT an end-to-end updater test, nor user-machine GUI acceptance.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Disposable Windows GitHub VM only; never run on a user installation.'
}
$Evidence = Join-Path (Split-Path $PSScriptRoot) 'target/windows-desktop-smoke'
New-Item -ItemType Directory -Force $Evidence | Out-Null
$RegistrationName = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}_is1'
foreach ($Hive in @('HKCU:', 'HKLM:', 'HKLM:\Software\WOW6432Node')) {
    $Registration = if ($Hive -like '*WOW6432Node') {
        Join-Path $Hive ($RegistrationName.Substring('Software\'.Length))
    } else { Join-Path $Hive $RegistrationName }
    if (Test-Path -LiteralPath $Registration) { throw 'Existing Studio registration; refusing to overwrite.' }
}
$MenuShortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'InstPlot Studio.lnk'
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'InstPlot Studio.lnk'
if ((Test-Path -LiteralPath $MenuShortcut) -or (Test-Path -LiteralPath $DesktopShortcut)) {
    throw 'Existing Studio shortcut; refusing to overwrite.'
}
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public sealed class StudioWindow {
    public long Handle; public int Pid; public bool Visible; public bool Minimized;
    public bool Cloaked; public bool OnMonitor; public int Left, Top, Right, Bottom;
}
public static class StudioDesktop {
    delegate bool Callback(IntPtr hwnd, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr p);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromRect(ref Rect r, uint flags);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, uint a, out uint value, uint size);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
    public static StudioWindow[] Windows(int processId) {
        var result = new List<StudioWindow>();
        EnumWindows((h,p) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pid != processId) return true;
            Rect r; if (!GetWindowRect(h, out r)) return true;
            uint cloaked; var status = DwmGetWindowAttribute(h,14,out cloaked,4);
            result.Add(new StudioWindow { Handle=h.ToInt64(), Pid=(int)pid,
                Visible=IsWindowVisible(h), Minimized=IsIconic(h),
                Cloaked=status != 0 || cloaked != 0, OnMonitor=MonitorFromRect(ref r,0)!=IntPtr.Zero,
                Left=r.Left, Top=r.Top, Right=r.Right, Bottom=r.Bottom });
            return true;
        },IntPtr.Zero);
        return result.ToArray();
    }
}
'@
$InputDesktop = [StudioDesktop]::OpenInputDesktop(0, $false, 1)
@{ scope = 'desktop-capability-and-ordinary-launch-only'; user_interactive = [Environment]::UserInteractive;
   session_id = [Diagnostics.Process]::GetCurrentProcess().SessionId;
   readable_input_desktop = ($InputDesktop -ne [IntPtr]::Zero);
   package_source = 'd44895d5e0956f3b1e8e862bba1be54e1d2e4c85'; identity = '37938590122-1'
} | ConvertTo-Json | Set-Content -Encoding utf8NoBOM (Join-Path $Evidence 'environment.json')
if ($InputDesktop -eq [IntPtr]::Zero -or -not [Environment]::UserInteractive) {
    if ($InputDesktop -ne [IntPtr]::Zero) { [void][StudioDesktop]::CloseDesktop($InputDesktop) }
    throw 'No interactive input desktop. This VM cannot pass GUI acceptance.'
}
[void][StudioDesktop]::CloseDesktop($InputDesktop)
$TaskRoot = Join-Path $env:RUNNER_TEMP ('studio-desktop-' + [guid]::NewGuid())
New-Item -ItemType Directory $TaskRoot | Out-Null
$InstallRoot = Join-Path $TaskRoot 'installation'
$PublicRoot = 'https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/windows-gui-qa/37938590122-1'
$Packages = @(
    @{ version='0.1.2-rc.2'; sha='8e9a7605ebdc04690db85298e9eb23d597cd1f3379cccf435320819a45d13e0c'; size=13410099 },
    @{ version='0.1.2-rc.3'; sha='b0a6cd0122de75ea5ed039c5133d834053946b8af320f132f8d3363c009ce1fd'; size=13413737 }
)
$Results = @()
foreach ($Package in $Packages) {
    $Version = $Package.version
    $Name = "InstPlot-Studio-$Version-windows-x86_64-setup.exe"
    $Installer = Join-Path $TaskRoot $Name
    Invoke-WebRequest "$PublicRoot/releases/$Version/$Name" -OutFile $Installer
    if ((Get-Item -LiteralPath $Installer).Length -ne $Package.size -or
        (Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Package.sha) {
        throw 'Frozen package size/hash mismatch.'
    }
    $Arguments = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-',
        '/NOCLOSEAPPLICATIONS','/NORESTARTAPPLICATIONS','/TASKS=!desktopicon',
        ('/DIR="' + $InstallRoot + '"'), ('/LOG="' + (Join-Path $Evidence "$Version-install.log") + '"'))
    $Setup = Start-Process -FilePath $Installer -ArgumentList $Arguments -PassThru
    if (-not $Setup.WaitForExit(120000) -or $Setup.ExitCode -ne 0) { throw 'Disposable installation did not finish successfully.' }
    $Registered = Get-ItemProperty "HKCU:\$RegistrationName"
    if ($Registered.DisplayVersion -cne $Version -or $Registered.InstallLocation.TrimEnd('\') -cne $InstallRoot) {
        throw 'Wrong installed version or directory.'
    }
    $Clock = [Diagnostics.Stopwatch]::StartNew()
    $Gui = Start-Process -FilePath (Join-Path $InstallRoot 'instplot-studio.exe') -PassThru `
        -RedirectStandardError (Join-Path $Evidence "$Version-startup.log")
    $Window = $null
    while ($Clock.Elapsed.TotalSeconds -lt 30) {
        if ($Gui.HasExited) { throw "GUI exited early: $($Gui.ExitCode)" }
        $Window = [StudioDesktop]::Windows($Gui.Id) | Where-Object {
            $_.Visible -and -not $_.Minimized -and -not $_.Cloaked -and $_.OnMonitor -and
            ($_.Right - $_.Left) -gt 200 -and ($_.Bottom - $_.Top) -gt 200
        } | Select-Object -First 1
        if ($null -ne $Window) { break }
        Start-Sleep -Milliseconds 200
    }
    [StudioDesktop]::Windows($Gui.Id) | ConvertTo-Json -Depth 3 |
        Set-Content -Encoding utf8NoBOM (Join-Path $Evidence "$Version-windows.json")
    if ($null -eq $Window) { throw 'No visible, non-minimized, uncloaked on-monitor GUI window.' }
    $Bounds = [Windows.Forms.SystemInformation]::VirtualScreen
    $Image = [Drawing.Bitmap]::new($Bounds.Width, $Bounds.Height)
    $Graphics = [Drawing.Graphics]::FromImage($Image)
    try {
        $Graphics.CopyFromScreen($Bounds.Left, $Bounds.Top, 0, 0, $Bounds.Size)
        $Image.Save((Join-Path $Evidence "$Version-desktop.png"), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $Graphics.Dispose(); $Image.Dispose() }
    $Results += @{ version=$Version; pid=$Gui.Id; visible_window=$Window; elapsed_ms=$Clock.ElapsedMilliseconds }
    $Results | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8NoBOM (Join-Path $Evidence 'results.json')
    # Normal close only for our exact owned child; no force-kill or retry spawn.
    if (-not $Gui.CloseMainWindow() -or -not $Gui.WaitForExit(20000)) { throw 'Owned GUI did not close normally.' }
}
Write-Output 'Real desktop ordinary-launch smoke passed. Updater end-to-end remains untested.'
