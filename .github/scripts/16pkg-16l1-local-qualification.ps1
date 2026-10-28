#requires -Version 7.0
# Owner/agent physical Windows 16L1 handoff. Never run automatically from CI/cloud.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][long]$RunId,
    [Parameter(Mandatory)][string]$SourceSha,
    [string]$Workspace = "D:\projects\CoS-Rust\qualification\16L1-$SourceSha",
    [switch]$Execute
)
$ErrorActionPreference = 'Stop'
$repository='KaisAbiyyi/chat-on-steroids-on-steroids'
$workflow="$repository/.github/workflows/16pkg-native-installer.yml"
if ($SourceSha -cnotmatch '^[a-f0-9]{40}$') { throw 'Full exact source SHA required' }
if (!(Test-Path -LiteralPath 'D:\projects\CoS-Rust')) { throw 'D: CoS-Rust workspace unavailable' }
$workspacePath=[IO.Path]::GetFullPath($Workspace).TrimEnd('\')
$allowedPrefix='D:\projects\CoS-Rust\qualification\'
if (!$workspacePath.StartsWith($allowedPrefix,[StringComparison]::OrdinalIgnoreCase) -or
    $workspacePath.Contains('..') -or $workspacePath.Length -lt ($allowedPrefix.Length+20)) {
    throw 'Refusing workspace outside isolated D: qualification root'
}
# Verification-only preflight may be resumed once for an explicit execution pass.
$existing = Test-Path -LiteralPath $workspacePath
if ($existing -and (!$Execute -or (Test-Path -LiteralPath (Join-Path $workspacePath 'evidence')) -or
    !(Test-Path -LiteralPath (Join-Path $workspacePath 'downloads/package-receipt.json')))) {
    throw 'Workspace already used or incomplete; preserve evidence and select a new workspace'
}
if ($Execute) {
    $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
    $principal=[Security.Principal.WindowsPrincipal]::new($identity)
    if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run a separately elevated PowerShell session after explicitly approving this isolated test'
    }
}
foreach ($tool in @('gh','git')) {
    if (!(Get-Command $tool -ErrorAction SilentlyContinue)) { throw "Missing required $tool CLI" }
}
$run=gh run view $RunId --repo $repository --json conclusion,event,workflowName | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $run.conclusion -ne 'success') {
    throw 'GitHub run is not successful'
}
if (!$existing) {
    New-Item -ItemType Directory -Path $workspacePath | Out-Null
}
$download=Join-Path $workspacePath 'downloads'
if (!$existing) {
    New-Item -ItemType Directory -Path $download | Out-Null
    $artifact="cosos-16pkg-unsigned-candidate-$SourceSha"
    gh run download $RunId --repo $repository --name $artifact --dir $download
    if ($LASTEXITCODE -ne 0) { throw 'Exact-run source artifact download failed' }
}
$receiptPath=Join-Path $download 'package-receipt.json'
$setup=Join-Path $download 'installer-package\CoSoSInstaller.exe'
$archive=Join-Path $download 'candidate.zip'
if (!(Test-Path -LiteralPath $receiptPath) -or !(Test-Path -LiteralPath $setup)) {
    throw 'Incomplete downloaded installer or receipt'
}
foreach ($file in @($receiptPath,$setup,$archive,
    (Join-Path $download 'installer-unsigned-0.2.0\CoSoSInstaller.exe'),
    (Join-Path $download 'installer-unsigned-0.3.0\CoSoSInstaller.exe'))) {
    gh attestation verify $file --repo $repository --signer-workflow $workflow
    if ($LASTEXITCODE -ne 0) { throw "Independent attestation verification failed: $file" }
}
$receipt=Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if ($receipt.sourceSha -ne $SourceSha) { throw 'Attested source mismatch' }
function RequireDigest([string]$file,[string]$expected) {
    if ($expected -notmatch '^[a-f0-9]{64}$' -or
        (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expected) {
        throw "Independent digest mismatch: $file"
    }
}
RequireDigest $setup $receipt.setupSha256
RequireDigest $archive $receipt.zipSha256
$up2=Join-Path $download 'installer-unsigned-0.2.0\CoSoSInstaller.exe'
$up3=Join-Path $download 'installer-unsigned-0.3.0\CoSoSInstaller.exe'
RequireDigest $up2 $receipt.update02SetupSha256
RequireDigest $up3 $receipt.update03SetupSha256
Write-Host "VERIFIED: GitHub workflow, 3 setup attestations, archive and exact source $SourceSha"
Write-Host "All packages are UNSIGNED. Windows may show Unknown Publisher/SmartScreen. Keep security enabled."
Write-Host 'Do not import TEST certificates or grant access to existing CoS/CoSoS profiles.'
if (!$Execute) {
    Write-Host "Preflight complete. Run again with -Execute in an elevated PowerShell 7 shell to qualify disposable $workspacePath."
    Write-Host 'Use -Execute with the SAME -RunId, -SourceSha and -Workspace to resume this verified preflight.'
    return
}
$root=Join-Path $workspacePath 'installed'
$evidence=Join-Path $workspacePath 'evidence'
New-Item -ItemType Directory -Path $evidence | Out-Null
if (Test-Path -LiteralPath $root) { throw 'Existing installation root refused' }
function Invoke-Setup([string]$bin,[string]$op,[string]$hash) {
    & $bin $op $root --unsigned-sha256 $hash
    if ($LASTEXITCODE -ne 0) { throw "Physical $op FAIL, exit=$LASTEXITCODE" }
}
$results=[ordered]@{}
try {
    Invoke-Setup $setup '--install' $receipt.setupSha256
    $results['install']='PASS'
    $state=Get-Content (Join-Path $root 'active.json') -Raw | ConvertFrom-Json
    $slot=Join-Path $root "slots\$($state.Active)"
    $runtime=Join-Path $slot 'bin\cos-rust.exe'
    $hostExe=Join-Path $slot 'renderer\WindowsRenderer.exe'
    & $runtime --data-dir (Join-Path $workspacePath 'disposable-profile') self-test
    if ($LASTEXITCODE -ne 0) { throw 'Installed Rust runtime smoke FAIL' }
    & $hostExe --contract-selftest
    if ($LASTEXITCODE -ne 0) { throw 'Installed WebView2 host smoke FAIL' }
    $results['rustAndWebView2Smoke']='PASS'
    Invoke-Setup $up2 '--update' $receipt.update02SetupSha256
    $results['upgrade']='PASS'
    Invoke-Setup $up2 '--rollback' $receipt.update02SetupSha256
    $results['rollback']='PASS'
    Invoke-Setup $up3 '--update' $receipt.update03SetupSha256
    $results['reUpdate']='PASS'
    Invoke-Setup $up3 '--recover' $receipt.update03SetupSha256
    $results['recovery']='PASS'
    Invoke-Setup $up3 '--finalize' $receipt.update03SetupSha256
    $results['finalize']='PASS'
    Invoke-Setup $up3 '--uninstall' $receipt.update03SetupSha256
    if (Test-Path -LiteralPath $root) { throw 'Uninstall left active installation' }
    $results['uninstall']='PASS'
} finally {
    @{
      schema='cosos.16l1.physical.unsigned.v1'
      sourceSha=$SourceSha
      runId=$RunId
      windowsVersion=[Environment]::OSVersion.VersionString
      downloadedInstallerSha256=$receipt.setupSha256
      results=$results
      physicalAcceptance='PENDING_OWNER_REVIEW'
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence 'qualification.json') -Encoding utf8
    Write-Host "Evidence: $evidence\qualification.json"
}
Write-Host 'Physical lifecycle completed; review receipt, Windows prompts, ACL and profile isolation before accepting 16L1.'
Write-Host "Cleanup: inspect the exact isolated workspace, then Remove-Item -LiteralPath '$workspacePath' -Recurse -Force"
