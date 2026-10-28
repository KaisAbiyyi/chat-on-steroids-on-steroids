#requires -Version 7.0
# Read-only verification of approved historical PR artifact. Never executes installers/elevates.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][long]$RunId,
    [Parameter(Mandatory)][string]$SourceSha,
    [Parameter(Mandatory)][string]$ArtifactRoot
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/16pkg-16l1-guards.ps1"
$approved=$script:16L1Approved
$repo=$approved.Repository
$workflow="$repo/$($approved.WorkflowPath)"
$attempt=[int]$approved.RunAttempt
if ($SourceSha -cne $approved.SourceSha -or $RunId -ne $approved.RunId) {
    throw 'Only independently authorized historical artifact is supported'
}
if (!(Get-Command gh -ErrorAction SilentlyContinue)) { throw 'Missing GitHub CLI' }
function Get-GitHubJson([string]$Path) {
    $output=gh api $Path
    if ($LASTEXITCODE -ne 0 -or !$output) { throw "GitHub API verification unavailable: $Path" }
    return ($output | ConvertFrom-Json)
}
$run=Get-GitHubJson "repos/$repo/actions/runs/$RunId"
$pull=Get-GitHubJson "repos/$repo/pulls/131"
$artifacts=Get-GitHubJson "repos/$repo/actions/runs/$RunId/artifacts?per_page=100"
$merge=Get-GitHubJson "repos/$repo/commits/$($approved.MergeCommit)"
$compare=Get-GitHubJson "repos/$repo/compare/$SourceSha...main"
Assert-16L1RunOrigin -Run $run -Pull $pull -Artifacts $artifacts -MergedCommit $merge -Compare $compare -RunId $RunId -RunAttempt $attempt -SourceSha $SourceSha -Repository $repo
$root=[IO.Path]::GetFullPath($ArtifactRoot)
$receiptPath=Join-Path $root 'package-receipt.json'
$checks=@(
    @{path=$receiptPath;property='';pin=''},
    @{path=(Join-Path $root 'installer-package/CoSoSInstaller.exe');property='setupSha256';pin='ce28a4f8c9c1edbd09a70146eadfab5f46d508d1fba7df92fe2427c064ab4fb4'},
    @{path=(Join-Path $root 'candidate.zip');property='zipSha256';pin='decb97eecedf0096f636e4327d7ff3ccb824095ed48e0e318e7a518f2a95d888'},
    @{path=(Join-Path $root 'installer-unsigned-0.2.0/CoSoSInstaller.exe');property='update02SetupSha256';pin='84d8eabca38dac2f2a2708e9ce3e3a5a0c45a451f058d72db8cffd2fa97f47d4'},
    @{path=(Join-Path $root 'installer-unsigned-0.3.0/CoSoSInstaller.exe');property='update03SetupSha256';pin='0eeef52ce01aadf53137d6997881db07fa2f7fa1b23368fa0c8471775449cd30'}
)
$eventDigest=''
$receipt=$null
foreach ($check in $checks) {
    $file=$check.path
    if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Required attested file missing: $file" }
    $params=@($file,'--repo',$repo,'--signer-workflow',$workflow,'--source-ref','refs/pull/131/merge','--format','json')
    # The original pull_request GITHUB_SHA was observed in signed OIDC evidence,
    # NOT calculated from the now-merged PR metadata or workflow_run.head_sha.
    $params+=@('--source-digest',$approved.OriginalEventSha)
    # CLI checks signature/certificate, SLSA predicate, actual file digest, ref and workflow.
    $verified=@(gh attestation verify @params | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0 -or $verified.Count -lt 1) { throw "GitHub Sigstore attestation failed: $file" }
    $signedDigest=Assert-16L1AttestationRun -Verification $verified -RunId $RunId -RunAttempt $attempt -Repository $repo -EventDigest $eventDigest
    if (!$eventDigest) { $eventDigest=$signedDigest }
    if ($eventDigest -cne $approved.OriginalEventSha) { throw 'Original signed event SHA is not the approved historical SHA' }
    if ($signedDigest -cne $eventDigest) { throw 'Original PR event SHA differs among attestations' }
    if (!$receipt) {
        $receipt=Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
        if ($receipt.sourceSha -cne $SourceSha -or $receipt.signed -ne $false -or $receipt.native16L1Accepted -ne $false) {
            throw 'Attested receipt source/policy mismatch'
        }
    } else {
        $expected=[string]$receipt.($check.property)
        if ($expected -cne $check.pin -or $expected -cnotmatch '^[a-f0-9]{64}$' -or
            (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expected) {
            throw "Attested receipt and independently pinned digest mismatch: $file"
        }
    }
}
foreach($entry in @(
    @{file='installer-release-manifest.json';key='releaseManifestSha256';pin='864c7d27a7075f4f0b54c0f8c31b06835179e2059164bbf6a16b1c7e46ae14ab'},
    @{file='installer-renderer-integrity.json';key='rendererManifestSha256';pin='3ae6bbd64bc8c4fef5c046ed1efc42e7af7c686c8d244d9382eac9c845cc4f16'}
)){
    $file=Join-Path $root $entry.file
    if (!(Test-Path -LiteralPath $file -PathType Leaf) -or
        [string]$receipt.($entry.key) -cne $entry.pin -or
        (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -cne $entry.pin){
        throw "Verified receipt-manifest digest mismatch: $file"
    }
}
Write-Host "VERIFIED ONLY: merged source $SourceSha; run $RunId/$attempt; artifact $($approved.ArtifactId)"
Write-Host "Original GitHub PR synthetic event SHA (signed OIDC certificate): $eventDigest"
Write-Host 'Five Sigstore proofs and independently pinned installer/manifest digests PASS'
Write-Host 'UNSIGNED artifact; NOT physical 16L1 acceptance.'
