# Cloud Windows CI only. Disposable self-signed TEST issuer, never production.
$ErrorActionPreference='Stop'
$source=(git rev-parse HEAD).Trim()
if($source -ne $env:PR_HEAD_SHA){throw 'Exact source SHA mismatch'}
$testHome=Join-Path $env:RUNNER_TEMP 'cosos-16pkg-disposable'
New-Item -ItemType Directory -Force -Path $testHome | Out-Null
$mapped=$false
$cert=$null
$root="D:\16pkg-ci-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
$dirty="$root-dirty"
function ExpectOk([string]$exe,[string[]]$options){
    & $exe @options
    if($LASTEXITCODE -ne 0){throw "Installer rejected valid fixture operation $($options[0])"}
}
try {
    if(!(Test-Path 'D:\')){
        & "$env:SystemRoot\System32\subst.exe" D: $testHome
        if($LASTEXITCODE -ne 0){throw 'Disposable D drive unavailable'}
        $mapped=$true
    }
    Write-Host '16PKG CI TEST: generating ephemeral certificate'
    # All identity material lives only on an ephemeral GitHub-hosted runner.
    # Managed X.509 APIs avoid interactive certificate issuance wizards.
    Write-Host '16PKG CI TEST: creating ephemeral .NET test-only code signer'
    $rsa=[Security.Cryptography.RSA]::Create(2048)
    $request=[Security.Cryptography.X509Certificates.CertificateRequest]::new(
        'CN=CoSoS-16PKG-TEST-ONLY-NOT-PRODUCTION',
        $rsa,[Security.Cryptography.HashAlgorithmName]::SHA256,
        [Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $oidCollection=[Security.Cryptography.OidCollection]::new()
    [void]$oidCollection.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
    $request.CertificateExtensions.Add(
        [Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new(
            $oidCollection,$false))
    $request.CertificateExtensions.Add(
        [Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
            [Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature,$true))
    $generated=$request.CreateSelfSigned(
        [DateTimeOffset]::UtcNow.AddMinutes(-5),[DateTimeOffset]::UtcNow.AddHours(2))
    $pfx=$generated.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx,'')
    $keyFlags=[Security.Cryptography.X509Certificates.X509KeyStorageFlags]::PersistKeySet -bor
        [Security.Cryptography.X509Certificates.X509KeyStorageFlags]::UserKeySet -bor
        [Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable
    $cert=[Security.Cryptography.X509Certificates.X509Certificate2]::new($pfx,'',$keyFlags)
    if (!$cert.HasPrivateKey -or $cert.Thumbprint -notmatch '^[0-9A-F]{40}$') {
        throw 'Managed TEST certificate cannot sign'
    }
    $myStore=[Security.Cryptography.X509Certificates.X509Store]::new(
        'My',[Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser)
    try {
        $myStore.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
        $myStore.Add($cert)
    }
    finally { $myStore.Close() }
    Write-Host '16PKG CI TEST: test signer generated and persisted only in CurrentUser/My'
    $public=Join-Path $testHome 'test-only.cer'
    Export-Certificate -Cert $cert -FilePath $public | Out-Null
    Write-Host '16PKG CI TEST: exported ephemeral TEST public certificate'
    foreach($storeName in @('Root','TrustedPublisher')){
        $store=[Security.Cryptography.X509Certificates.X509Store]::new(
            $storeName,[Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser)
        try {
            $store.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
            $store.Add($cert)
        }
        finally { $store.Close() }
        Write-Host "16PKG CI TEST: imported TEST certificate to CurrentUser/$storeName"
    }
    $publisher=Join-Path $testHome 'publisher-test-only.txt'
    [IO.File]::WriteAllText($publisher,$cert.Thumbprint,[Text.UTF8Encoding]::new($false))
    $shaFile=(Resolve-Path source-sha.txt).Path
    $setups=@{}
    foreach($v in @('0.1.0','0.2.0','0.3.0')){
        Write-Host "16PKG CI TEST: preparing version $v"
        $zip=Join-Path $testHome "bundle-$v.zip"
        $out=Join-Path $testHome "setup-$v"
        python tools/release_16pkg.py build --binary target/release/cos-rust.exe --renderer-dir renderer-package --license LICENSE --out $zip --source-sha $source --version $v
        if($LASTEXITCODE -ne 0){throw 'Versioned package build failed'}
        # Framework-dependent single-file TEST fixture; production is separate.
        $publishArgs = @(
            'publish', 'apps/windows-installer/CoSoSInstaller.csproj',
            '-c', 'Release', '-r', 'win-x64',
            "-p:BundlePath=$zip",
            "-p:SourceShaPath=$shaFile",
            "-p:PublisherThumbprintPath=$publisher",
            '-p:DefineConstants=COS_16PKG_TEST',
            '-p:SelfContained=false',
            '-o', $out
        )
        & dotnet @publishArgs
        if($LASTEXITCODE -ne 0){throw 'Native setup compile failed'}
        $exe=Join-Path $out 'CoSoSInstaller.exe'
        Write-Host "16PKG CI TEST: signing version $v"
        $signed=Set-AuthenticodeSignature -FilePath $exe -Certificate $cert -HashAlgorithm SHA256
        if($signed.Status -ne 'Valid'){throw "Test signer failed: $($signed.Status)"}
        $id=(& $exe --identity | ConvertFrom-Json)
        if($LASTEXITCODE -ne 0 -or $id.Source -ne $source -or $id.Version -ne $v -or $id.Publisher -ne $cert.Thumbprint){throw 'Signed identity mismatch'}
        $setups[$v]=$exe
    }
    Write-Host '16PKG CI TEST: bounded child-process regressions'
    foreach($case in @('large-stdout','large-stderr','oversized-stdout','oversized-stderr','hang','failure','malformed')){
        ExpectOk $setups['0.1.0'] @('--test-runchecked',$case)
    }
    $manifestB64 = & $setups['0.1.0'] --signed-manifest
    if ($LASTEXITCODE -ne 0 -or $manifestB64.Length -lt 65536) {
        throw 'Signed manifest too small to exercise redirected pipe'
    }
    if ([Convert]::FromBase64String($manifestB64).Length -gt 131072) {
        throw 'Signed manifest exceeded legitimate bound'
    }
    Write-Host '16PKG CI TEST: starting signed native lifecycle'
    $env:COSOS_16PKG_FAIL_AT='after_stage_mkdir'
    & $setups['0.1.0'] --install $root --publisher $cert.Thumbprint 2>$null
    if ($LASTEXITCODE -eq 0) { throw 'Unsealed stage crash injection did not terminate' }
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.1.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.1.0'] @('--install',$root,'--publisher',$cert.Thumbprint)
    # Abrupt crash after a complete signed slot rename, before state publish.
    # Root must retain old active slot; --recover removes verified orphan.
    $env:COSOS_16PKG_FAIL_AT='after_slot_publish'
    & $setups['0.2.0'] --update $root --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Fault injection did not terminate update'}
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.1.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.2.0'] @('--update',$root,'--publisher',$cert.Thumbprint)
    $env:COSOS_16PKG_FAIL_AT='after_rollback_publish'
    & $setups['0.2.0'] --rollback $root --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Fault injection did not terminate rollback'}
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.2.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    # High-water version cannot be reset just by rolling back to an older slot.
    & $setups['0.2.0'] --update $root --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Same-version replay after rollback accepted'}
    $global:LASTEXITCODE=0
    ExpectOk $setups['0.3.0'] @('--update',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.3.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.3.0'] @('--finalize',$root,'--publisher',$cert.Thumbprint)
    Write-Host '16PKG CI TEST: crash recovery and atomic uninstall'
    $env:COSOS_16PKG_FAIL_AT='after_uninstall_rename'
    & $setups['0.3.0'] --uninstall $root --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Fault injection did not terminate uninstall'}
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.3.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    if(Test-Path $root -or (Test-Path "$root-uninstalling")){throw 'Recovery left uninstall tombstone'}
    Write-Host '16PKG CI TEST: negative signed-slot tamper matrix'
    ExpectOk $setups['0.1.0'] @('--install',$dirty,'--publisher',$cert.Thumbprint)
    $sel=Get-Content (Join-Path $dirty 'active.json') -Raw | ConvertFrom-Json
    $statePath=Join-Path $dirty 'active.json'
    $stateBytes=[IO.File]::ReadAllBytes($statePath)
    $bad=$sel
    $bad.Publisher=('F' * 40)
    [IO.File]::WriteAllText($statePath,($bad | ConvertTo-Json -Compress))
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Tampered state accepted'}
    [IO.File]::WriteAllBytes($statePath,$stateBytes)
    $release=Join-Path $dirty "slots\$($sel.Active)\release.json"
    $releaseBytes=[IO.File]::ReadAllBytes($release)
    Add-Content -LiteralPath $release -Value 'tamper-manifest' -NoNewline
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Tampered release manifest accepted'}
    [IO.File]::WriteAllBytes($release,$releaseBytes)
    $external=Join-Path $testHome 'disposable-junction-target'
    New-Item -ItemType Directory -Force -Path $external | Out-Null
    $junction=Join-Path $dirty 'unowned-link'
    New-Item -ItemType Junction -Path $junction -Target $external | Out-Null
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Junction in installed root accepted'}
    Remove-Item -LiteralPath $junction -Force
    $authority=Join-Path $dirty "slots\$($sel.Active)\authority\issuer.exe"
    $authorityBytes=[IO.File]::ReadAllBytes($authority)
    [IO.File]::WriteAllBytes($authority,[IO.File]::ReadAllBytes($setups['0.2.0']))
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Different same-publisher signed release proof incorrectly accepted'}
    [IO.File]::WriteAllBytes($authority,$authorityBytes)
    $binary=Join-Path $dirty "slots\$($sel.Active)\bin\cos-rust.exe"
    Add-Content -LiteralPath $binary -Value 'test-tamper' -NoNewline
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Tampered binary passed integrity check'}
    # Self-consistent rehash of mutable binary+manifest+state must STILL fail,
    # because issuer-signed proof embeds the independent original manifest.
    $mutated=Get-Content $release -Raw | ConvertFrom-Json
    $entry=@($mutated.files | Where-Object { $_.path -eq 'bin/cos-rust.exe' })
    if($entry.Count -ne 1){throw 'Missing Rust executable from signed payload'}
    $entry[0].sha256=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
    $entry[0].bytes=(Get-Item -LiteralPath $binary).Length
    [IO.File]::WriteAllText($release,($mutated | ConvertTo-Json -Depth 10 -Compress))
    $alteredState=Get-Content $statePath -Raw | ConvertFrom-Json
    $alteredState.Generation++
    [IO.File]::WriteAllText($statePath,($alteredState | ConvertTo-Json -Compress))
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Rehashed binary+manifest+state improperly trusted'}
    $global:LASTEXITCODE=0
    @{sourceSha=$source;testPublisher=$cert.Thumbprint;testOnly=$true;productionSigned=$false;native16L1Accepted=$false;cleanInstall='PASS';upgrade='PASS';rollback='PASS';reUpdate='PASS';finalize='PASS';uninstall='PASS';binaryTamperRejected=$true;samePublisherWrongReleaseRefused=$true;jointSlotStateManifestTamperRejected=$true} | ConvertTo-Json | Set-Content -Encoding utf8 16pkg-native-test-receipt.json
}
finally {
    Remove-Item Env:\COSOS_16PKG_FAIL_AT -ErrorAction SilentlyContinue
    # The only quarantines allowed to be cleaned here are tied to this CI
    # root's exact run/attempt prefix. Never glob unrelated stage directories.
    foreach($stage in @(Get-ChildItem -LiteralPath 'D:\' -Directory -Force -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like ('.cosos-16pkg-'+(Split-Path $root -Leaf)+'-stage-*') -or
                        $_.Name -like ('.cosos-16pkg-'+(Split-Path $dirty -Leaf)+'-stage-*') })){
        Remove-Item -LiteralPath $stage.FullName -Recurse -Force -ErrorAction SilentlyContinue
    }
    foreach($p in @($root,$dirty,"$root-uninstalling")){
        if($p -like 'D:\16pkg-ci-*' -and (Test-Path $p)){
            Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    if($cert){
        foreach($store in @('Root','TrustedPublisher','My')){
            Get-Item "Cert:\CurrentUser\$store\$($cert.Thumbprint)" -ErrorAction SilentlyContinue | Remove-Item -ErrorAction SilentlyContinue
        }
    }
    if($mapped){& "$env:SystemRoot\System32\subst.exe" D: /D}
}
