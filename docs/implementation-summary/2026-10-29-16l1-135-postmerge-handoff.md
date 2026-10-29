# CoSoS 16L1 — PR135 immutable artifact / owner-local handoff

**STOP — PHYSICAL 16L1 FAIL / BLOCKED.** PR #135 is DRAFT / UNMERGED.
This follow-up tooling PR #136 is also DRAFT / UNMERGED. Do NOT run any
owner-local command below until both PRs receive independent security
review, are merged on main, and the owner explicitly approves testing.
Cloud verification is never physical acceptance.

This PR does NOT rebuild the installer or change original installer bytes.
Do not reuse the previously failed installer/source/run. No existing
machine ACL, CoS/CoSoS profile, stage or evidence may be changed.

## Exact new immutable approval candidate, NOT production approval

- Repository: KaisAbiyyi/chat-on-steroids-on-steroids; repository ID 1424422098
- Installer PR #135; original reviewed SOURCE HEAD:
  fb1d97a040a28abe74ccbcf5f92e2d33064b85ad
- Original signed OIDC PR synthetic event SHA, independently recovered
  from real attestation: b903132e2c8a32cde5143d40696813fb3233d11d
- Original signed source-ref: refs/pull/135/merge
- Workflow ID 398496557; path .github/workflows/16pkg-native-installer.yml
- Run 40808144696, attempt 1
- Artifact ID 12868909286; exact name:
  cosos-16pkg-unsigned-candidate-fb1d97a040a28abe74ccbcf5f92e2d33064b85ad
- GitHub artifact ZIP digest:
  sha256:07d68ffbd6eb30d444f153c51a6a0430d7019f9c919c141181e267de19e99cd0

Pinned signed receipt + real package digest values:

| File/content | SHA-256 |
|---|---|
| package-receipt.json | 9c473677c03cead075cfbf0bb6015962fdfc8f2e785e5c2334bb95282a56b308 |
| 0.1.0 CoSoSInstaller.exe | 66adbc6160f99730b029d6f8866aba0291ec98f55a48fa08458954faa29cc76b |
| candidate.zip | 2a5187faa62a8c296124c4bb81b1d5078485e4cbec4cf845ed8832279bebd17f |
| 0.2.0 CoSoSInstaller.exe | 93b68e7322a86a7acb556aaa9b705de16725cc09ea29276be0422644970989a4 |
| 0.3.0 CoSoSInstaller.exe | 27a98bf1c0ec90ed420b75012a681671e8886cb87f9b4cf36ed56f04d8cd5ede |
| candidate-0.2.0.zip | eb18b1a5963d85440b88bacb542425d40074e9e5108a8a94a2e0281d7c470aff |
| candidate-0.3.0.zip | e02acf99a856b28a9547147cc4f22286e849efb779aee94cc707d9cf1a2132d2 |
| release manifest | 24b9ddff3cca496aa3708706e4ad9326eb2520d4ce130ab6f4d77a8754aa53ae |
| renderer integrity manifest | c3bf83730594f7ece72bf224d7552c433e711ca510ae320e9b14d2ca6b4c4e95 |
| cos-rust.exe | c8ead7675c13457dd3b57304f2dd0fc86d0cbf6e00d71f75af33576df8f1a5e8 |
| WindowsRenderer.exe | bddec59bc5fc7af92605462c59bf660a3e2cd5d03c2f550254bb3392fa15093e |
| WindowsRenderer.dll | e007030cbf41a457c4bf730465239e766128c4e2f58bf5bee74eaed734ace8c2 |

All extracted executable bytes are verified through the signed receipt,
immutable independently pinned SHA-256 and source-bound release manifests.
Receipt, setup, payload and both update executables also require
individual genuine Sigstore/OIDC attestation proof from original run
and original pull_request event. Archive metadata expiry/size/id are
checked fail-closed through GitHub API.

## Historical evidence must remain untouched

The old physical source 9939ea0a9fdc592e6821296e07eb0b7db3653adb,
run 40710902296/1, native --install EXIT 3 is **FAIL**. Preserve old
D:\CoSoS-16L1-Qualification\16L1-9939ea0a9fdc592e6821296e07eb0b7db3653adb,
its failed protected .cosos-16pkg-*-stage-* directory, original
qualification.json and old scripts directory. Do not delete/migrate them.

The new three trusted scripts are staged separately under
D:\CoSoS-16L1-Qualification\scripts-135. Existing scripts/ and all
existing evidence remain untouched. Any unexpected child/ACL, wrong
ownership, reparse link, file mutation or unfinished staging fails closed.

The REQUIRED NEW workspace is a fresh unused direct child:
D:\CoSoS-16L1-Qualification\16L1-fb1d97a040a28abe74ccbcf5f92e2d33064b85ad-<16-32 lowercase hex nonce>.
The old nonce-less workspace is never valid for this candidate.

## Owner-only bootstrap: ONLY AFTER PR135 AND PR136 REVIEWED + MERGED

This is ONE fail-fast PowerShell 7 command block, not interactive
line-by-line fragments. It resolves the **ACTUAL reviewed tooling
MERGE COMMIT SHA from PR136 after merge**; there is no usable commit
to substitute before that. It verifies the two PRs, protected-main
ancestry, immutable Git tree and bootstrap blob before running any
downloaded code. The in-memory bootstrap then independently verifies
the exact three source files and stages only into scripts-135.

