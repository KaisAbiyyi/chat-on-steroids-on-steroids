param(
    [Parameter(Mandatory=$true)][string]$PayloadZip,
    [string]$PayloadSha256,
    [string]$Destination = $(if ($env:COS_7PL1_INSTALL_ROOT) { $env:COS_7PL1_INSTALL_ROOT } else { 'D:\Apps\CoSoS-7PL1-Synthetic-Probe' })
)
$ErrorActionPreference = 'Stop'
$Destination = [IO.Path]::GetFullPath($Destination)
if ([IO.Path]::GetPathRoot($Destination) -ne 'D:\') { throw '7P-L1 installer requires D: destination.' }
if (!(Test-Path -LiteralPath 'D:\')) { throw 'Required D: storage is not mounted.' }
if (!(Test-Path -LiteralPath $PayloadZip -PathType Leaf)) { throw 'Missing installer payload.' }
if (Test-Path -LiteralPath $Destination) { throw "Destination already exists: $Destination. Uninstall first; no overwrite is allowed." }
if ($PayloadSha256) {
    if (!(Test-Path -LiteralPath $PayloadSha256 -PathType Leaf)) { throw 'Missing checksum sidecar.' }
    $expected = (Get-Content -LiteralPath $PayloadSha256 -Raw).Trim().Split(' ')[0].ToUpperInvariant()
    $actual = (Get-FileHash -LiteralPath $PayloadZip -Algorithm SHA256).Hash
    if ($expected -ne $actual) { throw 'Payload SHA256 mismatch; refusing install.' }
}
$parent = Split-Path $Destination -Parent
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$staging = Join-Path $parent ('CoSoS-7PL1-STAGE-' + [guid]::NewGuid().ToString('N'))
try {
    Expand-Archive -LiteralPath $PayloadZip -DestinationPath $staging -ErrorAction Stop
    $manifest = Join-Path $staging 'probe-files.sha256'
    if (!(Test-Path -LiteralPath $manifest)) { throw 'Missing own-file manifest.' }
    $lines = @(Get-Content -LiteralPath $manifest | Where-Object { $_.Trim().Length -gt 0 })
    if ($lines.Count -eq 0) { throw 'Empty payload manifest.' }
    foreach ($line in $lines) {
        $parts = $line.Split(' ',2)
        if ($parts.Count -ne 2) { throw 'Malformed manifest line.' }
        $rel = $parts[1].Trim().Replace('/', '\')
        $item = [IO.Path]::GetFullPath((Join-Path $staging $rel))
        if (-not $item.StartsWith($staging + '\',[StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath $item -PathType Leaf)) { throw "Manifest file escapes payload or missing: $rel" }
        if ((Get-FileHash -LiteralPath $item -Algorithm SHA256).Hash -ne $parts[0].ToUpperInvariant()) { throw "File hash mismatch: $rel" }
    }
    Move-Item -LiteralPath $staging -Destination $Destination
    Write-Output "INSTALLED synthetic 7P-L1 probe at $Destination with $($lines.Count) verified files (NOT production)."
} finally {
    if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging -Recurse -Force }
}
