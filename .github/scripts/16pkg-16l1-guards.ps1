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
function Assert-16L1RunOrigin {
    param($Run,$Pull,$Artifacts,[long]$RunId,[string]$SourceSha,[string]$Repository)
    if ([long]$Run.id -ne $RunId -or $Run.status -ne 'completed' -or
        $Run.conclusion -ne 'success' -or $Run.event -ne 'pull_request' -or
        $Run.path -cne '.github/workflows/16pkg-native-installer.yml' -or
        $Run.repository.full_name -cne $Repository -or
        $Run.head_branch -cne 'work/16pkg-01-native-installer-20261027' -or
        $Run.head_sha -cne $SourceSha -or [int]$Run.run_attempt -lt 1) {
        throw 'Unexpected GitHub build run identity/event/source'
    }
    if ([int]$Pull.number -ne 131 -or $Pull.state -ne 'open' -or
        $Pull.head.sha -cne $SourceSha -or
        $Pull.head.ref -cne 'work/16pkg-01-native-installer-20261027' -or
        $Pull.head.repo.full_name -cne $Repository -or
        $Pull.base.ref -cne 'main' -or
        $Pull.merge_commit_sha -cnotmatch '^[a-f0-9]{40}$') {
        throw 'Expected reviewed PR source SHA/merge ref has changed'
    }
    $linked = @($Run.pull_requests | Where-Object {
        [int]$_.number -eq 131 -and $_.head.sha -ceq $SourceSha -and
        $_.base.ref -ceq 'main'
    })
    if ($linked.Count -ne 1) { throw 'Run is not associated with the exact PR head' }
    $name = "cosos-16pkg-unsigned-candidate-$SourceSha"
    $matches = @($Artifacts.artifacts | Where-Object {
        $_.name -ceq $name -and [long]$_.workflow_run.id -eq $RunId -and
        !$_.expired -and [long]$_.size_in_bytes -gt 0
    })
    if ($matches.Count -ne 1) { throw 'Exact run does not contain a unique valid source-bound artifact' }
}
function Assert-16L1AttestationRun {
    param($Verification,[long]$RunId,[int]$RunAttempt,[string]$Repository)
    $expected = "https://github.com/$Repository/actions/runs/$RunId/attempts/$RunAttempt"
    $matched = @($Verification | Where-Object {
        $_.verificationResult.statement.predicate.runDetails.metadata.invocationId -ceq $expected
    })
    if ($matched.Count -lt 1) { throw 'Verified attestation invocation does not match the selected GitHub run' }
}
