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
. "$PSScriptRoot/16pkg-16l1-guards.ps1"
$workspacePath=[IO.Path]::GetFullPath($Workspace).TrimEnd('\\')
$boundary='D:\\projects\\CoS-Rust\\qualification'
# Inspect every existing ancestor before making directories, downloading, or elevating.
[void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
$existing=Test-Path -LiteralPath $workspacePath
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
# Select only the owner-reviewed, merged and independently pinned candidate.
# API provenance, including the original synthetic PR SHA, is checked by the
# shared read-only verifier *after* download and before any binary is executed.
if ($SourceSha -cne $script:16L1Approved.SourceSha -or
    $RunId -ne $script:16L1Approved.RunId) {
    throw 'Unapproved historical candidate; no download or physical execution'
}
if (!$existing) {
    New-Item -ItemType Directory -Path $workspacePath -Force | Out-Null
    [void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
}
$download=Join-Path $workspacePath 'downloads'
if (!$existing) {
    New-Item -ItemType Directory -Path $download | Out-Null
    [void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
    $artifact="cosos-16pkg-unsigned-candidate-$SourceSha"
    gh run download $RunId --repo $repository --name $artifact --dir $download
    if ($LASTEXITCODE -ne 0) { throw 'Exact-run source artifact download failed' }
}
# Reject any linked archive member, altered ancestor or unsafe ACL after extraction.
[void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
$receiptPath=Join-Path $download 'package-receipt.json'
$setup=Join-Path $download 'installer-package\CoSoSInstaller.exe'
$archive=Join-Path $download 'candidate.zip'
if (!(Test-Path -LiteralPath $receiptPath) -or !(Test-Path -LiteralPath $setup)) {
    throw 'Incomplete downloaded installer or receipt'
}
# Verification-only reusable on both hosted CI and physical D: handoff.
# No current pull request merge_commit_sha or mutable run.pull_requests lookup.
& "$PSScriptRoot/16pkg-16l1-artifact-provenance.ps1" -RunId $RunId -SourceSha $SourceSha -ArtifactRoot $download
if (!$?) { throw 'Independent provenance preflight failed' }
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
$releaseManifest=Join-Path $download 'installer-release-manifest.json'
RequireDigest $releaseManifest $receipt.releaseManifestSha256
$manifest=Get-Content -LiteralPath $releaseManifest -Raw | ConvertFrom-Json
if($manifest.source_sha -ne $SourceSha -or $manifest.version -ne '0.1.0' -or
    $manifest.product -ne 'CoSoS' -or $manifest.platform -ne 'windows-x64') {
    throw 'Attested release manifest identity mismatch'
}
# The renderer manifest was generated inside candidate.zip and exported by
# the release workflow; verify the detached bytes against BOTH manifests and
# the source-bound receipt before trusting unsigned installed components.
$rendererPath=Join-Path $download 'installer-renderer-integrity.json'
RequireDigest $rendererPath $receipt.rendererManifestSha256
$renderer=Get-Content -LiteralPath $rendererPath -Raw | ConvertFrom-Json
if($renderer.format -ne 1 -or $renderer.sourceSha -cne $SourceSha){
    throw 'Renderer manifest source identity mismatch'
}
$rendererEntry=@($manifest.files | Where-Object { $_.path -ceq 'renderer/renderer-integrity.json' })
if($rendererEntry.Count -ne 1 -or
    $rendererEntry[0].sha256 -cne $receipt.rendererManifestSha256 -or
    [long]$rendererEntry[0].bytes -ne (Get-Item -LiteralPath $rendererPath).Length){
    throw 'Renderer manifest does not match source-bound release index'
}
foreach($spec in @(
    @{entry='bin/cos-rust.exe';digest=$receipt.rustSha256},
    @{entry='renderer/WindowsRenderer.exe';digest=$receipt.rendererHostSha256},
    @{entry='renderer/WindowsRenderer.dll';digest=$receipt.rendererDllSha256}
)){
    $indexed=@($manifest.files | Where-Object { $_.path -ceq $spec.entry })
    if($indexed.Count -ne 1 -or $indexed[0].sha256 -cne $spec.digest){
        throw "Receipt/release payload digest mismatch: $($spec.entry)"
    }
}
foreach($spec in @(
    @{name='WindowsRenderer.exe';digest=$receipt.rendererHostSha256},
    @{name='WindowsRenderer.dll';digest=$receipt.rendererDllSha256}
)){
    $indexed=@($renderer.files | Where-Object { $_.path -ceq $spec.name })
    if($indexed.Count -ne 1 -or $indexed[0].sha256 -cne $spec.digest){
        throw "Renderer manifest/receipt digest mismatch: $($spec.name)"
    }
}
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
[void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
New-Item -ItemType Directory -Path $evidence | Out-Null
[void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
if (Test-Path -LiteralPath $root) { throw 'Existing installation root refused' }
function Invoke-Setup([string]$bin,[string]$op,[string]$hash) {
    [void](Assert-16L1Workspace -Workspace $workspacePath -Boundary $boundary -RequirePhysicalD -InspectTree)
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
