#requires -Version 7.0
# Read-only verification of approved historical PR artifact. Never executes installers/elevates.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][long]$RunId,
    [Parameter(Mandatory)][string]$SourceSha,
    [Parameter(Mandatory)][string]$ArtifactRoot
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/16pkg-16l1-135-guards.ps1"
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
$pull=Get-GitHubJson "repos/$repo/pulls/135"
$artifacts=Get-GitHubJson "repos/$repo/actions/runs/$RunId/artifacts?per_page=100"
if (!$pull.merged -or !$pull.merged_at) { throw 'STOP: PR135 is not merged; preserve historical artifact' }
$merge=Get-GitHubJson "repos/$repo/commits/$($pull.merge_commit_sha)"
$compare=Get-GitHubJson "repos/$repo/compare/$SourceSha...main"
Assert-16L1RunOrigin -Run $run -Pull $pull -Artifacts $artifacts -MergedCommit $merge -Compare $compare -RunId $RunId -RunAttempt $attempt -SourceSha $SourceSha -Repository $repo
$root=[IO.Path]::GetFullPath($ArtifactRoot)
$receiptPath=Join-Path $root 'package-receipt.json'
$checks=@(
    @{path=$receiptPath;property='';pin='9c473677c03cead075cfbf0bb6015962fdfc8f2e785e5c2334bb95282a56b308'},
    @{path=(Join-Path $root 'installer-package/CoSoSInstaller.exe');property='setupSha256';pin='66adbc6160f99730b029d6f8866aba0291ec98f55a48fa08458954faa29cc76b'},
    @{path=(Join-Path $root 'candidate.zip');property='zipSha256';pin='2a5187faa62a8c296124c4bb81b1d5078485e4cbec4cf845ed8832279bebd17f'},
    @{path=(Join-Path $root 'installer-unsigned-0.2.0/CoSoSInstaller.exe');property='update02SetupSha256';pin='93b68e7322a86a7acb556aaa9b705de16725cc09ea29276be0422644970989a4'},
    @{path=(Join-Path $root 'installer-unsigned-0.3.0/CoSoSInstaller.exe');property='update03SetupSha256';pin='27a98bf1c0ec90ed420b75012a681671e8886cb87f9b4cf36ed56f04d8cd5ede'}
)
$eventDigest=''
$receipt=$null
foreach ($check in $checks) {
    $file=$check.path
    if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Required attested file missing: $file" }
    $params=@($file,'--repo',$repo,'--signer-workflow',$workflow,'--source-ref','refs/pull/135/merge','--format','json')
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
        if ((Get-FileHash -LiteralPath $receiptPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $check.pin) { throw 'Pinned signed PR135 receipt hash mismatch' }
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
    @{file='installer-release-manifest.json';key='releaseManifestSha256';pin='24b9ddff3cca496aa3708706e4ad9326eb2520d4ce130ab6f4d77a8754aa53ae'},
    @{file='installer-renderer-integrity.json';key='rendererManifestSha256';pin='c3bf83730594f7ece72bf224d7552c433e711ca510ae320e9b14d2ca6b4c4e95'}
)){
    $file=Join-Path $root $entry.file
    if (!(Test-Path -LiteralPath $file -PathType Leaf) -or
        [string]$receipt.($entry.key) -cne $entry.pin -or
        (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -cne $entry.pin){
        throw "Verified receipt-manifest digest mismatch: $file"
    }
}
Write-Host "VERIFIED ONLY: reviewed/merged PR135 source $SourceSha; run $RunId/$attempt; artifact $($approved.ArtifactId)"
Write-Host "Original GitHub PR synthetic event SHA (signed OIDC certificate): $eventDigest"
Write-Host 'Five Sigstore proofs and independently pinned installer/manifest digests PASS'
Write-Host 'UNSIGNED artifact; NOT physical 16L1 acceptance.'
