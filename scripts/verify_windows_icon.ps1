param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [string]$Shortcut
)
$ErrorActionPreference = 'Stop'
$ExecutablePath = (Resolve-Path $Executable).Path
if (-not ('StudioIconResource' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class StudioIconResource {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr LoadLibraryEx(string path, IntPtr file, uint flags);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr FindResource(IntPtr module, IntPtr name, IntPtr type);
    [DllImport("kernel32.dll")] static extern uint SizeofResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll")] static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll")] static extern IntPtr LockResource(IntPtr resource);
    [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
    public static void Verify(string path) {
        var module = LoadLibraryEx(path, IntPtr.Zero, 2);
        if (module == IntPtr.Zero) throw new Exception("Cannot read executable resources");
        try {
            var resource = FindResource(module, new IntPtr(1), new IntPtr(14));
            if (resource == IntPtr.Zero) throw new Exception("Missing product icon resource");
            var length = SizeofResource(module, resource);
            var bytes = new byte[checked((int)length)];
            var data = LockResource(LoadResource(module, resource));
            if (data == IntPtr.Zero) throw new Exception("Cannot load icon group");
            Marshal.Copy(data, bytes, 0, bytes.Length);
            if (bytes.Length < 6 || BitConverter.ToUInt16(bytes, 2) != 1)
                throw new Exception("Invalid icon group");
            var count = BitConverter.ToUInt16(bytes, 4);
            if (count != 7 || bytes.Length < 6 + count * 14)
                throw new Exception("Missing multi-resolution product icons");
            int[] sizes = {16, 24, 32, 48, 64, 128, 256};
            for (int i = 0; i < count; i++) {
                int offset = 6 + i * 14;
                int expected = sizes[i] == 256 ? 0 : sizes[i];
                if (bytes[offset] != expected || bytes[offset + 1] != expected)
                    throw new Exception("Unexpected icon dimensions");
                var id = BitConverter.ToUInt16(bytes, offset + 12);
                var image = FindResource(module, new IntPtr(id), new IntPtr(3));
                if (image == IntPtr.Zero || SizeofResource(module, image) == 0)
                    throw new Exception("Missing icon image");
            }
        } finally { FreeLibrary(module); }
    }
}
'@
}
[StudioIconResource]::Verify($ExecutablePath)
if ($Shortcut) {
    $ShortcutPath = (Resolve-Path $Shortcut).Path
    $Link = (New-Object -ComObject WScript.Shell).CreateShortcut($ShortcutPath)
    if ($Link.TargetPath -ne $ExecutablePath) { throw 'Shortcut targets the wrong executable' }
    if ($Link.IconLocation -ne "$ExecutablePath,0") { throw "Unexpected shortcut icon: $($Link.IconLocation)" }
}
Write-Output 'Windows product icon: PASS'
