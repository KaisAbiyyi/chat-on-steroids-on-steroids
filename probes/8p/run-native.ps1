# Owner-operated isolated D: native navigation only; never signs in or records credentials.
param(
  [Parameter(Mandatory=$true)][string]$Repo,
  [Parameter(Mandatory=$true)][string]$ArtifactRoot,
  [Parameter(Mandatory=$true)][string]$Out,
  [Parameter(Mandatory=$true)][string]$ProviderConfig,
  [Parameter(Mandatory=$true)][string]$ProviderId
)
$ErrorActionPreference = 'Stop'
function Assert-DPath([string]$path) {
  $full = [IO.Path]::GetFullPath($path)
  if (-not $full.StartsWith('D:\projects\CoS-Rust\', [StringComparison]::OrdinalIgnoreCase)) {
    throw '8P paths must be isolated under D:\projects\CoS-Rust'
  }
  $cursor = $full
  while ($cursor) {
    if (Test-Path -LiteralPath $cursor) {
      $item = Get-Item -LiteralPath $cursor -Force
      if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'Reparse/junction refused'
      }
    }
    $parent = [IO.Path]::GetDirectoryName($cursor)
    if (-not $parent -or $parent -eq $cursor) { break }
    $cursor = $parent
  }
  return $full
}
$Repo = Assert-DPath $Repo
$ArtifactRoot = Assert-DPath $ArtifactRoot
$Out = Assert-DPath $Out
$ProviderConfig = Assert-DPath $ProviderConfig
if (-not (Test-Path -LiteralPath $ProviderConfig -PathType Leaf)) { throw 'Missing host-owned provider config' }
if ((Test-Path -LiteralPath $Out) -and @(Get-ChildItem -LiteralPath $Out -Force).Count) {
  throw '8P profile/output must be new and empty'
}
$expected = (Get-Content -Raw -LiteralPath (Join-Path $ArtifactRoot 'source.sha')).Trim()
$actual = (git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $expected -notmatch '^[a-f0-9]{40}$' -or $expected -ne $actual) {
  throw '8P source/checkout mismatch'
}
$manifest = Get-Content -Raw -LiteralPath (Join-Path $ArtifactRoot 'hashes.json') | ConvertFrom-Json
if (@($manifest).Count -lt 5) { throw 'Missing binary manifest entries' }
foreach ($entry in $manifest) {
  $relative = [string]$entry.path
  if ($relative -notmatch '^(host/[a-zA-Z0-9_.\-/]+|7l2_bridge.exe|source.sha)$' -or $relative.Contains('..')) {
    throw 'Unexpected artifact path'
  }
  $candidate = Join-Path $ArtifactRoot $relative
  $digest = (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($digest -ne $entry.sha256) { throw '8P binary manifest mismatch' }
}
$hostDll = Join-Path $ArtifactRoot 'host/WindowsHost.dll'
if (-not (Test-Path -LiteralPath $hostDll)) { throw 'Missing host binary' }
Write-Output 'Owner chooses a configured provider. Native Yes only opens its allowed HTTPS landing origin; it does not grant sign-in/observation/permission.'
dotnet $hostDll --8p --repo $Repo --out $Out --bridge (Join-Path $ArtifactRoot '7l2_bridge.exe') --artifact-root $ArtifactRoot --8p-config $ProviderConfig --8p-provider $ProviderId
if ($LASTEXITCODE -ne 0) { throw '8P host execution failed' }
$receiptPath = Join-Path $Out '8p-navigation-receipt.json'
if (-not (Test-Path -LiteralPath $receiptPath)) {
  throw 'No navigation receipt (owner denial or closed before navigation)'
}
$r = Get-Content -Raw $receiptPath | ConvertFrom-Json
if ($r.sourceSha -ne $expected -or $r.native8PAccepted -ne $false -or
    $r.owner7U1Accepted -ne $false -or $r.rustChannelActive -ne $false -or
    $r.mv3WorkerAttested -ne $false) {
  throw 'Receipt authority boundary violation'
}
Write-Output 'Navigation receipt written; production signed-in gate remains NOT_RUN.'
