# Hosted Windows-only negative proof. All created paths remain under RUNNER_TEMP.
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw '16L1 guard regressions require hosted Windows CI'
}
. "$PSScriptRoot/16pkg-16l1-guards.ps1"
$root = Join-Path $env:RUNNER_TEMP ('16l1-guard-test-' + [guid]::NewGuid().ToString('N'))
$foreign = Join-Path $root 'foreign'
$qual = Join-Path $root 'qualification'
function Refused([scriptblock]$Action,[string]$Reason) {
    $blocked = $false
    try { & $Action | Out-Null } catch { $blocked = $true }
    if (!$blocked) { throw "Negative security regression accepted: $Reason" }
}
try {
    New-Item -ItemType Directory -Path $foreign -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $foreign 'sentinel'),'UNTOUCHED')
    New-Item -ItemType Junction -Path $qual -Target $foreign | Out-Null
    Refused { Assert-16L1Workspace -Workspace (Join-Path $qual ('16L1-' + ('a'*40))) -Boundary $qual } 'linked qualification ancestor'
    Remove-Item -LiteralPath $qual -Force
    New-Item -ItemType Directory -Path $qual | Out-Null
    $parent = Join-Path $qual ('16L1-' + ('a'*40))
    New-Item -ItemType Directory -Path $parent | Out-Null
    $linked = Join-Path $parent 'nested'
    New-Item -ItemType Junction -Path $linked -Target $foreign | Out-Null
    Refused { Assert-16L1Workspace -Workspace (Join-Path $linked 'newworkspace') -Boundary $qual } 'linked nested workspace'
    Refused { Assert-16L1Workspace -Workspace $parent -Boundary $qual -InspectTree } 'linked child inside workspace'
    if ([IO.File]::ReadAllText((Join-Path $foreign 'sentinel')) -cne 'UNTOUCHED') {
        throw 'Foreign test data was changed'
    }
    Remove-Item -LiteralPath $linked -Force
    # Real GitHub REST shapes observed in run 40710902296 *after* merge:
    # workflow_run.pull_requests is EMPTY; run.head_sha is checked-out PR HEAD,
    # while the OIDC certificate sourceRepositoryDigest is a DIFFERENT PR merge SHA.
    $sha='9939ea0a9fdc592e6821296e07eb0b7db3653adb'
    $repo='KaisAbiyyi/chat-on-steroids-on-steroids'
    $branch='work/16pkg-01-native-installer-20261027'
    $head=[pscustomobject]@{ref=$branch;sha=$sha;repo=[pscustomobject]@{full_name=$repo}}
    $run=[pscustomobject]@{
        id=40710902296L;run_attempt=1;status='completed';conclusion='success';event='pull_request'
        path='.github/workflows/16pkg-native-installer.yml';workflow_id=398496557
        repository=[pscustomobject]@{full_name=$repo;id=1424422098}
        head_repository=[pscustomobject]@{full_name=$repo;id=1424422098}
        head_branch=$branch;head_sha=$sha;head_commit=[pscustomobject]@{id=$sha}
        pull_requests=@()
    }
    $pr=[pscustomobject]@{
        number=131;state='closed';merged=$true;merged_at='2026-10-28T17:00:00Z'
        head=$head;base=[pscustomobject]@{ref='main'}
        merge_commit_sha=('f'*40) # deliberately changed current PR metadata
    }
    $merge=[pscustomobject]@{
        sha='be2727fe437f7878f023ddd3422b1c6ae4f70907'
        parents=@([pscustomobject]@{sha=('a'*40)},[pscustomobject]@{sha=$sha})
    }
    $compare=[pscustomobject]@{status='ahead';behind_by=0;
        base_commit=[pscustomobject]@{sha=$sha};merge_base_commit=[pscustomobject]@{sha=$sha}}
    $art=[pscustomobject]@{artifacts=@([pscustomobject]@{
        id=12827415031L;name="cosos-16pkg-unsigned-candidate-$sha";
        digest='sha256:38fc612236c093cde03d8134173b9c8f8d76c58203ec8614c80122e23e6b60b7'
        workflow_run=[pscustomobject]@{id=40710902296L;repository_id=1424422098;head_sha=$sha}
        expired=$false;size_in_bytes=124913556
    })}
    $origin=@{Run=$run;Pull=$pr;MergedCommit=$merge;Compare=$compare;Artifacts=$art;
        RunId=40710902296L;RunAttempt=1;SourceSha=$sha;Repository=$repo}
    Assert-16L1RunOrigin @origin
    $changedPr=$pr.PSObject.Copy()
    $changedPr.merge_commit_sha='0'*40
    $origin.Pull=$changedPr
    Assert-16L1RunOrigin @origin  # neither old nor new PR merge SHA is trusted
    $origin.Pull=$pr
    $badRun=$run.PSObject.Copy();$badRun.event='workflow_dispatch'
    $origin.Run=$badRun
    Refused { Assert-16L1RunOrigin @origin } 'wrong run event'
    $badRun=$run.PSObject.Copy();$badRun.head_sha='c'*40
    $origin.Run=$badRun
    Refused { Assert-16L1RunOrigin @origin } 'wrong checkout source SHA'
    $badRun=$run.PSObject.Copy();$badRun.run_attempt=2
    $origin.Run=$badRun
    Refused { Assert-16L1RunOrigin @origin } 'wrong run attempt'
    $badRun=$run.PSObject.Copy();$badRun.id=40710902297L
    $origin.Run=$badRun
    Refused { Assert-16L1RunOrigin @origin } 'wrong run identity'
    $badRun=$run.PSObject.Copy();$badRun.workflow_id=398496558
    $origin.Run=$badRun
    Refused { Assert-16L1RunOrigin @origin } 'wrong workflow identity'
    $badRun=$run.PSObject.Copy();$badRun.repository=[pscustomobject]@{id=999;full_name=$repo}
    $origin.Run=$badRun
    Refused { Assert-16L1RunOrigin @origin } 'wrong repository ID'
    $origin.Run=$run
    $badPr=$pr.PSObject.Copy();$badPr.merged=$false
    $origin.Pull=$badPr
    Refused { Assert-16L1RunOrigin @origin } 'source never merged'
    $origin.Pull=$pr
    $badMerge=$merge.PSObject.Copy();$badMerge.parents=@([pscustomobject]@{sha=('a'*40)})
    $origin.MergedCommit=$badMerge
    Refused { Assert-16L1RunOrigin @origin } 'source absent from approved merge'
    $origin.MergedCommit=$merge
    $badCompare=$compare.PSObject.Copy()
    $badCompare.merge_base_commit=[pscustomobject]@{sha=('b'*40)}
    $origin.Compare=$badCompare
    Refused { Assert-16L1RunOrigin @origin } 'source not in main DAG'
    $origin.Compare=$compare
    $badArt=[pscustomobject]@{artifacts=@([pscustomobject]@{
        id=12827415032L;name="cosos-16pkg-unsigned-candidate-$sha"
        digest='sha256:38fc612236c093cde03d8134173b9c8f8d76c58203ec8614c80122e23e6b60b7'
        workflow_run=[pscustomobject]@{id=40710902296L;repository_id=1424422098;head_sha=$sha}
        expired=$false;size_in_bytes=124913556
    })}
    $origin.Artifacts=$badArt
    Refused { Assert-16L1RunOrigin @origin } 'wrong artifact ID'
    $badArt.artifacts[0].id=12827415031L
    $badArt.artifacts[0].digest='sha256:'+('a'*64)
    Refused { Assert-16L1RunOrigin @origin } 'wrong immutable artifact SHA256'
    $origin.Artifacts=$art
    $expected="https://github.com/$repo/actions/runs/40710902296/attempts/1"
    $eventDigest='247e57638a8f5afce93d87fc6d671ce54eaf3ccd'
    $cert=[pscustomobject]@{
        runInvocationURI=$expected;sourceRepositoryIdentifier='1424422098'
        sourceRepositoryRef='refs/pull/131/merge';sourceRepositoryDigest=$eventDigest
        githubWorkflowTrigger='pull_request'
    }
    $statement=[pscustomobject]@{
        predicateType='https://slsa.dev/provenance/v1'
        predicate=[pscustomobject]@{runDetails=[pscustomobject]@{
            metadata=[pscustomobject]@{invocationId=$expected}}}
    }
    $verification=[pscustomobject]@{verificationResult=[pscustomobject]@{
        signature=[pscustomobject]@{certificate=$cert};statement=$statement}}
    $actual=Assert-16L1AttestationRun @($verification) 40710902296 1 $repo
    if ($actual -cne $eventDigest) { throw 'Original event digest extraction failed' }
    Refused { Assert-16L1AttestationRun @() 40710902296 1 $repo } 'missing attestation'
    Refused { Assert-16L1AttestationRun @($verification) 40710902297 1 $repo } 'wrong attested run'
    Refused { Assert-16L1AttestationRun @($verification) 40710902296 2 $repo } 'wrong attested attempt'
    Refused { Assert-16L1AttestationRun @($verification) 40710902296 1 $repo ('d'*40) } 'wrong original PR event SHA'
    $badCert=$cert.PSObject.Copy();$badCert.sourceRepositoryRef='refs/heads/main'
    $badVerification=[pscustomobject]@{verificationResult=[pscustomobject]@{
        signature=[pscustomobject]@{certificate=$badCert};statement=$statement}}
    Refused { Assert-16L1AttestationRun @($badVerification) 40710902296 1 $repo } 'wrong signed source ref'
    $badCert=$cert.PSObject.Copy();$badCert.sourceRepositoryIdentifier='111'
    $badVerification.verificationResult.signature.certificate=$badCert
    Refused { Assert-16L1AttestationRun @($badVerification) 40710902296 1 $repo } 'wrong signed repo ID'
    $badStatement=$statement.PSObject.Copy();$badStatement.predicateType='https://example.invalid'
    $badVerification.verificationResult.signature.certificate=$cert
    $badVerification.verificationResult.statement=$badStatement
    Refused { Assert-16L1AttestationRun @($badVerification) 40710902296 1 $repo } 'invalid SLSA type'
    Write-Host '16L1 security regressions PASS: merged PR/history/source, run/attempt/artifact/digest/OIDC and NTFS junction rejection'
} finally {
    if (Test-Path -LiteralPath $qual) {
        # Remove reparse children as links before removing only test-owned directories.
        $links=@(Get-ChildItem -LiteralPath $qual -Force -Recurse -Attributes ReparsePoint -ErrorAction SilentlyContinue)
        foreach ($link in $links) { Remove-Item -LiteralPath $link.FullName -Force }
    }
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
