# GUI-subsystem executables must be waited on explicitly, not invoked with &.
function Invoke-StudioProbe([string]$Binary, [string[]]$Arguments) {
    $Start = [System.Diagnostics.ProcessStartInfo]::new()
    $Start.FileName = (Resolve-Path -LiteralPath $Binary).Path
    $Start.UseShellExecute = $false
    $Start.CreateNoWindow = $true
    $Start.RedirectStandardOutput = $true
    $Start.RedirectStandardError = $true
    foreach ($Argument in $Arguments) { $Start.ArgumentList.Add($Argument) }
    $Process = [System.Diagnostics.Process]::new()
    $Process.StartInfo = $Start
    try {
        if (-not $Process.Start()) { throw 'Could not start Studio probe.' }
        # Drain both streams concurrently so neither full pipe can deadlock.
        $Output = $Process.StandardOutput.ReadToEndAsync()
        $ErrorOutput = $Process.StandardError.ReadToEndAsync()
        if (-not $Process.WaitForExit(30000)) {
            $Process.Kill($true)
            $Process.WaitForExit()
            throw 'Studio probe timed out.'
        }
        $Actual = $Output.GetAwaiter().GetResult()
        $Diagnostic = $ErrorOutput.GetAwaiter().GetResult()
        if ($Process.ExitCode -ne 0) {
            throw "Studio probe exited with $($Process.ExitCode): $Diagnostic"
        }
        return $Actual.TrimEnd("`r", "`n")
    } finally {
        $Process.Dispose()
    }
}
