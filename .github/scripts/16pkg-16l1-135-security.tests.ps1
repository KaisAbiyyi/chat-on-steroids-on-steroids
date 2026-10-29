#requires -Version 7.0
# Only ephemeral hosted Windows memory/runner-temp fixture; never touch owner D:.
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Hosted Windows 16L1-135 tests only'
}
. "$PSScriptRoot/16pkg-16l1-135-guards.ps1"
function MustReject([scriptblock]$Callback,[string]$Label) {
    $rejected=$false
    try { & $Callback | Out-Null } catch { $rejected=$true }
    if (!$rejected) { throw "Invalid PR135 acceptance: $Label" }
}
$source='fb1d97a040a28abe74ccbcf5f92e2d33064b85ad'
$eventSha='b903132e2c8a32cde5143d40696813fb3233d11d'
$repo='KaisAbiyyi/chat-on-steroids-on-steroids'
$branch='fix/16l1-native-stage-publish-20261029'
$merge='1'*40
$base=[pscustomobject]@{ref='main'}
$head=[pscustomobject]@{ref=$branch;sha=$source;repo=[pscustomobject]@{full_name=$repo}}
$pr=[pscustomobject]@{number=135;merged=$true;merged_at='2026-10-29T00:00:00Z';base=$base;head=$head;merge_commit_sha=$merge}
$run=[pscustomobject]@{
    id=40808144696L;run_attempt=1;status='completed';conclusion='success';
    event='pull_request';path='.github/workflows/16pkg-native-installer.yml';
    workflow_id=398496557;repository=[pscustomobject]@{full_name=$repo;id=1424422098}
    head_repository=[pscustomobject]@{full_name=$repo;id=1424422098}
    head_branch=$branch;head_sha=$source;head_commit=[pscustomobject]@{id=$source}
}
$merged=[pscustomobject]@{sha=$merge;parents=@([pscustomobject]@{sha=('0'*40)},[pscustomobject]@{sha=$source})}
$compare=[pscustomobject]@{base_commit=[pscustomobject]@{sha=$source};merge_base_commit=[pscustomobject]@{sha=$source};behind_by=0;status='ahead'}
$artifact=[pscustomobject]@{id=12868909286L;name="cosos-16pkg-unsigned-candidate-$source";
    digest='sha256:07d68ffbd6eb30d444f153c51a6a0430d7019f9c919c141181e267de19e99cd0';
    expired=$false;size_in_bytes=124911431;
    workflow_run=[pscustomobject]@{id=40808144696L;repository_id=1424422098;head_sha=$source}}
$artifacts=[pscustomobject]@{artifacts=@($artifact)}
$args=@{Run=$run;Pull=$pr;Artifacts=$artifacts;MergedCommit=$merged;Compare=$compare;
    RunId=40808144696L;RunAttempt=1;SourceSha=$source;Repository=$repo}
Assert-16L1RunOrigin @args
function Clone($obj) { return ($obj | ConvertTo-Json -Depth 30 | ConvertFrom-Json) }
$bad=Clone $run;$bad.run_attempt=2;$args.Run=$bad
MustReject { Assert-16L1RunOrigin @args } 'wrong attempt'
$bad=Clone $run;$bad.head_sha=('a'*40);$args.Run=$bad
MustReject { Assert-16L1RunOrigin @args } 'wrong source'
$bad=Clone $run;$bad.event='workflow_dispatch';$args.Run=$bad
MustReject { Assert-16L1RunOrigin @args } 'wrong event'
$bad=Clone $run;$bad.workflow_id=398496558;$args.Run=$bad
MustReject { Assert-16L1RunOrigin @args } 'wrong workflow'
$bad=Clone $run;$bad.repository.id=999;$args.Run=$bad
MustReject { Assert-16L1RunOrigin @args } 'wrong repository'
$args.Run=$run
$bad=Clone $pr;$bad.merged=$false;$args.Pull=$bad
MustReject { Assert-16L1RunOrigin @args } 'unmerged PR135'
$bad=Clone $pr;$bad.head.sha=('a'*40);$args.Pull=$bad
MustReject { Assert-16L1RunOrigin @args } 'changed PR135 head'
$args.Pull=$pr
$bad=Clone $merged;$bad.parents=@([pscustomobject]@{sha=('a'*40)});$args.MergedCommit=$bad
MustReject { Assert-16L1RunOrigin @args } 'source missing from merge parents'
$args.MergedCommit=$merged
$bad=Clone $compare;$bad.merge_base_commit.sha=('a'*40);$args.Compare=$bad
MustReject { Assert-16L1RunOrigin @args } 'not in main ancestry'
$args.Compare=$compare
$bad=Clone $artifact;$bad.id=12868909287L;$args.Artifacts=[pscustomobject]@{artifacts=@($bad)}
MustReject { Assert-16L1RunOrigin @args } 'wrong artifact ID'
$bad=Clone $artifact;$bad.digest='sha256:'+('a'*64);$args.Artifacts=[pscustomobject]@{artifacts=@($bad)}
MustReject { Assert-16L1RunOrigin @args } 'changed artifact ZIP digest'
$bad=Clone $artifact;$bad.expired=$true;$args.Artifacts=[pscustomobject]@{artifacts=@($bad)}
MustReject { Assert-16L1RunOrigin @args } 'expired source artifact'
$args.Artifacts=$artifacts
$invocation="https://github.com/$repo/actions/runs/40808144696/attempts/1"
$cert=[pscustomobject]@{
    runInvocationURI=$invocation;sourceRepositoryIdentifier='1424422098'
    sourceRepositoryRef='refs/pull/135/merge';sourceRepositoryDigest=$eventSha;githubWorkflowTrigger='pull_request'
}
$statement=[pscustomobject]@{predicateType='https://slsa.dev/provenance/v1';predicate=[pscustomobject]@{
    runDetails=[pscustomobject]@{metadata=[pscustomobject]@{invocationId=$invocation}}}}
