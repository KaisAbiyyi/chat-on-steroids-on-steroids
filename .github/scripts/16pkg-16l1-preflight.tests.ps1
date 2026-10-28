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
    $sha = 'a'*40
    $repo='KaisAbiyyi/chat-on-steroids-on-steroids'
    $head = [pscustomobject]@{ref='work/16pkg-01-native-installer-20261027';sha=$sha;repo=[pscustomobject]@{full_name=$repo}}
    $base = [pscustomobject]@{ref='main'}
    $run=[pscustomobject]@{id=42;status='completed';conclusion='success';event='pull_request';
        path='.github/workflows/16pkg-native-installer.yml';repository=[pscustomobject]@{full_name=$repo};
        head_branch=$head.ref;head_sha=$sha;run_attempt=1;pull_requests=@([pscustomobject]@{number=131;head=$head;base=$base})}
    $pr=[pscustomobject]@{number=131;state='open';head=$head;base=$base;merge_commit_sha=('b'*40)}
    $art=[pscustomobject]@{artifacts=@([pscustomobject]@{name="cosos-16pkg-unsigned-candidate-$sha";
        workflow_run=[pscustomobject]@{id=42};expired=$false;size_in_bytes=32})}
    Assert-16L1RunOrigin $run $pr $art 42 $sha $repo
    $badRun=$run.PSObject.Copy()
    $badRun.event='workflow_dispatch'
    Refused { Assert-16L1RunOrigin $badRun $pr $art 42 $sha $repo } 'wrong event'
    $badRun=$run.PSObject.Copy()
    $badRun.head_sha='c'*40
    Refused { Assert-16L1RunOrigin $badRun $pr $art 42 $sha $repo } 'wrong run source'
    $badPr=$pr.PSObject.Copy()
    $badPr.head=[pscustomobject]@{ref=$head.ref;sha=('d'*40);repo=$head.repo}
    Refused { Assert-16L1RunOrigin $run $badPr $art 42 $sha $repo } 'changed reviewed PR'
    $badArt=[pscustomobject]@{artifacts=@([pscustomobject]@{name="cosos-16pkg-unsigned-candidate-$sha";
        workflow_run=[pscustomobject]@{id=13};expired=$false;size_in_bytes=32})}
    Refused { Assert-16L1RunOrigin $run $pr $badArt 42 $sha $repo } 'artifact from another run'
    $valid=[pscustomobject]@{verificationResult=[pscustomobject]@{statement=[pscustomobject]@{
        predicate=[pscustomobject]@{runDetails=[pscustomobject]@{metadata=[pscustomobject]@{
            invocationId="https://github.com/$repo/actions/runs/42/attempts/1"
        }}}}}}
    Assert-16L1AttestationRun @($valid) 42 1 $repo
    Refused { Assert-16L1AttestationRun @($valid) 43 1 $repo } 'attestation signed in another run'
    Refused { Assert-16L1AttestationRun @($valid) 42 2 $repo } 'attestation signed in another attempt'
    Write-Host '16L1 security negative regressions PASS: linked ancestor/child, run/event/head/artifact and attestation mismatch'
} finally {
    if (Test-Path -LiteralPath $qual) {
        # Remove reparse children as links before removing only test-owned directories.
        $links=@(Get-ChildItem -LiteralPath $qual -Force -Recurse -Attributes ReparsePoint -ErrorAction SilentlyContinue)
        foreach ($link in $links) { Remove-Item -LiteralPath $link.FullName -Force }
    }
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
