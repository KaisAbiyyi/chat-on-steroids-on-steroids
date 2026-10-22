param([string]$Destination = $(if ($env:COS_7PL1_INSTALL_ROOT) { $env:COS_7PL1_INSTALL_ROOT } else { 'D:\Apps\CoSoS-7PL1-Synthetic-Probe' }))
$ErrorActionPreference = 'Stop'
$Destination = [IO.Path]::GetFullPath($Destination)
if ([IO.Path]::GetPathRoot($Destination) -ne 'D:\') { throw '7P-L1 uninstaller requires D: destination.' }
$Destination = $Destination.TrimEnd('\')
if ($Destination -eq 'D:') { throw 'Refusing to uninstall the drive root.' }
$manifest = Join-Path $Destination 'probe-files.sha256'
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
    $files += [pscustomobject]@{Path=$full;Hash=$parts[0].ToUpperInvariant()}
}
# Delete only packaged, unmodified files. Preserve unknown files, user data,
# modified files and profile roots.
$preservedModified = 0
foreach ($item in $files) {
    if (Test-Path -LiteralPath $item.Path -PathType Leaf) {
        if ((Get-FileHash -LiteralPath $item.Path -Algorithm SHA256).Hash -eq $item.Hash) {
            Remove-Item -LiteralPath $item.Path -Force
        } else { $preservedModified++ }
    }
}
Remove-Item -LiteralPath $manifest -Force
Get-ChildItem -LiteralPath $Destination -Directory -Recurse -Force | Sort-Object { $_.FullName.Length } -Descending | ForEach-Object { if (@(Get-ChildItem -LiteralPath $_.FullName -Force).Count -eq 0) { Remove-Item -LiteralPath $_.FullName -Force } }
if (@(Get-ChildItem -LiteralPath $Destination -Force).Count -eq 0) { Remove-Item -LiteralPath $Destination -Force }
Write-Output "UNINSTALLED owned 7P-L1 files from $Destination. Preserved $preservedModified modified files and any unowned files."
