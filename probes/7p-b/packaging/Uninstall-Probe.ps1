param([string]$Destination = $(if ($env:COS_7PL1_INSTALL_ROOT) { $env:COS_7PL1_INSTALL_ROOT } else { 'D:\Apps\CoSoS-7PL1-Synthetic-Probe' }))
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Assert-SafeDDrivePath.ps1')
$Destination = [IO.Path]::GetFullPath($Destination)
Assert-SafeDDrivePath $Destination
$Destination = $Destination.TrimEnd('\')
if ($Destination -eq 'D:') { throw 'Refusing to uninstall the drive root.' }
$manifest = Join-Path $Destination 'probe-files.sha256'
Assert-SafeDDrivePath $manifest
if (!(Test-Path -LiteralPath $manifest -PathType Leaf)) { throw 'Not an owned 7P-L1 probe installation. Refusing deletion.' }
$lines = @(Get-Content -LiteralPath $manifest | Where-Object { $_.Trim().Length -gt 0 })
if ($lines.Count -eq 0) { throw 'Empty ownership manifest. Refusing deletion.' }
$files = @()
foreach ($line in $lines) {
    $parts = $line.Split(' ',2)
    if ($parts.Count -ne 2) { throw 'Malformed ownership manifest.' }
    $rel = $parts[1].Trim().Replace('/', '\')
    $full = [IO.Path]::GetFullPath((Join-Path $Destination $rel))
    if (!$full.StartsWith($Destination + '\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Ownership manifest escapes installation root.' }
    Assert-SafeDDrivePath $full
    $files += [pscustomobject]@{Path=$full;Hash=$parts[0].ToUpperInvariant()}
}
# Delete only packaged, unmodified files. Preserve unknown files, user data,
# modified files and profile roots.
$preservedModified = 0
foreach ($item in $files) {
    Assert-SafeDDrivePath $item.Path
    if (Test-Path -LiteralPath $item.Path -PathType Leaf) {
        if ((Get-FileHash -LiteralPath $item.Path -Algorithm SHA256).Hash -eq $item.Hash) {
            Remove-Item -LiteralPath $item.Path -Force
        } else { $preservedModified++ }
    }
}
Assert-SafeDDrivePath $manifest
Remove-Item -LiteralPath $manifest -Force
# Only consider manifest-owned directories. Traversing unknown recursive trees
# could follow an unowned junction added inside the installation.
$dirs = @($files | ForEach-Object { Split-Path $_.Path -Parent } | Sort-Object -Unique)
$dirs = @($dirs | Where-Object { $_.StartsWith($Destination + '\',[StringComparison]::OrdinalIgnoreCase) } | Sort-Object Length -Descending)
foreach ($dir in $dirs) {
    Assert-SafeDDrivePath $dir
    if ((Test-Path -LiteralPath $dir -PathType Container) -and @(Get-ChildItem -LiteralPath $dir -Force).Count -eq 0) {
        Remove-Item -LiteralPath $dir -Force
    }
}
if (@(Get-ChildItem -LiteralPath $Destination -Force).Count -eq 0) { Remove-Item -LiteralPath $Destination -Force }
Write-Output "UNINSTALLED owned 7P-L1 files from $Destination. Preserved $preservedModified modified files and any unowned files."
