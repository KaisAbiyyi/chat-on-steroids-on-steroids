# Security boundary shared by the explicitly authorized local 16L1 runner and hosted negative tests.
# Loading this file does not touch the filesystem or execute any installation.
function Assert-16L1Node([string]$Path) {
    $node = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -eq $node) {
        if (Test-Path -LiteralPath $Path) { throw "Cannot inspect qualification path: $Path" }
        return $false
    }
    if (($node.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Linked/reparse qualification path refused: $Path"
    }
    $acl = Get-Acl -LiteralPath $Path -ErrorAction Stop
    $owner = $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
    $self = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    if ($owner -notin @($self,'S-1-5-18','S-1-5-32-544')) {
        throw "Qualification ancestor has untrusted NTFS owner: $Path"
    }
    $write = [Security.AccessControl.FileSystemRights]::Write -bor
        [Security.AccessControl.FileSystemRights]::Modify -bor
        [Security.AccessControl.FileSystemRights]::FullControl -bor
        [Security.AccessControl.FileSystemRights]::Delete -bor
        [Security.AccessControl.FileSystemRights]::ChangePermissions -bor
        [Security.AccessControl.FileSystemRights]::TakeOwnership
    foreach ($ace in $acl.Access) {
        if ($ace.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or
            ($ace.FileSystemRights -band $write) -eq 0) { continue }
        $sid = $ace.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
        if ($sid -in @('S-1-1-0','S-1-5-11','S-1-5-32-545','S-1-5-32-546','S-1-5-7')) {
            throw "Qualification ancestor allows broad write access: $Path"
        }
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
    $path = [IO.Path]::GetFullPath($Workspace).TrimEnd('\')
    $boundaryPath = [IO.Path]::GetFullPath($Boundary).TrimEnd('\')
    if (-not $path.StartsWith("$boundaryPath\",[StringComparison]::OrdinalIgnoreCase) -or
        $path.Length -lt ($boundaryPath.Length + 21)) {
        throw 'Workspace must be an isolated child of the qualification boundary'
    }
    foreach ($segment in $path.Split('\')) {
        if ($segment -eq '..' -or $segment -eq '.' -or
            $segment.EndsWith(' ') -or $segment.EndsWith('.')) {
            throw 'Ambiguous or traversal qualification path refused'
        }
    }
    $root = [IO.Path]::GetPathRoot($path)
    if ($RequirePhysicalD) {
        if ($root -cne 'D:\' -or
            -not (Get-Partition -DriveLetter D -ErrorAction SilentlyContinue)) {
            throw 'Qualification requires a physical D: disk partition (no SUBST/network mapping)'
        }
        $disk = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='D:'" -ErrorAction Stop
        if (!$disk -or $disk.DriveType -ne 3 -or $disk.FileSystem -ne 'NTFS') {
            throw 'Qualification requires a local NTFS D: volume'
        }
    }
    [void](Assert-16L1Node $root)
    $current = $root
    foreach ($piece in $path.Substring($root.Length).Split('\')) {
        if (!$piece) { continue }
        $current = Join-Path $current $piece
        [void](Assert-16L1Node $current)
    }
    if ($InspectTree -and (Test-Path -LiteralPath $path)) {
        $pending = [Collections.Generic.Stack[string]]::new()
        $pending.Push($path)
        while ($pending.Count -gt 0) {
            $directory = $pending.Pop()
            foreach ($child in @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop)) {
                [void](Assert-16L1Node $child.FullName)
                if ($child.PSIsContainer) { $pending.Push($child.FullName) }
            }
        }
    }
    return $path
}
# Immutable 16PKG-01 build/approval tuple. Do not infer PR event SHA from
# mutable pull_request.merge_commit_sha or from workflow_run.head_sha.
$script:16L1Approved = @{
    Repository='KaisAbiyyi/chat-on-steroids-on-steroids'
    RepositoryId=1424422098
    SourceSha='9939ea0a9fdc592e6821296e07eb0b7db3653adb'
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
        (!$EventDigest -or $cert.sourceRepositoryDigest -ceq $EventDigest) -and
        $statement.predicateType -ceq 'https://slsa.dev/provenance/v1' -and
        $statement.predicate.runDetails.metadata.invocationId -ceq $expected
    })
    if ($valid.Count -lt 1) {
        throw 'No valid verified certificate + provenance for original PR run/attempt/ref/digest'
    }
    return [string]$valid[0].verificationResult.signature.certificate.sourceRepositoryDigest
}
