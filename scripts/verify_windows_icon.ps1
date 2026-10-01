param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [string]$Shortcut
)
$ErrorActionPreference = 'Stop'
$ExecutablePath = (Resolve-Path $Executable).Path
if (-not ('StudioIconResource' -as [type])) {
    Add-Type @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
using System.Text;
[ComImport, Guid("00021401-0000-0000-C000-000000000046")]
class StudioShellLink {}
[ComImport, Guid("000214F9-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IStudioShellLinkW {
    void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder path, int count, IntPtr findData, uint flags);
    void GetIDList(out IntPtr idList);
    void SetIDList(IntPtr idList);
    void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder text, int count);
    void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string text);
    void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder text, int count);
    void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string text);
    void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder text, int count);
    void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string text);
    void GetHotkey(out short hotkey);
    void SetHotkey(short hotkey);
    void GetShowCmd(out int command);
    void SetShowCmd(int command);
    void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder path, int count, out int index);
}
public static class StudioIconResource {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern uint GetLongPathName(string path, StringBuilder buffer, uint size);
    static string CanonicalPath(string path) {
        if (String.IsNullOrWhiteSpace(path) || !File.Exists(path))
            throw new Exception("Shortcut path does not identify an existing file: " + path);
        var buffer = new StringBuilder(32768);
        uint length = GetLongPathName(Path.GetFullPath(path), buffer, (uint)buffer.Capacity);
        if (length == 0 || length >= buffer.Capacity)
            throw new Exception("Cannot normalize shortcut path: " + path);
        return buffer.ToString();
    }
    public static void VerifyShortcut(string shortcut, string executable) {
        // Read the Unicode Shell interface directly, without WScript path conversion
        // or Resolve() search/repair. Normalize only existing long/8.3 path aliases.
        object link = new StudioShellLink();
        try {
            ((IPersistFile)link).Load(shortcut, 0);
            var shell = (IStudioShellLinkW)link;
            var target = new StringBuilder(32768);
            var icon = new StringBuilder(32768);
            shell.GetPath(target, target.Capacity, IntPtr.Zero, 0);
            int index;
            shell.GetIconLocation(icon, icon.Capacity, out index);
            string expected = CanonicalPath(executable);
            if (!String.Equals(CanonicalPath(target.ToString()), expected, StringComparison.OrdinalIgnoreCase))
                throw new Exception("Shortcut targets the wrong executable: actual=" + target + "; expected=" + executable);
            if (index != 0 || !String.Equals(CanonicalPath(icon.ToString()), expected, StringComparison.OrdinalIgnoreCase))
                throw new Exception("Unexpected shortcut icon: " + icon + "," + index);
        } finally { Marshal.FinalReleaseComObject(link); }
    }
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
    [StudioIconResource]::VerifyShortcut($ShortcutPath, $ExecutablePath)
}
Write-Output 'Windows product icon: PASS'
