#requires -Version 7.0
# Run only AFTER PR #134 independent security approval and merge.
# Dot sourcing declares pure helpers for hosted Windows fixture tests; no owner operations.
[CmdletBinding()]
param([string]$TrustedCommit)
$ErrorActionPreference='Stop'
$script:BootstrapRepo='KaisAbiyyi/chat-on-steroids-on-steroids'
$script:BootstrapBoundary='D:\CoSoS-16L1-Qualification'
$script:BootstrapNames=@('16pkg-16l1-guards.ps1',
 '16pkg-16l1-artifact-provenance.ps1','16pkg-16l1-local-qualification.ps1')
function Get-16L1BlobHash([byte[]]$Bytes) {
    $prefix=[Text.Encoding]::ASCII.GetBytes("blob $($Bytes.Length)$([char]0)")
    $all=[byte[]]::new($prefix.Length+$Bytes.Length)
    [Array]::Copy($prefix,0,$all,0,$prefix.Length)
    [Array]::Copy($Bytes,0,$all,$prefix.Length,$Bytes.Length)
    return [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($all)).ToLowerInvariant()
}
function Assert-16L1BootstrapAcl {
    [CmdletBinding()]
    param($Acl,[string]$OwnerSid,[string]$ExpectedSid,[string]$Path,[switch]$RequireProtected)
    $allowed=@($ExpectedSid,'S-1-5-18','S-1-5-32-544')
    if (($RequireProtected -and $OwnerSid -cne $ExpectedSid) -or
        (!$RequireProtected -and $OwnerSid -notin $allowed)) {
        throw "Untrusted protected resource owner: $Path"
    }
    if ($RequireProtected -and !$Acl.AreAccessRulesProtected) { throw "Inherited boundary ACL: $Path" }
    $seen=@{}
    foreach($ace in $Acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        $id=$ace.IdentityReference.Value
        if ($id -notin $allowed -or $ace.AccessControlType -ne 'Allow' -or
            ($RequireProtected -and $ace.IsInherited)) {
            throw "Unexpected ACL rule: $Path ($id)"
        }
        $full=[int64][Security.AccessControl.FileSystemRights]::FullControl
        if (([int64]$ace.FileSystemRights -band $full) -ne $full) {
            throw "Trusted ACL entry lacks FullControl: $Path ($id)"
        }
        $seen[$id]=$true
    }
    foreach($id in $allowed) {
        if (!$seen.ContainsKey($id)) { throw "Missing trusted ACL entry: $Path ($id)" }
    }
}
function Assert-16L1BootstrapNode {
    [CmdletBinding()]
    param([string]$Path,[string]$OwnerSid,[switch]$Directory,[switch]$Protected)
    $item=Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Linked/reparse path: $Path"
    }
    if ($Directory -ne [bool]$item.PSIsContainer) {
        throw "Unexpected file/directory type: $Path"
    }
    $acl=Get-Acl -LiteralPath $Path -ErrorAction Stop
    Assert-16L1BootstrapAcl -Acl $acl -OwnerSid $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -ExpectedSid $OwnerSid -Path $Path -RequireProtected:$Protected
}
function Assert-16L1ChildNames {
    [CmdletBinding()]
    param([string]$Path,[string[]]$Allowed,[string]$OwnerSid)
    foreach($child in @(Get-ChildItem -LiteralPath $Path -Force -ErrorAction Stop)) {
        if ($child.Name -cnotin $Allowed) {
            throw "Unexpected existing content (never auto-delete): $($child.FullName)"
        }
        Assert-16L1BootstrapNode -Path $child.FullName -OwnerSid $OwnerSid -Directory:$child.PSIsContainer
    }
}
function Assert-16L1StagingState {
    [CmdletBinding()]
    param([string]$Dir,[hashtable]$Blobs,[string]$Commit,[string]$OwnerSid,[switch]$Complete)
    $stages=@($script:BootstrapNames | ForEach-Object { ".$_.stage-$Commit" })
    Assert-16L1BootstrapNode -Path $Dir -OwnerSid $OwnerSid -Directory
    Assert-16L1ChildNames -Path $Dir -Allowed @($script:BootstrapNames+$stages) -OwnerSid $OwnerSid
    foreach($name in $script:BootstrapNames) {
        $final=Join-Path $Dir $name
        $stage=Join-Path $Dir ".$name.stage-$Commit"
        if ($Complete -and (!(Test-Path -LiteralPath $final -PathType Leaf) -or
            (Test-Path -LiteralPath $stage))) {
            throw "Bootstrap not fully staged: $name"
        }
        foreach($candidate in @($final,$stage)) {
            if (!(Test-Path -LiteralPath $candidate)) { continue }
            Assert-16L1BootstrapNode -Path $candidate -OwnerSid $OwnerSid
            if ((Get-16L1BlobHash ([IO.File]::ReadAllBytes($candidate))) -cne $Blobs[$name].Sha) {
                throw "Existing partial/foreign script differs from reviewed source: $candidate"
            }
        }
    }
}
function Invoke-16L1Staging {
    [CmdletBinding()]
    param([string]$Dir,[hashtable]$Blobs,[string]$Commit,[string]$OwnerSid)
    Assert-16L1StagingState -Dir $Dir -Blobs $Blobs -Commit $Commit -OwnerSid $OwnerSid
    foreach($name in $script:BootstrapNames) {
        $final=Join-Path $Dir $name
        $stage=Join-Path $Dir ".$name.stage-$Commit"
        if (Test-Path -LiteralPath $final) {
            if (Test-Path -LiteralPath $stage) { throw "Duplicate partial staging: $name" }
            continue
        }
        if (!(Test-Path -LiteralPath $stage)) {
            $file=[IO.File]::Open($stage,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
            try { $file.Write($Blobs[$name].Bytes,0,$Blobs[$name].Bytes.Length) }
            finally { $file.Dispose() }
        }
        Assert-16L1StagingState -Dir $Dir -Blobs $Blobs -Commit $Commit -OwnerSid $OwnerSid
        [IO.File]::Move($stage,$final,$false)  # atomic same-directory; never replace
        Assert-16L1StagingState -Dir $Dir -Blobs $Blobs -Commit $Commit -OwnerSid $OwnerSid
    }
    Assert-16L1StagingState -Dir $Dir -Blobs $Blobs -Commit $Commit -OwnerSid $OwnerSid -Complete
}
function Get-16L1ApprovedBlobs {
    [CmdletBinding()]
    param([string]$Commit)
    if ($Commit -cnotmatch '^[a-f0-9]{40}$') {
        throw 'STOP: full independently reviewed MERGED tooling SHA required; no placeholder or PR HEAD'
    }
    if (!(Get-Command gh -ErrorAction SilentlyContinue)) { throw 'Missing GitHub CLI' }
    $repo=$script:BootstrapRepo
    function Api([string]$Path) {
        $output=& gh api $Path
        if ($LASTEXITCODE -ne 0 -or !$output) { throw "GitHub API verification failed: $Path" }
        return ($output | ConvertFrom-Json -ErrorAction Stop)
    }
    $pr=Api "repos/$repo/pulls/134"
    if (!$pr.merged -or !$pr.merged_at -or $pr.base.ref -cne 'main' -or
        $pr.head.repo.full_name -cne $repo -or $pr.merge_commit_sha -cne $Commit) {
        throw 'STOP: PR #134 not yet merged at reviewed tooling SHA'
    }
    $commitObject=Api "repos/$repo/git/commits/$Commit"
    if ($commitObject.sha -cne $Commit -or $commitObject.tree.sha -cnotmatch '^[a-f0-9]{40}$') {
        throw 'Unexpected Git commit/tree identity'
    }
    $comparison=Api "repos/$repo/compare/$Commit...main"
    if ($comparison.merge_base_commit.sha -cne $Commit -or [int]$comparison.behind_by -ne 0 -or
        $comparison.status -notin @('ahead','identical')) {
        throw 'Reviewed merge commit is not an ancestor of main'
    }
    $tree=Api "repos/$repo/git/trees/$($commitObject.tree.sha)?recursive=1"
    if ($tree.truncated -or $tree.sha -cne $commitObject.tree.sha) {
        throw 'Incomplete or mismatched reviewed tooling tree'
    }
    $blobs=@{}
    foreach($name in $script:BootstrapNames) {
        $path=".github/scripts/$name"
        $matches=@($tree.tree | Where-Object { $_.path -ceq $path -and $_.type -ceq 'blob' })
        if ($matches.Count -ne 1) { throw "Missing exact reviewed script: $name" }
        $entry=Api "repos/$repo/contents/$($path)?ref=$Commit"
        if ($entry.sha -cne $matches[0].sha -or $entry.encoding -cne 'base64') {
            throw "GitHub content/blob identity mismatch: $name"
        }
        $bytes=[Convert]::FromBase64String($entry.content)
        if ((Get-16L1BlobHash $bytes) -cne $entry.sha) {
            throw "Downloaded script failed Git object verification: $name"
        }
        $blobs[$name]=@{Sha=$entry.sha;Bytes=$bytes}
    }
    return $blobs
}
function Assert-16L1OwnerVolume {
    [CmdletBinding()]
    param([string]$OwnerSid)
    if (!(Get-Partition -DriveLetter D -ErrorAction SilentlyContinue)) { throw 'Physical D: required' }
    $disk=Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='D:'" -ErrorAction Stop
    if (!$disk -or $disk.DriveType -ne 3 -or $disk.FileSystem -ne 'NTFS') { throw 'Local NTFS D: required' }
    $node=Get-Item -LiteralPath 'D:\' -Force -ErrorAction Stop
    if (($node.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Linked volume refused' }
    $acl=Get-Acl -LiteralPath 'D:\' -ErrorAction Stop
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin @($OwnerSid,'S-1-5-18','S-1-5-32-544')) {
        throw 'Unsafe volume owner'
    }
    $danger=[int64]([Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles) -bor
      [int64]([Security.AccessControl.FileSystemRights]::Delete) -bor
      [int64]([Security.AccessControl.FileSystemRights]::ChangePermissions) -bor
      [int64]([Security.AccessControl.FileSystemRights]::TakeOwnership)
    foreach($ace in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        if ($ace.AccessControlType -ne 'Allow' -or
            ($ace.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) -ne 0 -or
            $ace.IdentityReference.Value -in @($OwnerSid,'S-1-5-18','S-1-5-32-544')) { continue }
        if (([int64]$ace.FileSystemRights -band $danger) -ne 0) {
            throw 'Unsafe effective ancestor DeleteChild/Modify/ownership rights'
        }
    }
}
function Invoke-16L1OwnerBootstrap {
    [CmdletBinding()]
    param([string]$Commit)
    # No files or ACL changes before approved merged source is verified.
    $blobs=Get-16L1ApprovedBlobs -Commit $Commit
    if (!$IsWindows) { throw 'Windows only' }
    $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
    if (!([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Elevated PowerShell 7 required'
    }
    $sid=$identity.User.Value
    Assert-16L1OwnerVolume -OwnerSid $sid
    $boundary=$script:BootstrapBoundary
    if (!(Test-Path -LiteralPath $boundary)) {
        New-Item -ItemType Directory -Path $boundary -ErrorAction Stop | Out-Null
        # Scoped icacls avoids Set-Acl SACL/SeSecurityPrivilege problem.
        & icacls $boundary '/setowner' ('*'+$sid) | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Failed to assign exact owner of new directory' }
        & icacls $boundary '/inheritance:r' | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Failed to protect newly-created directory ACL' }
        $ownerGrant='*'+$sid+':(OI)(CI)F'
        & icacls $boundary '/grant:r' $ownerGrant '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Failed to grant trusted directory ACL' }
        & icacls $boundary '/remove' '*S-1-3-0' | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Failed to remove default creator-owner from new directory' }
        if ($LASTEXITCODE -ne 0) { throw 'Failed to grant trusted directory ACL' }
    }
    # Existing boundary is NEVER auto-repaired; it must be fully trusted.
    Assert-16L1BootstrapNode -Path $boundary -OwnerSid $sid -Directory -Protected
    Assert-16L1ChildNames -Path $boundary -Allowed @('scripts') -OwnerSid $sid
    $scripts=Join-Path $boundary 'scripts'
    if (!(Test-Path -LiteralPath $scripts)) {
        New-Item -ItemType Directory -Path $scripts -ErrorAction Stop | Out-Null
    }
    Assert-16L1BootstrapNode -Path $scripts -OwnerSid $sid -Directory
    Invoke-16L1Staging -Dir $scripts -Blobs $blobs -Commit $Commit -OwnerSid $sid
    Assert-16L1BootstrapNode -Path $boundary -OwnerSid $sid -Directory -Protected
    Assert-16L1ChildNames -Path $boundary -Allowed @('scripts') -OwnerSid $sid
    Assert-16L1StagingState -Dir $scripts -Blobs $blobs -Commit $Commit -OwnerSid $sid -Complete
    Write-Host "BOOTSTRAP PASS: source $Commit; protected boundary and all three scripts verified. Installer NOT_RUN."
}
if ($MyInvocation.InvocationName -ne '.') {
    Invoke-16L1OwnerBootstrap -Commit $TrustedCommit
}
