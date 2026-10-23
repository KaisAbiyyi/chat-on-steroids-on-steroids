function Assert-SafeDDrivePath {
    param([Parameter(Mandatory=$true)][string]$Path)
    $full = [IO.Path]::GetFullPath($Path)
    if ([IO.Path]::GetPathRoot($full) -ine 'D:\') { throw "D: storage required: $full" }
    $cursor = 'D:\'
    $components = @('') + @($full.Substring(3) -split '[\\/]')
    foreach ($part in $components) {
        if ($part) { $cursor = Join-Path $cursor $part }
        $existing = Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue
        if ($existing -and (($existing.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
            throw "Reparse-point D: ancestor is not an owned storage boundary: $cursor"
        }
    }
}