$verification=[pscustomobject]@{verificationResult=[pscustomobject]@{
    signature=[pscustomobject]@{certificate=$cert};statement=$statement}}
$observed=Assert-16L1AttestationRun @($verification) 40808144696 1 $repo
if ($observed -cne $eventSha) { throw 'Signed original event SHA mismatch' }
MustReject { Assert-16L1AttestationRun @() 40808144696 1 $repo } 'missing Sigstore proof'
$bad=Clone $verification;$bad.verificationResult.signature.certificate.sourceRepositoryDigest=('0'*40)
MustReject { Assert-16L1AttestationRun @($bad) 40808144696 1 $repo } 'wrong OIDC PR merge SHA'
$bad=Clone $verification;$bad.verificationResult.signature.certificate.sourceRepositoryRef='refs/heads/main'
MustReject { Assert-16L1AttestationRun @($bad) 40808144696 1 $repo } 'wrong signed ref'
$bad=Clone $verification;$bad.verificationResult.signature.certificate.sourceRepositoryIdentifier='999'
MustReject { Assert-16L1AttestationRun @($bad) 40808144696 1 $repo } 'wrong certificate repo'
$bad=Clone $verification;$bad.verificationResult.statement.predicateType='https://example.invalid'
MustReject { Assert-16L1AttestationRun @($bad) 40808144696 1 $repo } 'wrong SLSA predicate'
MustReject { Assert-16L1AttestationRun @($verification) 40808144696 2 $repo } 'wrong attested attempt'
Assert-16L1135WorkspaceNonce -Name "16L1-$source-1234567890abcdef"
MustReject { Assert-16L1135WorkspaceNonce -Name "16L1-$source" } 'missing nonce'
MustReject { Assert-16L1135WorkspaceNonce -Name '16L1-9939ea0a9fdc592e6821296e07eb0b7db3653adb-1234567890abcdef' } 'old source workspace'
MustReject { Assert-16L1135WorkspaceNonce -Name "16L1-$source-12345678" } 'short nonce'
MustReject { Assert-16L1135WorkspaceNonce -Name "16L1-$source-1234567890abcdeg" } 'nonhex nonce'
$self=[Security.Principal.WindowsIdentity]::GetCurrent().User
$acl=[Security.AccessControl.DirectorySecurity]::new()
$acl.SetAccessRuleProtection($true,$false)
$acl.SetOwner($self)
foreach($id in @($self.Value,'S-1-5-18','S-1-5-32-544')) {
    $sid=[Security.Principal.SecurityIdentifier]::new($id)
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,
        [Security.AccessControl.FileSystemRights]::FullControl,[Security.AccessControl.AccessControlType]::Allow))
}
Assert-16L1Acl -Acl $acl -OwnerSid $self.Value -Path 'fixture' -ProtectedBoundary
$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
    [Security.Principal.SecurityIdentifier]::new('S-1-5-32-545'),
    [Security.AccessControl.FileSystemRights]::Modify,[Security.AccessControl.AccessControlType]::Allow))
MustReject { Assert-16L1Acl -Acl $acl -OwnerSid $self.Value -Path 'fixture' -ProtectedBoundary } 'broad Users modify'
$root=Join-Path $env:RUNNER_TEMP ('16l1-135-hash-'+[guid]::NewGuid().ToString('N'))
try {
    [IO.File]::WriteAllText($root,'approved')
    $hash=Get-16L1GitBlobSha $root
    [IO.File]::WriteAllText($root,'changed')
    if ((Get-16L1GitBlobSha $root) -ceq $hash) { throw 'Altered Git blob bytes accepted' }
} finally { Remove-Item -LiteralPath $root -ErrorAction SilentlyContinue }
Write-Host '16L1 PR135 negative security fixtures PASS: run/ref/OIDC/manifest/ACL/nonces/immutable bytes'
