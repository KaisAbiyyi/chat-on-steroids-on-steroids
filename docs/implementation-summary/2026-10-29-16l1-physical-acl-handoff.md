# 16L1 protected physical ACL handoff (cloud-only change, physical NOT_RUN)

All commands here are for the owner ONLY AFTER independent security review and merge.
Do not execute physical installer from hosted CI. No existing CoS/CoSoS profiles.
The historical artifact is unchanged: source 9939ea0a9fdc592e6821296e07eb0b7db3653adb,
run 40710902296 attempt 1, artifact 12827415031, archive SHA-256
38fc612236c093cde03d8134173b9c8f8d76c58203ec8614c80122e23e6b60b7.

## 1. Owner: create a new isolated boundary (ELEVATED PowerShell 7)

Stop if the directory already exists; do not blindly trust or reuse it.
Only the NEW dedicated directory is modified. No icacls /reset, no changes
to D:\ or D:\projects, no Defender/SmartScreen/UAC adjustments.

~~~powershell
$ErrorActionPreference='Stop'
$boundary='D:\CoSoS-16L1-Qualification'
if ((Get-Volume -DriveLetter D).FileSystem -ne 'NTFS') { throw 'D: must be NTFS' }
if (Test-Path -LiteralPath $boundary) { throw 'Boundary already exists: STOP' }
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
if (!([Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))) { throw 'Elevated pwsh 7 required' }
$me=$identity.User
$acl=[Security.AccessControl.DirectorySecurity]::new()
$acl.SetAccessRuleProtection($true,$false)
$acl.SetOwner($me)
$inherit=[Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [Security.AccessControl.InheritanceFlags]::ObjectInherit
foreach ($sid in @($me,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'),[Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))) {
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,[Security.AccessControl.FileSystemRights]::FullControl,$inherit,[Security.AccessControl.PropagationFlags]::None,[Security.AccessControl.AccessControlType]::Allow))
}
New-Item -ItemType Directory -Path $boundary -ErrorAction Stop | Out-Null
Set-Acl -LiteralPath $boundary -AclObject $acl -ErrorAction Stop
$check=Get-Acl -LiteralPath $boundary
if (!$check.AreAccessRulesProtected -or $check.GetOwner([Security.Principal.SecurityIdentifier]).Value -cne $me.Value) { throw 'Protected boundary ACL failed' }
icacls $boundary
~~~

If interrupted, STOP and investigate; do not run from reused/untrusted path.

## 2. Owner: stage verified GitHub scripts, with exact REVIEWED tooling commit

Fill in $trustedCommit with the full reviewed commit SHA after independent
review and merge. It is NOT the historical installer source SHA. GitHub's
immutable commit/tree identities and every downloaded Git blob digest are checked
before any downloaded script is executed. Never stage from mutable main,
sandbox-writable CoS-Rust, or a PR branch not yet approved.

~~~powershell
$ErrorActionPreference='Stop'
$trustedCommit='<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>'
if ($trustedCommit -cnotmatch '^[a-f0-9]{40}$') { throw 'Full tooling commit required' }
$repo='KaisAbiyyi/chat-on-steroids-on-steroids'
$boundary='D:\CoSoS-16L1-Qualification'
$commit=gh api "repos/$repo/git/commits/$trustedCommit" | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $commit.sha -cne $trustedCommit) { throw 'Commit identity failure' }
$tree=gh api "repos/$repo/git/trees/$($commit.tree.sha)?recursive=1" | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $tree.sha -cne $commit.tree.sha -or $tree.truncated) { throw 'Tree identity failure' }
$scripts=Join-Path $boundary 'scripts'
if (Test-Path -LiteralPath $scripts) { throw 'Scripts already exist: STOP' }
New-Item -ItemType Directory -Path $scripts -ErrorAction Stop | Out-Null
foreach ($name in @('16pkg-16l1-guards.ps1','16pkg-16l1-artifact-provenance.ps1','16pkg-16l1-local-qualification.ps1')) {
    $repoPath=".github/scripts/$name"
    $match=@($tree.tree | Where-Object { $_.path -ceq $repoPath -and $_.type -ceq 'blob' })
    if ($match.Count -ne 1) { throw "Missing pinned script: $name" }
    $remote=gh api "repos/$repo/contents/$($repoPath)?ref=$trustedCommit" | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $remote.sha -cne $match[0].sha -or $remote.encoding -cne 'base64') { throw "Wrong script source: $name" }
    $bytes=[Convert]::FromBase64String($remote.content)
    $prefix=[Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)$([char]0)")
    $buffer=[byte[]]::new($prefix.Length+$bytes.Length)
    [Array]::Copy($prefix,0,$buffer,0,$prefix.Length)
    [Array]::Copy($bytes,0,$buffer,$prefix.Length,$bytes.Length)
    $blob=[Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($buffer)).ToLowerInvariant()
    if ($blob -cne $match[0].sha) { throw "Downloaded bytes mismatch: $name" }
    [IO.File]::WriteAllBytes((Join-Path $scripts $name),$bytes)
}
Write-Host "Reviewed scripts staged from $trustedCommit; installer not executed"
~~~

The runner independently verifies ALL THREE scripts again against GitHub's exact
commit/tree/blob identity before any artifact download. No scripts stored in a
sandbox-writable checkout are trusted. This assumes GitHub API/TLS, reviewed
commit identity, owner/admin/SYSTEM and host OS are trustworthy; local admins
capable of changing ACLs are outside the sandbox threat boundary. Git blob hash
is not a substitute for independent commit review or publisher signing.

## 3. Owner: READ-ONLY preflight (PowerShell 7, no installer execution)

~~~powershell
$source='9939ea0a9fdc592e6821296e07eb0b7db3653adb'
$run=40710902296
$trustedCommit='<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>'
$runner='D:\CoSoS-16L1-Qualification\scripts\16pkg-16l1-local-qualification.ps1'
pwsh -NoProfile -File $runner -RunId $run -SourceSha $source -TrustedCommit $trustedCommit
~~~

This step verifies NTFS ACL, owners, reparse paths, approved merged source,
exact GitHub run/workflow, artifact ID and digest, verified signed OIDC origin,
and local package hashes. Stop on any failure. Preflight PASS is NOT physical PASS.

## 4. Owner: PHYSICAL qualification (only after independent review and approval)

Run in an elevated PowerShell 7 console, only after the read-only preflight
succeeds, the owner approves installation and protection prompts, and independent
security review is complete.

~~~powershell
pwsh -NoProfile -File 'D:\CoSoS-16L1-Qualification\scripts\16pkg-16l1-local-qualification.ps1' -RunId 40710902296 -SourceSha '9939ea0a9fdc592e6821296e07eb0b7db3653adb' -TrustedCommit '<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>' -Execute
~~~

Same workspace and immutable tuple. Executes only disposable installation/profile
under protected D:\CoSoS-16L1-Qualification, tests install, smoke, update,
rollback, recovery, finalize, uninstall and records evidence\qualification.json.
No existing CoS/CoSoS profiles, certificate import, SmartScreen bypass or
global ACL edits. Preserve evidence for independent owner review.
Physical status is BLOCKED / NOT_RUN until actually passed and reviewed.
