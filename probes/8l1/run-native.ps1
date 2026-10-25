# Source-matched, locally authorized synthetic WebView2 native qualifier.
# Cloud CI compiles artifacts but never asserts physical 8L1 acceptance.
param(
  [Parameter(Mandatory=$true)][string]$Repo,
  [Parameter(Mandatory=$true)][string]$ArtifactRoot,
  [Parameter(Mandatory=$true)][string]$Out
)
$ErrorActionPreference = 'Stop'
function Assert-OwnedPath([string]$path) {
  $full = [IO.Path]::GetFullPath($path)
  if (-not $full.StartsWith('D:\projects\CoS-Rust\', [StringComparison]::OrdinalIgnoreCase)) {
    throw '8L1 must use D:\projects\CoS-Rust and an isolated synthetic profile'
  }
  $cursor = $full
  while ($cursor) {
    if (Test-Path -LiteralPath $cursor) {
      $item = Get-Item -LiteralPath $cursor -Force
      if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'Reparse/junction path denied'
      }
    }
    $parent = [IO.Path]::GetDirectoryName($cursor)
    if (-not $parent -or $parent -eq $cursor) { break }
    $cursor = $parent
  }
  return $full
}
$Repo = Assert-OwnedPath $Repo
$ArtifactRoot = Assert-OwnedPath $ArtifactRoot
$Out = Assert-OwnedPath $Out
if ((Test-Path -LiteralPath $Out) -and @(Get-ChildItem -LiteralPath $Out -Force).Count) {
  throw 'Output must be new/empty; do not overwrite earlier physical evidence'
}
$actual = (git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Git rev-parse failure' }
$expected = (Get-Content -Raw -LiteralPath (Join-Path $ArtifactRoot 'source.sha')).Trim()
if ($actual -ne $expected -or $expected -notmatch '^[a-f0-9]{40}$') {
  throw 'Source mismatch with packaged artifact'
}
$hostDll = Join-Path $ArtifactRoot 'host\WindowsHost.dll'
$rustExe = Join-Path $ArtifactRoot '7l2_bridge.exe'
if (-not (Test-Path -LiteralPath $hostDll) -or -not (Test-Path -LiteralPath $rustExe)) {
  throw 'Missing source-bound product Windows host or authenticated Rust child'
}
dotnet $hostDll --8l1 --repo $Repo --bridge $rustExe --artifact-root $ArtifactRoot --out $Out
$code = $LASTEXITCODE
$receipt = Join-Path $Out 'receipt.json'
if (-not (Test-Path -LiteralPath $receipt)) { throw 'Native host did not return receipt' }
Get-Content -Raw -LiteralPath $receipt
if ($code -ne 0) { throw "8L1-HOST bounded check failed with exit $code" }
Write-Output 'Bounded host test only. Full physical 8L1 remains NOT_RUN; see receipt pending checks.'
