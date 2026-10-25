# Source-matched, locally authorized synthetic WebView2 native qualifier.
# Cloud CI compiles artifacts but never asserts physical 8L1 acceptance.
param(
  [Parameter(Mandatory=$true)][string]$Repo,
  [Parameter(Mandatory=$true)][string]$ArtifactRoot,
  [Parameter(Mandatory=$true)][string]$Out,
  [switch]$VerifyRestart,
  [switch]$CrashProbe
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
# Each launch creates a NEW OS host process and Rust child, checked by PID
# and source/binary identities. Profiles are independently synthetic; this
# is a clean restart smoke check, not an OS crash or profile persistence claim.
$receipts = @()
$rounds = if ($VerifyRestart) { @('before-restart','after-restart') } else { @('single') }
foreach ($round in $rounds) {
  $destination = if ($VerifyRestart) { Join-Path $Out $round } else { $Out }
  $hostArgs = @($hostDll, '--8l1', '--repo', $Repo, '--bridge', $rustExe,
    '--artifact-root', $ArtifactRoot, '--out', $destination)
  if ($CrashProbe) { $hostArgs += '--8l1-crash' }
  dotnet @hostArgs
  $code = $LASTEXITCODE
  $receiptPath = Join-Path $destination 'receipt.json'
  if (-not (Test-Path -LiteralPath $receiptPath)) {
    throw "Native host did not return receipt for $round"
  }
  $receipt = Get-Content -Raw -LiteralPath $receiptPath | ConvertFrom-Json
  $receipts += $receipt
  if ($CrashProbe -and $receipt.checks.native_browser_process_failed.status -ne 'PASS') {
    throw "Real browser crash failed to retire all scopes in $round"
  }
  if ($code -ne 0 -or $receipt.rustProcessExit -ne 0 -or
      $receipt.sourceSha -ne $expected -or $receipt.native8L1Accepted -ne $false) {
    throw "8L1-HOST bounded check failed during $round; inspect $receiptPath"
  }
}
if ($VerifyRestart) {
  $first = $receipts[0]
  $second = $receipts[1]
  $fresh = $first.hostProcessId -ne $second.hostProcessId -and
    $first.rustProcessId -ne $second.rustProcessId -and
    $first.hostProcessId -gt 0 -and $second.hostProcessId -gt 0 -and
    $first.rustProcessId -gt 0 -and $second.rustProcessId -gt 0
  $sameBinaries = $first.hostBinarySha256 -eq $second.hostBinarySha256 -and
    $first.rustBinarySha256 -eq $second.rustBinarySha256
  $result = [pscustomobject]@{
    sourceSha = $expected
    cleanProcessRelaunch = if ($fresh -and $sameBinaries) { 'PASS' } else { 'FAIL' }
    physical8L1Accepted = $false
    osCrashQualification = if ($CrashProbe) { 'PASS' } else { 'NOT_RUN' }
    retainedProfileQualification = 'NOT_RUN'
    firstHostPid = $first.hostProcessId
    secondHostPid = $second.hostProcessId
    firstRustPid = $first.rustProcessId
    secondRustPid = $second.rustProcessId
    sameManifestedBinaries = $sameBinaries
  }
  $result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Out 'restart-receipt.json')
  if (-not $fresh -or -not $sameBinaries) { throw 'Native process relaunch identity failed' }
}
Write-Output '8L1-HOST physical checks recorded. Full 8L1 owner permission interaction and persistent profile recovery still require separate acceptance.'
