# Security boundary shared by the explicitly authorized local 16L1 runner and hosted negative tests.
# Loading this file does not touch the filesystem or execute any installation.
$script:16L1Boundary = 'D:\CoSoS-16L1-Qualification'
function Get-16L1TrustedSids {
    $self=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    return @($self,'S-1-5-18','S-1-5-32-544')
}
function Assert-16L1Node {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$Path,
          [switch]$ProtectedBoundary,[switch]$WithinBoundary)
    $node=Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $node) {
        if (Test-Path -LiteralPath $Path) { throw "Cannot inspect qualification path: $Path" }
        if ($ProtectedBoundary) { throw "Pre-provisioned protected boundary missing: $Path" }
        return $false
    }
    if (($node.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Linked/reparse qualification path refused: $Path"
    }
    if ($ProtectedBoundary -and !$node.PSIsContainer) {
        throw "Qualification boundary is not a directory: $Path"
    }
    $acl=Get-Acl -LiteralPath $Path -ErrorAction Stop
    $owner=$acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
    Assert-16L1Acl -Acl $acl -OwnerSid $owner -Path $Path -ProtectedBoundary:$ProtectedBoundary -WithinBoundary:$WithinBoundary
    return $true
}
# Pure ACL evaluator is used by the filesystem guard and hostile hosted fixtures.
function Assert-16L1Acl {
    [CmdletBinding()]
    param([Parameter(Mandatory)]$Acl,[Parameter(Mandatory)][string]$OwnerSid,
          [Parameter(Mandatory)][string]$Path,
          [switch]$ProtectedBoundary,[switch]$WithinBoundary)
    $trusted=@(Get-16L1TrustedSids)
    if ($OwnerSid -notin $trusted) { throw "Untrusted qualification NTFS owner: $Path ($OwnerSid)" }
    if ($ProtectedBoundary -and !$acl.AreAccessRulesProtected) {
        throw "Protected qualification boundary inherits ACLs: $Path"
    }
    # DeleteChild on an ancestor can delete/replace an otherwise protected descendant.
    # Plain Write (AddFile/AddSubdirectory/WriteAttributes) on D:\ is NOT DeleteChild.
    $danger=[int64]([Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles) -bor
        [int64]([Security.AccessControl.FileSystemRights]::Delete) -bor
        [int64]([Security.AccessControl.FileSystemRights]::ChangePermissions) -bor
        [int64]([Security.AccessControl.FileSystemRights]::TakeOwnership)
    $aces=$acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])
    $trustedAllow=0
    foreach ($ace in $aces) {
        $sid=$ace.IdentityReference.Value
        if ($ace.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow) {
            if ($ProtectedBoundary) { throw "Unexpected protected-boundary deny ACE: $Path" }
            continue
        }
        if ($sid -in $trusted) {
            $trustedAllow++
            continue
        }
        $rights=[int64]$ace.FileSystemRights
        if ($ProtectedBoundary -or $WithinBoundary) {
            # Never allow sandbox or other untrusted ACEs within the hardened tree.
            throw "Untrusted identity on protected qualification resource: $Path ($sid)"
        }
        if (($rights -band $danger) -ne 0) {
            throw "Qualification ancestor permits deletion/replacement/ACL takeover: $Path ($sid)"
        }
    }
    if ($ProtectedBoundary -and ($trustedAllow -lt 2 -or
        @($aces | Where-Object { $_.IsInherited }).Count -ne 0)) {
        throw "Missing trusted explicit ACLs or inherited rules on protected boundary: $Path"
    }
    return $true
}
function Assert-16L1Workspace {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Workspace,
        [Parameter(Mandatory)][string]$Boundary,
        [switch]$RequirePhysicalD,
        [switch]$InspectTree
    )
    foreach ($value in @($Workspace,$Boundary)) {
        if ($value -match '(^|[\\/])\.\.?([\\/]|$)' -or $value -match '[. ]([\\/]|$)') {
            throw 'Ambiguous or traversal qualification path refused'
        }
    }
    $path=[IO.Path]::GetFullPath($Workspace).TrimEnd('\')
    $boundaryPath=[IO.Path]::GetFullPath($Boundary).TrimEnd('\')
    if (-not $path.StartsWith("$boundaryPath\",[StringComparison]::OrdinalIgnoreCase) -or
        $path.Length -lt ($boundaryPath.Length + 21)) {
        throw 'Workspace must be an isolated child of the qualification boundary'
    }
    $workspaceParent=[IO.Path]::GetDirectoryName($path).TrimEnd('\')
    $workspaceName=[IO.Path]::GetFileName($path)
    if (!$workspaceParent.Equals($boundaryPath,[StringComparison]::OrdinalIgnoreCase) -or
        $workspaceName -cnotmatch '^16L1-[a-f0-9]{40}(-[a-f0-9]{8,32})?$' -or
        $path.Substring([IO.Path]::GetPathRoot($path).Length).Contains(':')) {
        throw 'Only a direct, source-named child workspace is allowed (no ADS or script-tree nesting)'
    }
    if ($RequirePhysicalD) {
        if ($boundaryPath -cne $script:16L1Boundary -or
            [IO.Path]::GetPathRoot($path) -cne 'D:\' -or
            -not (Get-Partition -DriveLetter D -ErrorAction SilentlyContinue)) {
            throw 'Qualification requires dedicated physical D: boundary (no SUBST/network mapping)'
        }
        $disk=Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='D:'" -ErrorAction Stop
        if (!$disk -or $disk.DriveType -ne 3 -or $disk.FileSystem -ne 'NTFS') {
            throw 'Qualification requires a local NTFS D: volume'
        }
    }
    $root=[IO.Path]::GetPathRoot($path)
    [void](Assert-16L1Node -Path $root)
    $current=$root
    foreach ($piece in $path.Substring($root.Length).Split('\')) {
        if (!$piece) { continue }
        $current=Join-Path $current $piece
        $isBoundary=$current.Equals($boundaryPath,[StringComparison]::OrdinalIgnoreCase)
        $within=$current.StartsWith("$boundaryPath\",[StringComparison]::OrdinalIgnoreCase)
        [void](Assert-16L1Node -Path $current -ProtectedBoundary:$isBoundary -WithinBoundary:$within)
    }
    if (!(Test-Path -LiteralPath $boundaryPath -PathType Container)) {
        throw "Protected qualification boundary is not pre-provisioned: $boundaryPath"
    }
    if ($InspectTree -and (Test-Path -LiteralPath $path)) {
        $pending=[Collections.Generic.Stack[string]]::new()
        $pending.Push($path)
        while ($pending.Count -gt 0) {
            $directory=$pending.Pop()
            foreach ($child in @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop)) {
                [void](Assert-16L1Node -Path $child.FullName -WithinBoundary)
                if ($child.PSIsContainer) { $pending.Push($child.FullName) }
            }
        }
    }
    return $path
}
function Get-16L1GitBlobSha([string]$File) {
    $bytes=[IO.File]::ReadAllBytes($File)
    $prefix=[Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)$([char]0)")
    $all=[byte[]]::new($prefix.Length+$bytes.Length)
    [Array]::Copy($prefix,0,$all,0,$prefix.Length)
    [Array]::Copy($bytes,0,$all,$prefix.Length,$bytes.Length)
    return [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($all)).ToLowerInvariant()
}
function Assert-16L1TrustedScripts {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$ScriptRoot,
          [Parameter(Mandatory)][string]$TrustedCommit)
    if ($TrustedCommit -cnotmatch '^[a-f0-9]{40}$') { throw 'Reviewed full tooling commit SHA required' }
    $expectedRoot=Join-Path $script:16L1Boundary 'scripts'
    if (![IO.Path]::GetFullPath($ScriptRoot).TrimEnd('\').Equals($expectedRoot,[StringComparison]::OrdinalIgnoreCase)) {
        throw 'Verification scripts must run from protected D: scripts directory'
    }
    [void](Assert-16L1Workspace -Workspace (Join-Path $script:16L1Boundary ('16L1-' + $script:16L1Approved.SourceSha)) -Boundary $script:16L1Boundary -RequirePhysicalD)
    [void](Assert-16L1Node -Path $expectedRoot -WithinBoundary)
    if (!(Get-Command gh -ErrorAction SilentlyContinue)) { throw 'GitHub CLI needed for independent commit blob verification' }
    $repo=$script:16L1Approved.Repository
    $commitJson=gh api "repos/$repo/git/commits/$TrustedCommit"
    if ($LASTEXITCODE -ne 0 -or !$commitJson) { throw 'Cannot verify pinned tooling commit via GitHub' }
    $commit=$commitJson | ConvertFrom-Json
    if ($commit.sha -cne $TrustedCommit -or $commit.tree.sha -cnotmatch '^[a-f0-9]{40}$') {
        throw 'Unexpected trusted tooling commit identity'
    }
    $treeJson=gh api "repos/$repo/git/trees/$($commit.tree.sha)?recursive=1"
    if ($LASTEXITCODE -ne 0 -or !$treeJson) { throw 'Cannot verify pinned tooling tree via GitHub' }
    $tree=$treeJson | ConvertFrom-Json
    if ($tree.sha -cne $commit.tree.sha -or $tree.truncated) { throw 'Incomplete or changed tooling tree' }
    foreach ($name in @('16pkg-16l1-guards.ps1','16pkg-16l1-artifact-provenance.ps1',
                        '16pkg-16l1-local-qualification.ps1')) {
        $gitPath=".github/scripts/$name"
        $matches=@($tree.tree | Where-Object { $_.path -ceq $gitPath -and $_.type -ceq 'blob' })
        $local=Join-Path $expectedRoot $name
        if ($matches.Count -ne 1 -or !(Test-Path -LiteralPath $local -PathType Leaf)) {
            throw "Missing pinned verification script: $name"
        }
        [void](Assert-16L1Node -Path $local -WithinBoundary)
        if ((Get-16L1GitBlobSha $local) -cne $matches[0].sha) {
            throw "Verification script not identical to reviewed GitHub commit: $name"
        }
    }
}
# Immutable 16PKG-01 build/approval tuple. Do not infer PR event SHA from
# mutable pull_request.merge_commit_sha or from workflow_run.head_sha.
$script:16L1Approved = @{
    Repository='KaisAbiyyi/chat-on-steroids-on-steroids'
    RepositoryId=1424422098
    SourceSha='9939ea0a9fdc592e6821296e07eb0b7db3653adb'
    OriginalEventSha='247e57638a8f5afce93d87fc6d671ce54eaf3ccd'
    MergeCommit='be2727fe437f7878f023ddd3422b1c6ae4f70907'
    PrNumber=131
    RunId=40710902296L
    RunAttempt=1
    WorkflowId=398496557
    ArtifactId=12827415031L
    ArtifactDigest='sha256:38fc612236c093cde03d8134173b9c8f8d76c58203ec8614c80122e23e6b60b7'
    WorkflowPath='.github/workflows/16pkg-native-installer.yml'
    Branch='work/16pkg-01-native-installer-20261027'
}
function Assert-16L1RunOrigin {
    [CmdletBinding()]
    param($Run,$Pull,$Artifacts,$MergedCommit,$Compare,
        [long]$RunId,[int]$RunAttempt,[string]$SourceSha,[string]$Repository)
    $approved=$script:16L1Approved
    if ($Repository -cne $approved.Repository -or $SourceSha -cne $approved.SourceSha -or
        $RunId -ne $approved.RunId -or $RunAttempt -ne $approved.RunAttempt) {
        throw 'Unapproved historical build tuple'
    }
    if ([long]$Run.id -ne $RunId -or [int]$Run.run_attempt -ne $RunAttempt -or
        $Run.status -cne 'completed' -or $Run.conclusion -cne 'success' -or
        $Run.event -cne 'pull_request' -or $Run.path -cne $approved.WorkflowPath -or
        [long]$Run.workflow_id -ne $approved.WorkflowId -or
        $Run.repository.full_name -cne $Repository -or
        [long]$Run.repository.id -ne $approved.RepositoryId -or
        $Run.head_repository.full_name -cne $Repository -or
        [long]$Run.head_repository.id -ne $approved.RepositoryId -or
        $Run.head_branch -cne $approved.Branch -or
        $Run.head_sha -cne $SourceSha -or $Run.head_commit.id -cne $SourceSha) {
        throw 'Unexpected historical GitHub build run identity/event/attempt/source'
    }
    # GitHub legitimately returns pull_requests=[] for some post-merge runs.
    # The independent approved merge commit and DAG ancestry supersede that
    # mutable association, never arbitrary empty PR metadata.
    if ([int]$Pull.number -ne $approved.PrNumber -or !$Pull.merged -or
        !$Pull.merged_at -or $Pull.base.ref -cne 'main' -or
        $Pull.head.sha -cne $SourceSha -or
        $Pull.head.ref -cne $approved.Branch -or
        $Pull.head.repo.full_name -cne $Repository) {
        throw 'Source is not the reviewed, merged PR 131 head'
    }
    # Do NOT consume Pull.merge_commit_sha. GitHub changes its meaning on merge.
    if ($MergedCommit.sha -cne $approved.MergeCommit -or
        @($MergedCommit.parents | Where-Object { $_.sha -ceq $SourceSha }).Count -ne 1 -or
        $Compare.base_commit.sha -cne $SourceSha -or
        $Compare.merge_base_commit.sha -cne $SourceSha -or
        [int]$Compare.behind_by -ne 0 -or $Compare.status -notin @('ahead','identical')) {
        throw 'Approved source is not part of the independently pinned merge on main'
    }
    $name="cosos-16pkg-unsigned-candidate-$SourceSha"
    $matches=@($Artifacts.artifacts | Where-Object {
        [long]$_.id -eq $approved.ArtifactId -and $_.name -ceq $name -and
        [long]$_.workflow_run.id -eq $RunId -and
        [long]$_.workflow_run.repository_id -eq $approved.RepositoryId -and
        $_.workflow_run.head_sha -ceq $SourceSha -and
        $_.digest -ceq $approved.ArtifactDigest -and
        !$_.expired -and [long]$_.size_in_bytes -gt 0
    })
    if ($matches.Count -ne 1 -or @($Artifacts.artifacts | Where-Object { $_.name -ceq $name }).Count -ne 1) {
        throw 'Wrong archived artifact ID, immutable SHA-256, origin or expiry'
    }
}
function Assert-16L1AttestationRun {
    [CmdletBinding()]
    param($Verification,[long]$RunId,[int]$RunAttempt,[string]$Repository,
        [string]$EventDigest='')
    $approved=$script:16L1Approved
    $expected="https://github.com/$Repository/actions/runs/$RunId/attempts/$RunAttempt"
    if ($RunId -ne $approved.RunId -or $RunAttempt -ne $approved.RunAttempt -or
        $Repository -cne $approved.Repository) { throw 'Unapproved attestation run tuple' }
    $valid=@($Verification | Where-Object {
        $cert=$_.verificationResult.signature.certificate
        $statement=$_.verificationResult.statement
        $cert.runInvocationURI -ceq $expected -and
        [string]$cert.sourceRepositoryIdentifier -ceq [string]$approved.RepositoryId -and
        $cert.sourceRepositoryRef -ceq 'refs/pull/131/merge' -and
        $cert.githubWorkflowTrigger -ceq 'pull_request' -and
        $cert.sourceRepositoryDigest -cmatch '^[a-f0-9]{40}$' -and
        $cert.sourceRepositoryDigest -cne $approved.SourceSha -and
        $cert.sourceRepositoryDigest -ceq $approved.OriginalEventSha -and
        (!$EventDigest -or $cert.sourceRepositoryDigest -ceq $EventDigest) -and
        $statement.predicateType -ceq 'https://slsa.dev/provenance/v1' -and
        $statement.predicate.runDetails.metadata.invocationId -ceq $expected
    })
    if ($valid.Count -lt 1) {
        throw 'No valid verified certificate + provenance for original PR run/attempt/ref/digest'
    }
    return [string]$valid[0].verificationResult.signature.certificate.sourceRepositoryDigest
}
