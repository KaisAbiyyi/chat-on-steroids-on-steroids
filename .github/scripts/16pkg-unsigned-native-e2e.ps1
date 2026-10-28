# Actual RELEASE-build unsigned Rust + WebView2 Windows x64 native lifecycle.
# Runs solely on ephemeral GitHub-hosted Windows. No test certificate or trust bootstrap.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Unsigned native test requires disposable GitHub-hosted Windows'
}
$source = (git rev-parse HEAD).Trim()
if ($source -ne $env:PR_HEAD_SHA) { throw 'Source SHA mismatch' }
$root = "D:\16pkg-unsigned-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
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
    # This is an ephemeral CI-owned path, bounded to this run/attempt only.
    if ($root -like 'D:\16pkg-unsigned-*') {
        foreach ($p in @($root,"$root-uninstalling")) {
            if (Test-Path -LiteralPath $p) {
                Remove-Item -LiteralPath $p -Force -Recurse -ErrorAction SilentlyContinue
            }
        }
        Get-ChildItem -LiteralPath 'D:\' -Directory -Force -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like ('.cosos-16pkg-'+(Split-Path $root -Leaf)+'-stage-*') } |
            ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force -Recurse -ErrorAction SilentlyContinue }
    }
    if ($mapped) { & "$env:SystemRoot\System32\subst.exe" D: /D }
}