#requires -Version 7.0
# Hosted Windows-only fixtures. Never run owner D: bootstrap or installers in CI.
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Require hosted Windows runner'
}
. "$PSScriptRoot/16pkg-16l1-owner-bootstrap.ps1"
$root=Join-Path $env:RUNNER_TEMP ('bootstrap-fixture-'+[guid]::NewGuid().ToString('N'))
$owner=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$commit=('a'*40)
function Refused([scriptblock]$Work,[string]$Reason) {
    $blocked=$false
    try { & $Work | Out-Null } catch { $blocked=$true }
    if (!$blocked) { throw "Fail-closed regression accepted: $Reason" }
}
function New-Root([string]$Path) {
    New-Item -ItemType Directory -Path $Path -ErrorAction Stop | Out-Null
    & icacls $Path '/setowner' ('*'+$owner) | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Fixture owner assignment failed' }
    & icacls $Path '/inheritance:r' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Fixture protect failed' }
    $grant='*'+$owner+':(OI)(CI)F'
    & icacls $Path '/grant:r' $grant '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Fixture grant failed' }
    & icacls $Path '/remove' '*S-1-3-0' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Fixture creator-owner cleanup failed' }
    if ($LASTEXITCODE -ne 0) { throw 'Fixture grant failed' }
    Assert-16L1BootstrapNode -Path $Path -OwnerSid $owner -Directory -Protected
    New-Item -ItemType Directory -Path (Join-Path $Path 'scripts') | Out-Null
    Assert-16L1ChildNames -Path $Path -Allowed @('scripts') -OwnerSid $owner
    return (Join-Path $Path 'scripts')
}
$blobs=@{}
foreach($name in $script:BootstrapNames) {
    $bytes=[Text.Encoding]::UTF8.GetBytes("approved-$name")
    $blobs[$name]=@{Bytes=$bytes;Sha=(Get-16L1BlobHash $bytes)}
}
try {
    New-Item -ItemType Directory -Path $root -ErrorAction Stop | Out-Null
    Refused { Get-16L1ApprovedBlobs -Commit '<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>' } 'literal placeholder'
    Refused { Get-16L1ApprovedBlobs -Commit 'deadbeef' } 'short source'
    # Existing protected boundary + empty scripts, then idempotent repetition.
    $empty=New-Root (Join-Path $root 'empty')
    Invoke-16L1Staging -Dir $empty -Blobs $blobs -Commit $commit -OwnerSid $owner
    Invoke-16L1Staging -Dir $empty -Blobs $blobs -Commit $commit -OwnerSid $owner
    Assert-16L1StagingState -Dir $empty -Blobs $blobs -Commit $commit -OwnerSid $owner -Complete
    # Valid partial scripts and valid interrupted staging.
    $partial=New-Root (Join-Path $root 'partial')
    $first=$script:BootstrapNames[0]
    [IO.File]::WriteAllBytes((Join-Path $partial $first),$blobs[$first].Bytes)
    Invoke-16L1Staging -Dir $partial -Blobs $blobs -Commit $commit -OwnerSid $owner
    Assert-16L1StagingState -Dir $partial -Blobs $blobs -Commit $commit -OwnerSid $owner -Complete
    $staged=New-Root (Join-Path $root 'staged')
    [IO.File]::WriteAllBytes((Join-Path $staged (".$first.stage-$commit")),$blobs[$first].Bytes)
    Invoke-16L1Staging -Dir $staged -Blobs $blobs -Commit $commit -OwnerSid $owner
    Assert-16L1StagingState -Dir $staged -Blobs $blobs -Commit $commit -OwnerSid $owner -Complete
    # Existing foreign/half-written files must be preserved and rejected.
    $bad=New-Root (Join-Path $root 'foreign-bytes')
    [IO.File]::WriteAllText((Join-Path $bad $first),'FOREIGN')
    Refused { Invoke-16L1Staging -Dir $bad -Blobs $blobs -Commit $commit -OwnerSid $owner } 'tampered partial script'
    if ([IO.File]::ReadAllText((Join-Path $bad $first)) -cne 'FOREIGN') { throw 'Foreign bytes mutated' }
    $truncated=New-Root (Join-Path $root 'truncated')
    [IO.File]::WriteAllText((Join-Path $truncated (".$first.stage-$commit")),'INCOMPLETE')
    Refused { Invoke-16L1Staging -Dir $truncated -Blobs $blobs -Commit $commit -OwnerSid $owner } 'incomplete stage'
    $extra=New-Root (Join-Path $root 'unrelated')
    [IO.File]::WriteAllText((Join-Path $extra 'foreign.txt'),'PRESERVE')
    Refused { Invoke-16L1Staging -Dir $extra -Blobs $blobs -Commit $commit -OwnerSid $owner } 'unexpected script'
    if ([IO.File]::ReadAllText((Join-Path $extra 'foreign.txt')) -cne 'PRESERVE') { throw 'Foreign content overwritten' }
    $acl=Get-Acl -LiteralPath (Join-Path $root 'empty')
    Refused { Assert-16L1BootstrapAcl -Acl $acl -OwnerSid 'S-1-5-32-545' -ExpectedSid $owner -Path $empty -RequireProtected } 'wrong owner'
    $unsafe=[Security.AccessControl.DirectorySecurity]::new()
    Refused { Assert-16L1BootstrapAcl -Acl $unsafe -OwnerSid $owner -ExpectedSid $owner -Path $empty -RequireProtected } 'unprotected ACL'
    $hostile=[Security.AccessControl.DirectorySecurity]::new()
    $hostile.SetAccessRuleProtection($true,$false)
    foreach($id in @($owner,'S-1-5-18','S-1-5-32-544','S-1-5-32-545')) {
        $hostile.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
            [Security.Principal.SecurityIdentifier]::new($id),[Security.AccessControl.FileSystemRights]::FullControl,
            [Security.AccessControl.AccessControlType]::Allow))
    }
    Refused { Assert-16L1BootstrapAcl -Acl $hostile -OwnerSid $owner -ExpectedSid $owner -Path $empty -RequireProtected } 'broad Modify ACL'
    $linkRoot=New-Root (Join-Path $root 'link-test')
    $linked=Join-Path $linkRoot 'linked'
    New-Item -ItemType Junction -Path $linked -Target $empty | Out-Null
    Refused { Assert-16L1ChildNames -Path $linkRoot -Allowed @('linked') -OwnerSid $owner } 'linked script path'
    Remove-Item -LiteralPath $linked -Force
    $global:PSNativeCommandUseErrorActionPreference=$false
    $output=& pwsh -NoProfile -File "$PSScriptRoot/16pkg-16l1-owner-bootstrap.ps1" -TrustedCommit '<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>' 2>&1
    if ($LASTEXITCODE -eq 0 -or (($output | Out-String) -match 'BOOTSTRAP PASS')) {
        throw 'Failed bootstrap printed success or exited zero'
    }
    $global:LASTEXITCODE=0
    Write-Host '16L1 resumable owner bootstrap regressions PASS: valid existing/partial, bad files/ACL/owner/link, fail exit'
} finally {
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
