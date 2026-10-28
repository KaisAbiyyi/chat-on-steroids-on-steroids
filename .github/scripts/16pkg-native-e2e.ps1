# Cloud Windows CI only. Disposable self-signed TEST issuer, never production.
$ErrorActionPreference='Stop'
$source=(git rev-parse HEAD).Trim()
if($source -ne $env:PR_HEAD_SHA){throw 'Exact source SHA mismatch'}
$testHome=Join-Path $env:RUNNER_TEMP 'cosos-16pkg-disposable'
New-Item -ItemType Directory -Force -Path $testHome | Out-Null
$mapped=$false
$cert=$null
$trustedStores=@()
$root="D:\16pkg-ci-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
$dirty="$root-dirty"
function Invoke-HostedCertutil([string[]]$Options){
    $psi=[Diagnostics.ProcessStartInfo]::new((Join-Path $env:SystemRoot 'System32\certutil.exe'))
    $psi.UseShellExecute=$false
    $psi.CreateNoWindow=$true
    $psi.RedirectStandardOutput=$true
    $psi.RedirectStandardError=$true
    foreach($option in $Options){[void]$psi.ArgumentList.Add($option)}
    $child=[Diagnostics.Process]::Start($psi)
    if(!$child){throw 'Hosted TEST certutil unavailable'}
    try {
        $stdout=$child.StandardOutput.ReadToEndAsync()
        $stderr=$child.StandardError.ReadToEndAsync()
        if(!$child.WaitForExit(15000)){
            $child.Kill($true)
            [void]$child.WaitForExit(5000)
            throw "TEST certutil timeout: $($Options[1]) $($Options[2])"
        }
        [void][Threading.Tasks.Task]::WaitAll(@($stdout,$stderr),5000)
        Write-Host "16PKG TEST trust $($Options[1]) LocalMachine/$($Options[2]) exit=$($child.ExitCode)"
        if($child.ExitCode -ne 0){throw 'Hosted TEST certutil refused operation'}
    } finally { $child.Dispose() }
}
function ExpectOk([string]$exe,[string[]]$options){
    & $exe @options
    if($LASTEXITCODE -ne 0){throw "Installer rejected valid fixture operation $($options[0])"}
}
function ExpectDenied([string]$exe,[string[]]$options){
    & $exe @options 2>$null
    if($LASTEXITCODE -eq 0){throw "Installer accepted forbidden operation $($options[0])"}
    $global:LASTEXITCODE=0
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
    # Separate hosted diagnostic 40642249837 proved LocalMachine Root and
    # TrustedPublisher noninteractive import, Authenticode Valid and cleanup.
    if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted'){
        throw 'TEST trust changes require disposable GitHub-hosted Windows'
    }
    $id=[Security.Principal.WindowsIdentity]::GetCurrent()
    $admin=[Security.Principal.WindowsPrincipal]::new($id).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    Write-Host "16PKG CI TEST: admin=$admin OS=$([Environment]::OSVersion.VersionString)"
    if(!$admin){throw 'TEST trust requires elevated disposable runner'}
    foreach($storeName in @('Root','TrustedPublisher')){
        Invoke-HostedCertutil -Options @('-f','-addstore',$storeName,$public)
        $trustedStores += $storeName
        $store=[Security.Cryptography.X509Certificates.X509Store]::new(
            $storeName,[Security.Cryptography.X509Certificates.StoreLocation]::LocalMachine)
        try {
            $store.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly)
            if(!$store.Certificates.Find(
                    [Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint,
                    $cert.Thumbprint,$false).Count){
                throw 'TEST certificate not found in scoped machine store'
            }
        } finally { $store.Close() }
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
        if($v -eq '0.1.0'){
            Copy-Item -LiteralPath $exe -Destination (Join-Path $testHome 'unsigned-installer.exe')
        }
        Write-Host "16PKG CI TEST: signing version $v"
        $signed=Set-AuthenticodeSignature -FilePath $exe -Certificate $cert -HashAlgorithm SHA256
        if($signed.Status -ne 'Valid'){throw "Test signer failed: $($signed.Status)"}
        $id=(& $exe --identity | ConvertFrom-Json)
        if($LASTEXITCODE -ne 0 -or $id.Source -ne $source -or $id.Version -ne $v -or $id.Publisher -ne $cert.Thumbprint){throw 'Signed identity mismatch'}
        $setups[$v]=$exe
    }
    # An untrusted caller-controlled module path must not affect the real
    # production Authenticode verifier used inside this TEST-compiled binary.
    $originalPsModules=$env:PSModulePath
    $poisonModules=Join-Path $testHome 'untrusted-test-module-path'
    New-Item -ItemType Directory -Path $poisonModules -Force | Out-Null
    $env:PSModulePath=$poisonModules
    Write-Host '16PKG CI TEST: independently checking production signature/proof path'
    foreach($version in @('0.1.0','0.2.0','0.3.0')){
        ExpectOk $setups[$version] @('--test-signed-issuer')
    }
    $env:PSModulePath=$originalPsModules
    # Compile the *production* preprocessor path independently with the same
    # TEST signer. It must reject this otherwise trusted self-signed identity.
    Write-Host '16PKG CI TEST: production binary rejects trusted TEST signer'
    $prodOut=Join-Path $testHome 'production-policy-rejection'
    $prodArgs=@('publish','apps/windows-installer/CoSoSInstaller.csproj',
        '-c','Release','-r','win-x64',
        "-p:BundlePath=$(Join-Path $testHome 'bundle-0.1.0.zip')",
        "-p:SourceShaPath=$shaFile",
        "-p:PublisherThumbprintPath=$publisher",
        '-p:DefineConstants=',
        '-p:SelfContained=false',
        '-o',$prodOut)
    & dotnet @prodArgs
    if($LASTEXITCODE -ne 0){throw 'Production policy fixture build failed'}
    $prodExe=Join-Path $prodOut 'CoSoSInstaller.exe'
    $prodSigned=Set-AuthenticodeSignature -FilePath $prodExe -Certificate $cert -HashAlgorithm SHA256
    if($prodSigned.Status -ne 'Valid'){throw 'Production-policy negative fixture not Authenticode valid'}
    $prodProof=(& $prodExe --identity | ConvertFrom-Json)
    if($LASTEXITCODE -ne 0 -or $prodProof.Source -ne $source -or $prodProof.Publisher -ne $cert.Thumbprint){
        throw 'Production-policy fixture identity invalid'
    }
    ExpectDenied $prodExe @('--install',$root,'--publisher',$cert.Thumbprint)
    if(Test-Path -LiteralPath $root){throw 'Production binary authorized disposable self-signed TEST publisher'}
    Write-Host '16PKG CI TEST: bounded child-process regressions'
    foreach($case in @('large-stdout','large-stderr','oversized-stdout','oversized-stderr','hang','failure','malformed')){
        ExpectOk $setups['0.1.0'] @('--test-runchecked',$case)
    }
    $manifestB64 = & $setups['0.1.0'] --signed-manifest
    # The legitimate 127-file renderer fixture gives a sizeable encoded
    # manifest; the separate --test-runchecked fixture exercises >64KiB.
    if ($LASTEXITCODE -ne 0 -or $manifestB64.Length -lt 32768) {
        throw 'Largest legitimate renderer manifest unexpectedly small'
    }
    if ([Convert]::FromBase64String($manifestB64).Length -gt 131072) {
        throw 'Signed manifest exceeded legitimate bound'
    }
    # Production CheckSignature uses inbox Windows PowerShell 5.1. Confirm
    # both the signer identity and trusted Status under that same engine.
    $env:COSOS_16PKG_SIGCHECK_PATH=$setups['0.1.0']
    try {
        $winPs=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $check='$s=Get-AuthenticodeSignature -LiteralPath $env:COSOS_16PKG_SIGCHECK_PATH;[Console]::WriteLine("WINPS_SIGNATURE_STATUS="+$s.Status+" SIGNER_MATCH="+($s.SignerCertificate.Thumbprint -eq $env:COSOS_16PKG_EXPECTED_SIGNER));if($s.Status -ne "Valid" -or $s.SignerCertificate.Thumbprint -ne $env:COSOS_16PKG_EXPECTED_SIGNER){exit 23}'
        $env:COSOS_16PKG_EXPECTED_SIGNER=$cert.Thumbprint
        $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($check))
        & $winPs -NoProfile -NonInteractive -EncodedCommand $encoded
        if($LASTEXITCODE -ne 0) { throw 'Inbox Windows PowerShell Authenticode trust differs from TEST signing engine' }
    } finally {
        Remove-Item Env:\COSOS_16PKG_SIGCHECK_PATH -ErrorAction SilentlyContinue
        Remove-Item Env:\COSOS_16PKG_EXPECTED_SIGNER -ErrorAction SilentlyContinue
    }
    Write-Host '16PKG CI TEST: starting signed native lifecycle'
    $env:COSOS_16PKG_FAIL_AT='after_stage_protect'
    & $setups['0.1.0'] --install $root --publisher $cert.Thumbprint
    if ($LASTEXITCODE -ne 197) { throw "Expected synthetic stage exit 197; got $LASTEXITCODE" }
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.1.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.1.0'] @('--install',$root,'--publisher',$cert.Thumbprint)
    # Abrupt crash after a complete signed slot rename, before state publish.
    # Root must retain old active slot; --recover removes verified orphan.
    $env:COSOS_16PKG_FAIL_AT='after_slot_publish'
    & $setups['0.2.0'] --update $root --publisher $cert.Thumbprint
    if($LASTEXITCODE -ne 197){throw "Expected synthetic update exit 197; got $LASTEXITCODE"}
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.1.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    ExpectOk $setups['0.2.0'] @('--update',$root,'--publisher',$cert.Thumbprint)
    $env:COSOS_16PKG_FAIL_AT='after_rollback_publish'
    & $setups['0.2.0'] --rollback $root --publisher $cert.Thumbprint
    if($LASTEXITCODE -ne 197){throw "Expected synthetic rollback exit 197; got $LASTEXITCODE"}
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
    & $setups['0.3.0'] --uninstall $root --publisher $cert.Thumbprint
    if($LASTEXITCODE -ne 197){throw "Expected synthetic uninstall exit 197; got $LASTEXITCODE"}
    Remove-Item Env:\COSOS_16PKG_FAIL_AT
    ExpectOk $setups['0.3.0'] @('--recover',$root,'--publisher',$cert.Thumbprint)
    if((Test-Path -LiteralPath $root) -or (Test-Path -LiteralPath "$root-uninstalling")) {
        throw 'Recovery left uninstall tombstone'
    }
    Write-Host '16PKG CI TEST: unsigned, wrong publisher, and unsafe path denials'
    ExpectDenied (Join-Path $testHome 'unsigned-installer.exe') @('--install',$root,'--publisher',$cert.Thumbprint)
    ExpectDenied $setups['0.1.0'] @('--install',$root,'--publisher',('F'*40))
    ExpectDenied $setups['0.1.0'] @('--install','D:\','--publisher',$cert.Thumbprint)
    ExpectDenied $setups['0.1.0'] @('--install',("D:\outside\..\task"),'--publisher',$cert.Thumbprint)
    if(Test-Path -LiteralPath $root){throw 'Denied installer modified clean target'}
    Write-Host '16PKG CI TEST: negative signed-slot tamper matrix'
    ExpectOk $setups['0.1.0'] @('--install',$dirty,'--publisher',$cert.Thumbprint)
    # A matching stage-shaped sibling can belong to somebody else. The
    # recovery scanner must preserve it unless ACL ownership is authenticated.
    $foreignStage=Join-Path 'D:\' ('.cosos-16pkg-'+(Split-Path $dirty -Leaf)+'-stage-'+('b'*32))
    New-Item -ItemType Directory -Path $foreignStage | Out-Null
    ExpectDenied $setups['0.1.0'] @('--recover',$dirty,'--publisher',$cert.Thumbprint)
    if(!(Test-Path -LiteralPath $foreignStage)){throw 'Unowned empty staging directory deleted'}
    Remove-Item -LiteralPath $foreignStage
    $rootAcl=Get-Acl -LiteralPath $dirty
    if(!$rootAcl.AreAccessRulesProtected) {throw 'Installed root still inherits writable ACLs'}
    $owned=Join-Path $dirty 'unowned-keep.bin'
    [IO.File]::WriteAllBytes($owned,[Text.Encoding]::UTF8.GetBytes('UNOWNED_KEPT'))
    ExpectDenied $setups['0.1.0'] @('--uninstall',$dirty,'--publisher',$cert.Thumbprint)
    if(!(Test-Path -LiteralPath $dirty) -or
        [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($owned)) -ne 'UNOWNED_KEPT') {
        throw 'Unowned file lost or changed after refused uninstall'
    }
    Remove-Item -LiteralPath $owned -Force
    $empty=Join-Path $dirty 'unowned-empty-directory'
    New-Item -ItemType Directory -Path $empty | Out-Null
    ExpectDenied $setups['0.1.0'] @('--uninstall',$dirty,'--publisher',$cert.Thumbprint)
    if(!(Test-Path -LiteralPath $empty)){throw 'Unowned directory deleted on refused uninstall'}
    Remove-Item -LiteralPath $empty
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
    $setupHashes=@{}
    foreach($v in @('0.1.0','0.2.0','0.3.0')){
        $setupHashes[$v]=(Get-FileHash -LiteralPath $setups[$v] -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    @{schema='cosos.16pkg.native-test.v1';sourceSha=$source;testPublisher=$cert.Thumbprint;
      windowsVersion=[Environment]::OSVersion.VersionString;powerShellVersion=$PSVersionTable.PSVersion.ToString();
      testOnly=$true;productionSigned=$false;native16L1Accepted=$false;
      testTrustScope='Disposable GitHub-hosted LocalMachine Root/TrustedPublisher';
      testCompiledAuthorization=$true;
      setupSha256=$setupHashes;
      assertions=@{cleanInstall='PASS';upgrade='PASS';rollback='PASS';reUpdate='PASS';
        finalize='PASS';uninstall='PASS';interruptedInstallRecovery='PASS';
        interruptedUpdateRecovery='PASS';interruptedRollbackRecovery='PASS';
        interruptedUninstallRecovery='PASS';signedProofVersions='PASS';rendererLimit127='PASS';
        oversizedRenderer128Refused='PASS';largeStdout='PASS';largeStderr='PASS';
        oversizedStdoutRefused='PASS';oversizedStderrRefused='PASS';hungChildReaped='PASS';
        abnormalExitRefused='PASS';malformedResponseRefused='PASS';
        wrongPublisherRefused='PASS';unsignedInstallerRefused='PASS';unsafeRootRefused='PASS';
        productionRejectsTrustedTestSigner='PASS';
        unownedFilePreserved='PASS';unownedEmptyDirectoryPreserved='PASS';
        unownedStageSiblingPreserved='PASS';
        protectedRootAcl='PASS';stateTamperRefused='PASS';manifestTamperRefused='PASS';
        junctionRefused='PASS';foreignProofRefused='PASS';binaryTamperRefused='PASS';
        jointBinaryManifestStateTamperRefused='PASS';downgradeReplayRefused='PASS'};
      gates=@{cloudNativeE2E='PASS';productionSigning='BLOCKED';physical16L1='NOT_RUN'};
    } | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 16pkg-native-test-receipt.json
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
        $cleanupOk=$true
        foreach($store in @('TrustedPublisher','Root')){
            if($store -in $trustedStores){
                try { Invoke-HostedCertutil -Options @('-f','-delstore',$store,$cert.Thumbprint) }
                catch {
                    Write-Warning "Disposable LocalMachine/$store TEST cleanup failed"
                    $cleanupOk=$false
                }
            }
        }
        Get-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -ErrorAction SilentlyContinue |
            Remove-Item -ErrorAction SilentlyContinue
        foreach($store in @('Root','TrustedPublisher')){
            if(Test-Path -LiteralPath "Cert:\LocalMachine\$store\$($cert.Thumbprint)"){
                Write-Warning "TEST issuer persisted in LocalMachine/$store"
                $cleanupOk=$false
            }
        }
        if(Test-Path -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)"){
            Write-Warning 'TEST signing key persists in CurrentUser/My'
            $cleanupOk=$false
        }
        if(Test-Path -LiteralPath '16pkg-native-test-receipt.json'){
            $receipt=Get-Content -LiteralPath '16pkg-native-test-receipt.json' -Raw |
                ConvertFrom-Json -AsHashtable
            $receipt['certificateCleanup']=if($cleanupOk){'PASS'}else{'FAIL'}
            $receipt | ConvertTo-Json -Depth 8 |
                Set-Content -LiteralPath '16pkg-native-test-receipt.json' -Encoding utf8
            if(!$cleanupOk){throw 'Disposable TEST certificate cleanup incomplete'}
        }
    }
    if($mapped){& "$env:SystemRoot\System32\subst.exe" D: /D}
}
