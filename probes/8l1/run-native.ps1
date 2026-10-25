# Source-matched, locally authorized synthetic WebView2 native qualifier.
# Cloud CI compiles artifacts but never asserts physical 8L1 acceptance.
param(
  [Parameter(Mandatory=$true)][string]$Repo,
  [Parameter(Mandatory=$true)][string]$ArtifactRoot,
  [Parameter(Mandatory=$true)][string]$Out,
  [switch]$VerifyRestart,
  [switch]$CrashProbe,
  [switch]$ConsentProbe,
  [switch]$ProfileActionsProbe
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
# Each launch creates a NEW OS host process and Rust child; restart uses
# the EXACT SAME browser-owned profile directories and packaged extension,
# never live user profiles. Compare independent browser-local storage markers,
# extension identity and fresh Rust attestations across processes.
if ($ProfileActionsProbe) {
  Write-Output 'Physical owner sequence: choose Yes for synthetic provider sign-in, then Yes for cross-profile logical session handoff. This does NOT authenticate a real provider or copy browser credentials.'
}
if ($ConsentProbe) {
  Write-Output 'Physical owner sequence: choose Allow (Yes), then Deny (No), then Allow (Yes) on the stale-navigation prompt. The replayed approval will be denied without a dialog.'
}
$receipts = @()
$profileRoot = Assert-OwnedPath (Join-Path $Out 'retained-profiles')
$rounds = if ($VerifyRestart) { @('before-restart','after-restart') } else { @('single') }
foreach ($round in $rounds) {
  $destination = if ($VerifyRestart) { Join-Path $Out $round } else { $Out }
  $hostArgs = @($hostDll, '--8l1', '--repo', $Repo, '--bridge', $rustExe,
    '--artifact-root', $ArtifactRoot, '--out', $destination,
    '--profile-root', $profileRoot)
  if ($round -eq 'after-restart') { $hostArgs += '--reuse-8l1-profile' }
  if ($CrashProbe) { $hostArgs += '--8l1-crash' }
  if ($ConsentProbe) { $hostArgs += '--8l1-consent' }
  if ($ProfileActionsProbe) { $hostArgs += '--8l1-profile-actions' }
  dotnet @hostArgs
  $code = $LASTEXITCODE
  $receiptPath = Join-Path $destination 'receipt.json'
  if (-not (Test-Path -LiteralPath $receiptPath)) {
    throw "Native host did not return receipt for $round"
  }
  $receipt = Get-Content -Raw -LiteralPath $receiptPath | ConvertFrom-Json
  $receipts += $receipt
  if ($ConsentProbe) {
    foreach ($checkName in @(
      'native_permission_positive_consent',
      'native_permission_replay_event_denied',
      'native_permission_stale_event_denied',
      'native_permission_recovered_new_channel'
    )) {
      if ($receipt.checks.$checkName.status -ne 'PASS') {
        throw "Actual WebView2 permission $checkName failed in $round"
      }
    }
  }
  if ($ProfileActionsProbe) {
    foreach ($checkName in @(
      'native_8b_signin_navigation',
      'native_8b_signin_rebind',
      'native_8b_source_reattested',
      'native_8b_session_handoff_atomic',
      'native_8b_handoff_replay_denied',
      'native_8b_handoff_fresh_observation'
    )) {
      if ($receipt.checks.$checkName.status -ne 'PASS') {
        throw "Actual native 8B profile action $checkName failed in $round"
      }
    }
  }
  if ($CrashProbe -and $receipt.checks.native_browser_process_failed.status -ne 'PASS') {
    throw "Real browser crash failed to retire all scopes in $round"
  }
  if ($code -ne 0 -or $receipt.rustProcessExit -ne 0 -or
      $receipt.sourceSha -ne $expected -or $receipt.native8L1Accepted -ne $false -or
      $receipt.checks.native_profile_storage_proof.status -ne 'PASS' -or
      $receipt.checks.native_rust_mv3_authenticated.status -ne 'PASS') {
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
  $sameProfiles = $first.retainedProfileMode -eq $true -and
    $first.reusedRetainedProfiles -eq $false -and
    $second.reusedRetainedProfiles -eq $true -and
    $first.profileAPath -eq $second.profileAPath -and
    $first.profileBPath -eq $second.profileBPath -and
    $first.profileAPath -ne $first.profileBPath -and
    $first.extensionAId -eq $second.extensionAId -and
    $first.extensionBId -eq $second.extensionBId -and
    $first.syntheticMarkerA -eq $second.syntheticMarkerA -and
    $first.syntheticMarkerB -eq $second.syntheticMarkerB -and
    -not [string]::IsNullOrWhiteSpace($second.syntheticMarkerA) -and
    -not [string]::IsNullOrWhiteSpace($second.syntheticMarkerB) -and
    $second.checks.native_rust_mv3_authenticated.status -eq 'PASS'
  $result = [pscustomobject]@{
    sourceSha = $expected
    cleanProcessRelaunch = if ($fresh -and $sameBinaries) { 'PASS' } else { 'FAIL' }
    retainedProfileAndRustRecovery = if ($fresh -and $sameBinaries -and $sameProfiles) { 'PASS' } else { 'FAIL' }
    physical8L1Accepted = $false
    osCrashQualification = if ($CrashProbe) { 'PASS' } else { 'NOT_RUN' }
    retainedProfileQualification = if ($sameProfiles) { 'PASS' } else { 'FAIL' }
    firstHostPid = $first.hostProcessId
    secondHostPid = $second.hostProcessId
    firstRustPid = $first.rustProcessId
    secondRustPid = $second.rustProcessId
    sameManifestedBinaries = $sameBinaries
  }
  $result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Out 'restart-receipt.json')
  if (-not $fresh -or -not $sameBinaries -or -not $sameProfiles) {
    throw 'Native profile/channel recovery failed after process restart'
  }
}
Write-Output '8L1-HOST physical checks recorded. Restart requires the SAME persisted WebView2 profiles, new Rust authority and fresh authenticated channels. Full 8L1 acceptance remains separate.'