~~~powershell
pwsh -NoProfile -Command {
    $ErrorActionPreference='Stop'
    $repo='KaisAbiyyi/chat-on-steroids-on-steroids'
    function Api([string]$p) {
        $raw=& gh api $p
        if ($LASTEXITCODE -ne 0 -or !$raw) { throw "GitHub API error: $p" }
        return ($raw | ConvertFrom-Json -ErrorAction Stop)
    }
    $sourcePr=Api "repos/$repo/pulls/135"
    $toolPr=Api "repos/$repo/pulls/136"
    if (!$sourcePr.merged -or !$sourcePr.merged_at -or
        $sourcePr.head.sha -cne 'fb1d97a040a28abe74ccbcf5f92e2d33064b85ad' -or
        !$toolPr.merged -or !$toolPr.merged_at -or $toolPr.base.ref -cne 'main') {
        throw 'STOP: approved source/tooling merge missing'
    }
    $tooling=[string]$toolPr.merge_commit_sha
    if ($tooling -cnotmatch '^[a-f0-9]{40}$') { throw 'Bad merged tooling SHA' }
    $commit=Api "repos/$repo/git/commits/$tooling"
    if ($commit.sha -cne $tooling) { throw 'Tooling Git object mismatch' }
    $ancestry=Api "repos/$repo/compare/$tooling...main"
    if ($ancestry.merge_base_commit.sha -cne $tooling -or
        [int]$ancestry.behind_by -ne 0) { throw 'Tooling not merged on protected main' }
    $tree=Api "repos/$repo/git/trees/$($commit.tree.sha)?recursive=1"
    if ($tree.truncated -or $tree.sha -cne $commit.tree.sha) { throw 'Tooling tree mismatch' }
    $path='.github/scripts/16pkg-16l1-135-owner-bootstrap.ps1'
    $matches=@($tree.tree | Where-Object { $_.path -ceq $path -and $_.type -ceq 'blob' })
    if ($matches.Count -ne 1) { throw 'Reviewed bootstrap not found' }
    $entry=Api "repos/$repo/contents/$($path)?ref=$tooling"
    if ($entry.encoding -cne 'base64' -or $entry.sha -cne $matches[0].sha) {
        throw 'Untrusted bootstrap Git object'
    }
    $bytes=[Convert]::FromBase64String($entry.content)
    $prefix=[Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)$([char]0)")
    $all=[byte[]]::new($prefix.Length+$bytes.Length)
    [Array]::Copy($prefix,0,$all,0,$prefix.Length)
    [Array]::Copy($bytes,0,$all,$prefix.Length,$bytes.Length)
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($all)).ToLowerInvariant()
    if ($hash -cne $entry.sha) { throw 'Downloaded bootstrap bytes not pinned' }
    & ([ScriptBlock]::Create([Text.Encoding]::UTF8.GetString($bytes))) -TrustedCommit $tooling
    if (!$?) { throw 'Protected bootstrap did not pass' }
}
~~~

Only an explicit BOOTSTRAP PASS after final script/ACL verification
is success. Failures must never be followed by a success message.

## Owner-only read-only preflight AFTER BOOTSTRAP PASS

The following is ONE fail-fast PowerShell 7 command. It uses the actual
merged tooling SHA (not the PR HEAD), and a NEW random source-named
workspace. **Copy its WORKSPACE= result** for any separately authorized
physical lifecycle; never regenerate the nonce on a subsequent -Execute.

~~~powershell
pwsh -NoProfile -Command {
    $ErrorActionPreference='Stop'
    $repo='KaisAbiyyi/chat-on-steroids-on-steroids'
    $raw=& gh api "repos/$repo/pulls/136"
    if ($LASTEXITCODE -ne 0 -or !$raw) { throw 'Tooling merge lookup failed' }
    $pr=$raw | ConvertFrom-Json
    if (!$pr.merged -or !$pr.merged_at) { throw 'Tooling PR136 not merged' }
    $tooling=[string]$pr.merge_commit_sha
    $source='fb1d97a040a28abe74ccbcf5f92e2d33064b85ad'
    $nonce=[Guid]::NewGuid().ToString('N')
    $workspace="D:\CoSoS-16L1-Qualification\16L1-$source-$nonce"
    $runner='D:\CoSoS-16L1-Qualification\scripts-135\16pkg-16l1-135-local-qualification.ps1'
    & pwsh -NoProfile -File $runner -RunId 40808144696 -SourceSha $source -TrustedCommit $tooling -Workspace $workspace
    if ($LASTEXITCODE -ne 0) { throw 'New source local preflight FAILED' }
    Write-Host "PREFLIGHT ONLY PASS; WORKSPACE=$workspace"
}
~~~

Separately authorized physical lifecycle uses EXACT SAME runner, source,
run, reviewed merged tooling SHA and the SAME WORKSPACE from preflight,
then adds the -Execute switch. Do not use a different nonce. If an
attempt leaves partial stage/evidence, preserve it and stop. Keep
Defender, SmartScreen and UAC active; never import TEST certificates or
touch existing profiles. Review the new qualification.json and actual
install/update/rollback/recover/uninstall outcomes before 16L1 acceptance.

## Hosted Windows review evidence / remaining blockers

Cloud CI checks PowerShell parsing, source/event/run/attempt/artifact/ACL
negative fixtures, valid nonce, tampered bytes, wrong signed source-ref,
digest and workflow, and real original signed PR event SHA. While PR135
is unmerged, the correct live positive ancestry gate outcome is
EXPECTED BLOCKED. After approved PR135 merge, rerun this lightweight
16L1 workflow for positive merged ancestry and live five-proof receipt
verification. No native rebuild is needed.

Do not claim physical 16L1 PASS until owner-local preflight, install,
update, rollback, recovery and uninstall independently succeed.
