# Actual RELEASE-build unsigned Rust + WebView2 Windows x64 native lifecycle.
# Runs solely on ephemeral GitHub-hosted Windows. No test certificate or trust bootstrap.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Unsigned native test requires disposable GitHub-hosted Windows'
}
$source = (git rev-parse HEAD).Trim()
if ($source -ne $env:PR_HEAD_SHA) { throw 'Source SHA mismatch' }
$boundary = "D:\16l1-publish-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
$root = Join-Path $boundary 'installed'
$testHome = Join-Path $env:RUNNER_TEMP 'cosos-16pkg-unsigned'
$mapped = $false
$checks = [ordered]@{}
$setups = @{'0.1.0' = (Resolve-Path 'installer-package/CoSoSInstaller.exe').Path}
$digests = @{}
function Assert([bool]$condition,[string]$label) {
    if (!$condition) { throw "Unsigned native assertion failed: $label" }
    $checks[$label] = 'PASS'
}
function Ok([string]$exe,[string]$operation,[string]$digest,[string]$where) {
    & $exe $operation $where --unsigned-sha256 $digest
    if ($LASTEXITCODE -ne 0) { throw "Unsigned native $operation failed, exit $LASTEXITCODE" }
}
function Denied([string]$exe,[string]$operation,[string]$digest,[string]$where) {
    & $exe $operation $where --unsigned-sha256 $digest 2>$null
    if ($LASTEXITCODE -eq 0) { throw "Unverified $operation accepted" }
    $global:LASTEXITCODE = 0
}
function ActiveState([string]$where) {
    Get-Content -LiteralPath (Join-Path $where 'active.json') -Raw | ConvertFrom-Json
}
function ProtectedStore() {
    $store = @()
    foreach ($s in @('Cert:\CurrentUser\My','Cert:\CurrentUser\Root',
            'Cert:\LocalMachine\Root','Cert:\LocalMachine\TrustedPublisher')) {
        $store += @(Get-ChildItem -LiteralPath $s | ForEach-Object { "$s/$($_.Thumbprint)" })
    }
    return (@($store | Sort-Object) -join '|')
}
try {
    New-Item -ItemType Directory -Path $testHome -Force | Out-Null
    if (!(Test-Path 'D:\')) {
        & "$env:SystemRoot\System32\subst.exe" D: $testHome
        if ($LASTEXITCODE -ne 0) { throw 'Unable to map isolated D drive' }
        $mapped = $true
    }
    if ((Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='D:'").FileSystem -ne 'NTFS') {
        throw 'Protected 16L1 publish regression requires NTFS'
    }
    if (Test-Path -LiteralPath $boundary) { throw 'Unexpected existing protected test boundary' }
    $owner=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    New-Item -ItemType Directory -Path $boundary -ErrorAction Stop | Out-Null
    $icacls=Join-Path $env:SystemRoot 'System32\icacls.exe'
    & $icacls $boundary '/setowner' ('*'+$owner) | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to assign exact protected boundary owner' }
    & $icacls $boundary '/inheritance:r' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to disable protected boundary inheritance' }
    & $icacls $boundary '/grant:r' ('*'+$owner+':(OI)(CI)F') '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-18:(OI)(CI)F' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to grant trusted protected boundary ACL' }
    # Fresh CI-owned directory: remove default CREATOR OWNER/Users ACEs
    # that Windows may install as explicit even after /inheritance:r.
    # Never apply this operation to any pre-existing owner boundary.
    $trusted=@($owner,'S-1-5-32-544','S-1-5-18')
    $initialAcl=Get-Acl -LiteralPath $boundary
    $extras=@($initialAcl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]) |
        Where-Object { $_.IdentityReference.Value -notin $trusted } |
        ForEach-Object { $_.IdentityReference.Value } | Select-Object -Unique)
    foreach($sid in $extras) {
        & $icacls $boundary '/remove' ('*'+$sid) | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "Cannot remove CI-only untrusted ACE $sid" }
    }
    $protectedAcl=Get-Acl -LiteralPath $boundary
    if (!$protectedAcl.AreAccessRulesProtected -or
        $protectedAcl.GetOwner([Security.Principal.SecurityIdentifier]).Value -cne $owner) {
        throw 'Protected boundary owner / inheritance not established'
    }
    $expected=@($owner,'S-1-5-32-544','S-1-5-18')
    foreach ($ace in $protectedAcl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        if ($ace.AccessControlType -ne 'Allow' -or $ace.IsInherited -or
            $ace.IdentityReference.Value -notin $expected) {
            throw "Unexpected broad protected boundary ACE: $($ace.IdentityReference.Value)"
        }
    }
    # Same-volume, protected stage publication probe independent of installer.
    $renameStage=Join-Path $boundary '.ci-publish-probe'
    $renameFinal=Join-Path $boundary 'ci-published-probe'
    New-Item -ItemType Directory -Path $renameStage | Out-Null
    [IO.File]::WriteAllText((Join-Path $renameStage 'marker'),'PROTECTED_RENAME')
    [IO.Directory]::Move($renameStage,$renameFinal)
    if ([IO.File]::ReadAllText((Join-Path $renameFinal 'marker')) -cne 'PROTECTED_RENAME') {
        throw 'Protected directory rename did not preserve contents'
    }
    Remove-Item -LiteralPath $renameFinal -Recurse -Force
    $checks['physical-pattern-protected-parent-rename']='PASS'
    if (Test-Path -LiteralPath $root) { throw 'Disposable root unexpectedly exists' }
    $beforeStores = ProtectedStore
    $digests['0.1.0'] = (Get-FileHash $setups['0.1.0'] -Algorithm SHA256).Hash.ToLowerInvariant()
    foreach ($version in @('0.2.0','0.3.0')) {
        $candidate = "candidate-$version.zip"
        python tools/release_16pkg.py build --binary target/release/cos-rust.exe --renderer-dir renderer-package --license LICENSE --out $candidate --source-sha $source --version $version
        if ($LASTEXITCODE -ne 0) { throw "Unable to construct real product v$version" }
        $package = "installer-unsigned-$version"
        New-Item -ItemType Directory -Path $package -Force | Out-Null
        dotnet publish apps/windows-installer/CoSoSInstaller.csproj -c Release -r win-x64 "-p:BundlePath=$((Resolve-Path $candidate).Path)" "-p:SourceShaPath=$((Resolve-Path source-sha.txt).Path)" -o $package
        if ($LASTEXITCODE -ne 0) { throw "Unable to compile unsigned native v$version" }
        $setups[$version] = (Resolve-Path "$package/CoSoSInstaller.exe").Path
        $digests[$version] = (Get-FileHash $setups[$version] -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    foreach ($version in @('0.1.0','0.2.0','0.3.0')) {
        & $setups[$version] --verify
        if ($LASTEXITCODE -ne 0) { throw "Bundled product verify failed: $version" }
        $identity = (& $setups[$version] --identity | ConvertFrom-Json)
        Assert ($identity.Source -eq $source -and $identity.Version -eq $version -and
            $identity.Publisher -eq '') "identity-$version"
    }
    # A supplied checksum is deliberately an independent user input. Both
    # mismatch and modified PE refuse before touching installed state.
    Denied $setups['0.1.0'] --install ('0' * 64) $root
    Assert (!(Test-Path $root)) 'wrong-digest-does-not-install'
    $corrupt = Join-Path $testHome 'mutated-CoSoSInstaller.exe'
    Copy-Item -LiteralPath $setups['0.1.0'] -Destination $corrupt
    [IO.File]::AppendAllText($corrupt,'CHANGED_UNSIGNED_INSTALLER')
    Denied $corrupt --install $digests['0.1.0'] $root
    Assert (!(Test-Path $root)) 'tampered-installer-refused'
    Ok $setups['0.1.0'] --install $digests['0.1.0'] $root
    $state = ActiveState $root
    $active = Join-Path $root "slots/$($state.Active)"
    Assert ($state.HighWater -eq '0.1.0' -and $state.Publisher -eq 'UNSIGNED-MANUAL-SHA256') 'clean-install-state'
    $runtime = Join-Path $active 'bin/cos-rust.exe'
    $renderer = Join-Path $active 'renderer/WindowsRenderer.exe'
    $smokeData = Join-Path $testHome 'smoke-profile'
    & $runtime --data-dir $smokeData self-test
    Assert ($LASTEXITCODE -eq 0) 'installed-rust-runtime-smoke'
    & $renderer --contract-selftest
    Assert ($LASTEXITCODE -eq 0) 'installed-real-webview2-host-smoke'
    $issuer = Join-Path $active 'authority/issuer.exe'
    $marker = "$issuer.sha256"
    Assert ((Get-Content $marker -Raw).Trim() -eq $digests['0.1.0']) 'protected-issuer-hash'
    $before = (Get-Content (Join-Path $root 'active.json') -Raw)
    Denied $setups['0.2.0'] --update $digests['0.1.0'] $root
    Assert ((Get-Content (Join-Path $root 'active.json') -Raw) -eq $before) 'unverified-update-no-state-change'
    Denied $setups['0.1.0'] --update $digests['0.1.0'] $root
    Assert ((Get-Content (Join-Path $root 'active.json') -Raw) -eq $before) 'same-version-replay-denied'
    Ok $setups['0.2.0'] --update $digests['0.2.0'] $root
    $state = ActiveState $root
    Assert ($state.HighWater -eq '0.2.0' -and $state.Previous) 'upgrade-has-rollback'
    Ok $setups['0.2.0'] --rollback $digests['0.2.0'] $root
    $state = ActiveState $root
    Assert ($state.HighWater -eq '0.2.0' -and $state.Previous) 'rollback-preserves-highwater'
    Denied $setups['0.2.0'] --update $digests['0.2.0'] $root
    $checks['replay-after-rollback-denied'] = 'PASS'
    Ok $setups['0.3.0'] --update $digests['0.3.0'] $root
    Ok $setups['0.3.0'] --recover $digests['0.3.0'] $root
    $state = ActiveState $root
    Assert ($state.HighWater -eq '0.3.0') 'reupdate-after-rollback'
    Ok $setups['0.3.0'] --finalize $digests['0.3.0'] $root
    $state = ActiveState $root
    Assert ($null -eq $state.Previous) 'finalize-old-release'
    $active = Join-Path $root "slots/$($state.Active)"
    $manifest = Join-Path $active 'release.json'
    $original = [IO.File]::ReadAllBytes($manifest)
    try {
        [IO.File]::AppendAllText($manifest,'tamper')
        Denied $setups['0.3.0'] --recover $digests['0.3.0'] $root
        Assert ((ActiveState $root).HighWater -eq '0.3.0') 'tampered-manifest-rejected'
    } finally { [IO.File]::WriteAllBytes($manifest,$original) }
    $marker = Join-Path $active 'authority/issuer.exe.sha256'
    $trustedMarker = [IO.File]::ReadAllBytes($marker)
    try {
        [IO.File]::WriteAllText($marker,('f' * 64))
        Denied $setups['0.3.0'] --recover $digests['0.3.0'] $root
        $checks['tampered-authority-proof-rejected'] = 'PASS'
    } finally { [IO.File]::WriteAllBytes($marker,$trustedMarker) }
    Ok $setups['0.3.0'] --recover $digests['0.3.0'] $root
    Ok $setups['0.3.0'] --recover $digests['0.3.0'] $root
    $checks['repeat-recovery-idempotent'] = 'PASS'
    $unowned = Join-Path $root 'unowned.txt'
    [IO.File]::WriteAllText($unowned,'OWNER FILE')
    Denied $setups['0.3.0'] --uninstall $digests['0.3.0'] $root
    Assert ((Test-Path $unowned) -and (Get-Content $unowned -Raw) -eq 'OWNER FILE') 'unowned-file-preserved'
    Remove-Item -LiteralPath $unowned
    $dir = Join-Path $root 'unowned-empty-directory'
    New-Item -ItemType Directory -Path $dir | Out-Null
    Denied $setups['0.3.0'] --uninstall $digests['0.3.0'] $root
    Assert (Test-Path $dir) 'unowned-directory-preserved'
    Remove-Item -LiteralPath $dir
    $unsafe = Join-Path $root 'unexpected-junction'
    New-Item -ItemType Junction -Path $unsafe -Target $testHome | Out-Null
    Denied $setups['0.3.0'] --recover $digests['0.3.0'] $root
    Assert (Test-Path $testHome) 'junction-rejected-without-target-deletion'
    Remove-Item -LiteralPath $unsafe -Force
    Ok $setups['0.3.0'] --uninstall $digests['0.3.0'] $root
    Assert (!(Test-Path $root) -and !(Test-Path "$root-uninstalling")) 'complete-uninstall'
    Ok $setups['0.3.0'] --recover $digests['0.3.0'] $root
    $checks['repeat-post-uninstall-recovery'] = 'PASS'
    # Kill the *actual release-build executable* in a protected staging
    # window. This does not compile test-only crash hooks into production.
    $crashRoot = "$root-crash"
    function InterruptTransaction([string]$exe,[string]$op,[string]$digest) {
        $stagePrefix = ".cosos-16pkg-$(Split-Path $crashRoot -Leaf)-stage-"
        $stdout=Join-Path $testHome "$op-stdout.txt"
        $stderr=Join-Path $testHome "$op-stderr.txt"
        $process=Start-Process -FilePath $exe -ArgumentList @($op,$crashRoot,'--unsigned-sha256',$digest) -NoNewWindow -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        $interrupted=$false
        try {
            for($probe=0;$probe -lt 400;$probe++) {
                $stage=@(Get-ChildItem -LiteralPath $boundary -Directory -Force |
                    Where-Object { $_.Name.StartsWith($stagePrefix,[StringComparison]::OrdinalIgnoreCase) })
                foreach($entry in $stage) {
                    try {
                        $acl=Get-Acl -LiteralPath $entry.FullName
                        $process.Refresh()
                        if($acl.AreAccessRulesProtected -and !$process.HasExited) {
                            Stop-Process -Id $process.Id -Force
                            $interrupted=$true
                            break
                        }
                    } catch [System.IO.IOException] { }
                }
                if($interrupted) { break }
                $process.Refresh()
                if($process.HasExited) { break }
                Start-Sleep -Milliseconds 50
            }
            if(!$interrupted) {
                throw "No observable protected-stage interruption for $op"
            }
            $process.WaitForExit()
        } finally {
            $process.Dispose()
        }
    }
    InterruptTransaction $setups['0.1.0'] '--install' $digests['0.1.0']
    Ok $setups['0.1.0'] --recover $digests['0.1.0'] $crashRoot
    if(Test-Path $crashRoot) {
        $crashState=ActiveState $crashRoot
        Assert ($crashState.HighWater -eq '0.1.0') 'interrupted-install-committed-state-valid'
    } else {
        $checks['interrupted-install-isolated-state']='PASS'
        Ok $setups['0.1.0'] --install $digests['0.1.0'] $crashRoot
    }
    $checks['real-process-interrupted-install-recovery']='PASS'
    InterruptTransaction $setups['0.2.0'] '--update' $digests['0.2.0']
    Ok $setups['0.2.0'] --recover $digests['0.2.0'] $crashRoot
    $afterCrash=ActiveState $crashRoot
    Assert ($afterCrash.HighWater -in @('0.1.0','0.2.0')) 'interrupted-update-state-valid'
    Ok $setups['0.2.0'] --uninstall $digests['0.2.0'] $crashRoot
    Assert (!(Test-Path $crashRoot)) 'interrupted-update-clean-uninstall'
    $checks['real-process-interrupted-update-recovery']='PASS'
    $afterStores = ProtectedStore
    Assert ($beforeStores -ceq $afterStores) 'no-certificate-store-modification'
    @{
      schema='cosos.16pkg.unsigned-native-product.v1'
      sourceSha=$source
      windowsVersion=[Environment]::OSVersion.VersionString
      nativeProduct=$true
      testCertificateUsed=$false
      publisherAuthenticodeSigned=$false
      physical16L1Accepted=$false
      manuallyProvidedSha256=$digests
      assertions=$checks
    } | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 '16pkg-unsigned-native-receipt.json'
} finally {
    # Hosted ephemeral task-owned boundary only. Owner D: is never accessed.
    if ($boundary -like "D:\16l1-publish-$env:GITHUB_RUN_ID-*") {
        if (Test-Path -LiteralPath $boundary) {
            Remove-Item -LiteralPath $boundary -Force -Recurse -ErrorAction SilentlyContinue
        }
    }
    if ($mapped) { & "$env:SystemRoot\System32\subst.exe" D: /D }
}