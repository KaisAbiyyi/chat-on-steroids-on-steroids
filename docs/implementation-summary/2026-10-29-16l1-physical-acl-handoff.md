# 16L1 physical ACL — protected, resumable owner bootstrap

> **STOP: PR #134 is DRAFT / UNMERGED and its independent review is NOT complete.**
> Do not execute ANY owner-local bootstrap/preflight/physical commands yet.
> No approved MERGED tooling SHA exists. Never use the current PR HEAD as
> the tooling SHA or execute the literal placeholder. This handoff only
> becomes actionable after fresh independent review and explicit merge.
> Physical 16L1 status remains BLOCKED / NOT_RUN.

Historical unchanged installer provenance:
- Source SHA: 9939ea0a9fdc592e6821296e07eb0b7db3653adb
- Workflow ID: 398496557; run: 40710902296 attempt 1
- Artifact ID: 12827415031; SHA-256:
  38fc612236c093cde03d8134173b9c8f8d76c58203ec8614c80122e23e6b60b7

## Recovery policy

The owner already has D:\CoSoS-16L1-Qualification and an empty scripts folder
from an interrupted setup. **Do not delete, reset, or rename these directories.**
The bootstrap validates their exact owner SID, protected NTFS ACL with only
owner/SYSTEM/Administrators Full Control, reparse protection, expected path,
existing children and approved Git blob identities. An empty scripts folder,
a subset of already approved script files, or exact fully written staging
members are safe to resume. Successful repetition is idempotent.

An unexpected file, changed/half-written script, bad owner, inherited ACL,
unsafe identity or junction **aborts without modifying or deleting it**.
No automatic destructive cleanup. On a failure, preserve the path for review.
For a new boundary only, a narrowly scoped icacls fallback replaces the
earlier Set-Acl approach that raised SeSecurityPrivilege; it modifies
only the newly created D:\CoSoS-16L1-Qualification folder.
Do not modify D:\, D:\projects or existing CoS/CoSoS trees.

## Owner-only verified bootstrap AFTER REVIEW AND MERGE

Use ONE fail-fast PowerShell 7 invocation, not separate interactive lines:
the entire block starts at pwsh -NoProfile -Command and executes only when
the closing brace is entered. Supply the exact 40-character lowercase
SHA of the independently reviewed **MERGED PR #134** in the $trusted value.
This is a *tooling* commit, not the historical installer source.
The bootstrap code is downloaded from that pinned immutable commit,
verified against its Git tree/blob identities, and executed in memory
rather than executing code stored in the sandbox-writable CoS-Rust tree.
The bootstrap then independently pins all three local verification scripts,
uses non-overwriting atomic staging and reports PASS only after full verification.

~~~powershell
pwsh -NoProfile -Command {
    $ErrorActionPreference='Stop'
    $trusted='<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>'
    if ($trusted -cnotmatch '^[a-f0-9]{40}$') {
        throw 'STOP: reviewed merged tooling SHA required'
    }
    $repo='KaisAbiyyi/chat-on-steroids-on-steroids'
    function Api([string]$path) {
        $result=& gh api $path
        if ($LASTEXITCODE -ne 0 -or !$result) {
            throw "GitHub API failed: $path"
        }
        return ($result | ConvertFrom-Json -ErrorAction Stop)
    }
    $pr=Api "repos/$repo/pulls/134"
    if (!$pr.merged -or !$pr.merged_at -or
        $pr.base.ref -cne 'main' -or $pr.merge_commit_sha -cne $trusted) {
        throw 'STOP: approved PR #134 not merged at supplied tooling SHA'
    }
    $commit=Api "repos/$repo/git/commits/$trusted"
    if ($commit.sha -cne $trusted) { throw 'Commit mismatch' }
    $tree=Api "repos/$repo/git/trees/$($commit.tree.sha)?recursive=1"
    if ($tree.truncated -or $tree.sha -cne $commit.tree.sha) {
        throw 'Git tree mismatch'
    }
    $path='.github/scripts/16pkg-16l1-owner-bootstrap.ps1'
    $matches=@($tree.tree | Where-Object {
        $_.path -ceq $path -and $_.type -ceq 'blob'
    })
    if ($matches.Count -ne 1) { throw 'Missing reviewed bootstrap blob' }
    $entry=Api "repos/$repo/contents/$($path)?ref=$trusted"
    if ($entry.encoding -cne 'base64' -or $entry.sha -cne $matches[0].sha) {
        throw 'Bootstrap Git object mismatch'
    }
    $bytes=[Convert]::FromBase64String($entry.content)
    $prefix=[Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)$([char]0)")
    $all=[byte[]]::new($prefix.Length+$bytes.Length)
    [Array]::Copy($prefix,0,$all,0,$prefix.Length)
    [Array]::Copy($bytes,0,$all,$prefix.Length,$bytes.Length)
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($all)).ToLowerInvariant()
    if ($hash -cne $entry.sha) { throw 'Untrusted bootstrap bytes' }
    & ([ScriptBlock]::Create([Text.Encoding]::UTF8.GetString($bytes))) -TrustedCommit $trusted
    if (!$?) { throw 'Owner bootstrap failed; no success claim' }
}
~~~

Run the block from a separately elevated PowerShell 7 session, after
independent security review and merge only. This wrapper has no file writes
of its own; it never uses a local untrusted script as a trust anchor.
The owner bootstrap checks that the supplied merged PR SHA is an ancestor
of main and verifies all THREE scripts before staging.

**Only the terminal message BOOTSTRAP PASS indicates success.**
Any GH 404, invalid SHA, missing scripts, ACL error, stopped command, or
partial write means STOP, no success. The owner's earlier transcript
was NOT a successful bootstrap.

## Owner-local preflight after bootstrap PASS (not installer execution)

Use this single command only after staging succeeds. Substitute the SAME
reviewed merged tooling SHA, not the historical source SHA.

~~~powershell
pwsh -NoProfile -File 'D:\CoSoS-16L1-Qualification\scripts\16pkg-16l1-local-qualification.ps1' -RunId 40710902296 -SourceSha '9939ea0a9fdc592e6821296e07eb0b7db3653adb' -TrustedCommit '<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>'
~~~

## Separately approved physical installer qualification

Only after an owner-local read-only preflight PASS, explicit physical owner
authorization, and independent security review. Keep Windows Defender,
SmartScreen and UAC enabled. Stop and record any Windows refusal.

~~~powershell
pwsh -NoProfile -File 'D:\CoSoS-16L1-Qualification\scripts\16pkg-16l1-local-qualification.ps1' -RunId 40710902296 -SourceSha '9939ea0a9fdc592e6821296e07eb0b7db3653adb' -TrustedCommit '<REVIEWED_MERGED_TOOLING_COMMIT_40_HEX>' -Execute
~~~

Use disposable protected workspaces only, never existing CoS/CoSoS profiles.
Review the physical evidence/qualification.json before acceptance; hosted
Windows CI is not physical 16L1 qualification. No installer runs in cloud CI.
