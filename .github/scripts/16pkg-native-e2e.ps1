# Cloud Windows CI only. Disposable self-signed TEST issuer, never production.
$ErrorActionPreference='Stop'
$source=(git rev-parse HEAD).Trim()
if($source -ne $env:PR_HEAD_SHA){throw 'Exact source SHA mismatch'}
$home=Join-Path $env:RUNNER_TEMP 'cosos-16pkg-disposable'
New-Item -ItemType Directory -Force -Path $home | Out-Null
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
        & "$env:SystemRoot\System32\subst.exe" D: $home
        if($LASTEXITCODE -ne 0){throw 'Disposable D drive unavailable'}
        $mapped=$true
    }
    $cert=New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=CoSoS-16PKG-TEST-ONLY-NOT-PRODUCTION' -CertStoreLocation 'Cert:\CurrentUser\My' -HashAlgorithm SHA256 -NotAfter (Get-Date).AddHours(2)
    $public=Join-Path $home 'test-only.cer'
    Export-Certificate -Cert $cert -FilePath $public | Out-Null
    Import-Certificate -FilePath $public -CertStoreLocation 'Cert:\CurrentUser\Root' | Out-Null
    Import-Certificate -FilePath $public -CertStoreLocation 'Cert:\CurrentUser\TrustedPublisher' | Out-Null
    $publisher=Join-Path $home 'publisher-test-only.txt'
    [IO.File]::WriteAllText($publisher,$cert.Thumbprint,[Text.UTF8Encoding]::new($false))
    $shaFile=(Resolve-Path source-sha.txt).Path
    $setups=@{}
    foreach($v in @('0.1.0','0.2.0','0.3.0')){
        $zip=Join-Path $home "bundle-$v.zip"
        $out=Join-Path $home "setup-$v"
        python tools/release_16pkg.py build --binary target/release/cos-rust.exe --renderer-dir renderer-package --license LICENSE --out $zip --source-sha $source --version $v
        if($LASTEXITCODE -ne 0){throw 'Versioned package build failed'}
        dotnet publish apps/windows-installer/CoSoSInstaller.csproj -c Release -r win-x64 "-p:BundlePath=$zip" "-p:SourceShaPath=$shaFile" "-p:PublisherThumbprintPath=$publisher" "-p:DefineConstants=COS_16PKG_TEST" -o $out
        if($LASTEXITCODE -ne 0){throw 'Native setup compile failed'}
        $exe=Join-Path $out 'CoSoSInstaller.exe'
        $signed=Set-AuthenticodeSignature -FilePath $exe -Certificate $cert -HashAlgorithm SHA256
        if($signed.Status -ne 'Valid'){throw "Test signer failed: $($signed.Status)"}
        $id=(& $exe --identity | ConvertFrom-Json)
        if($LASTEXITCODE -ne 0 -or $id.Source -ne $source -or $id.Version -ne $v -or $id.Publisher -ne $cert.Thumbprint){throw 'Signed identity mismatch'}
        $setups[$v]=$exe
    }
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
    ExpectOk $setups['0.3.0'] @('--update',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.3.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.3.0'] @('--finalize',$root,'--publisher',$cert.Thumbprint)
    $env:COSOS_16PKG_FAIL_AT='after_uninstall_rename'
    & $setups['0.3.0'] --uninstall $root --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Fault injection did not terminate uninstall'}
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.3.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    if(Test-Path $root -or (Test-Path "$root-uninstalling")){throw 'Recovery left uninstall tombstone'}
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
    $external=Join-Path $home 'disposable-junction-target'
    New-Item -ItemType Directory -Force -Path $external | Out-Null
    $junction=Join-Path $dirty 'unowned-link'
    New-Item -ItemType Junction -Path $junction -Target $external | Out-Null
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Junction in installed root accepted'}
    Remove-Item -LiteralPath $junction -Force
    $binary=Join-Path $dirty "slots\$($sel.Active)\bin\cos-rust.exe"
    Add-Content -LiteralPath $binary -Value 'test-tamper' -NoNewline
    & $setups['0.3.0'] --recover $dirty --publisher $cert.Thumbprint 2>$null
    if($LASTEXITCODE -eq 0){throw 'Tampered binary passed integrity check'}
    $global:LASTEXITCODE=0
    @{sourceSha=$source;testPublisher=$cert.Thumbprint;testOnly=$true;productionSigned=$false;native16L1Accepted=$false;cleanInstall='PASS';upgrade='PASS';rollback='PASS';reUpdate='PASS';finalize='PASS';uninstall='PASS';binaryTamperRejected=$true} | ConvertTo-Json | Set-Content -Encoding utf8 16pkg-native-test-receipt.json
}
finally {
    Remove-Item Env:\COSOS_16PKG_FAIL_AT -ErrorAction SilentlyContinue
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
