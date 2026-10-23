#Requires -Version 5.1
<#
.SYNOPSIS
Qualify the physical-Windows 7P-L1 browser feasibility boundary.

.DESCRIPTION
This is the separate L-gate acceptance receipt required by probes/7p-b/README.md.
The embedded synthetic host deliberately leaves two umbrella checks NOT_RUN;
this runner executes those *feasibility* checks using an observed native run
and independently measured distribution fixtures. Production desktop/release
qualification remains a distinct 16L3 gate and is never inferred from them.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$NativeOutputRoot,
    [Parameter(Mandatory = $true)][string]$PublishedExe,
    [Parameter(Mandatory = $true)][string]$InstallRoot,
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$DistributionReceipt,
    [Parameter(Mandatory = $true)][string]$BenchmarkReceipt,
    [Parameter(Mandatory = $true)][string]$OutputReceipt
)
$ErrorActionPreference = 'Stop'
$taskRoot = 'D:\projects\CoS-Rust\.test-tmp\7p-l1'
$expectedSource = '53164da79b32b4a4cacb433678a94ec13de57888'
$expectedExecutableSha256 = '1E972C70DDC74200896EB2E5C607094E2892FDA4DF0C6CCACA63870E1DB3A02C'
$expectedInstallerSha256 = '90CB1FAB5436BCDA01F431A1E998692634DD90BC2EDA1DC450465C4A62295C34'
$expectedDistributionReceiptSha256 = '97808F0CEA4E7F727A3B275D4478259EB9357B273B600364F66442E7F10ACF6D'
$expectedBenchmarkReceiptSha256 = '5F4C92A964900EFF0871D317CA277292B9AEB1D2023FADD805AFDB01894B85C2'
. (Join-Path $PSScriptRoot '..\packaging\Assert-SafeDDrivePath.ps1')

function Inside-TaskRoot([string]$path) {
    $full = [IO.Path]::GetFullPath($path)
    $root = [IO.Path]::GetFullPath($taskRoot).TrimEnd('\')
    if (-not $full.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Qualification file is outside the D: task root: $full"
    }
    Assert-SafeDDrivePath $full
    return $full
}

function File-Hash([string]$path) {
    return (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}

$outputPath = Inside-TaskRoot $OutputReceipt
if (Test-Path -LiteralPath $outputPath) { throw "Refusing to overwrite existing acceptance receipt: $outputPath" }
$nativeRoot = Inside-TaskRoot $NativeOutputRoot
if (Test-Path -LiteralPath $nativeRoot) { throw "Refusing to reuse a native profile/receipt root: $nativeRoot" }
$installRoot = Inside-TaskRoot $InstallRoot
if (Test-Path -LiteralPath $installRoot) { throw "Refusing to reuse an install root: $installRoot" }
$exePath = Inside-TaskRoot $PublishedExe
$installerPath = Inside-TaskRoot $Installer
$distPath = Inside-TaskRoot $DistributionReceipt
$benchmarkPath = Inside-TaskRoot $BenchmarkReceipt
$distribution = Get-Content -LiteralPath $distPath -Raw | ConvertFrom-Json
$benchmark = Get-Content -LiteralPath $benchmarkPath -Raw | ConvertFrom-Json
$publishedExeHash = File-Hash $exePath
$installerHash = File-Hash $installerPath
$actualInstallerBytes = [long](Get-Item -LiteralPath $installerPath).Length
$expectedArtifacts =
    ($publishedExeHash -eq $expectedExecutableSha256) -and
    ($installerHash -eq $expectedInstallerSha256) -and
    ((File-Hash $distPath) -eq $expectedDistributionReceiptSha256) -and
    ((File-Hash $benchmarkPath) -eq $expectedBenchmarkReceiptSha256)
if (-not $expectedArtifacts) { throw 'Pinned source/installer/distribution/benchmark hash mismatch; refusing to qualify different evidence' }

# Qualify a fresh installation from the actual EXE package. Its own archive,
# file manifest and SHA checks must complete before executing the installed host.
& $installerPath --verify | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Package self-verification failed' }
& $installerPath --install $installRoot | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Synthetic package installation failed' }
Assert-SafeDDrivePath $installRoot
$installedExe = Join-Path $installRoot 'WindowsHost.exe'
$assetsPath = Join-Path $installRoot 'renderer'
$extensionPath = Join-Path $installRoot 'extension'
Assert-SafeDDrivePath $assetsPath
Assert-SafeDDrivePath $extensionPath
if (-not (Test-Path -LiteralPath (Join-Path $assetsPath 'index.html')) -or
    -not (Test-Path -LiteralPath (Join-Path $extensionPath 'manifest.json'))) {
    throw 'Installed and verified package is missing bundled renderer/extension'
}
$installedExeHash = File-Hash $installedExe
$installedFileBytes = [long](Get-ChildItem -LiteralPath $installRoot -Recurse -File | Measure-Object -Property Length -Sum).Sum
$installedManifest = @(Get-Content -LiteralPath (Join-Path $installRoot 'probe-files.sha256') | Where-Object { $_.Trim().Length -gt 0 })
if ($installedExeHash -ne $publishedExeHash -or
    $installedManifest.Count -ne [int]$distribution.packagedFileCount -or
    $installedFileBytes -ne [long]$benchmark.installedFileBytes) {
    throw 'Installed payload differs from reviewed published executable, manifest or measured bytes'
}

# An actual Windows WebView2 run is mandatory. Reading an old receipt is never
# enough to pass the physical native gate; capture the child exit code here.
New-Item -ItemType Directory -Path $nativeRoot | Out-Null
$nativeArgs = @('--batch', '--native-idle', '--native-auth',
    '--out', $nativeRoot, '--assets', $assetsPath, '--extension', $extensionPath)
$nativeStartedUtc = [DateTimeOffset]::UtcNow.ToString('o')
$process = Start-Process -FilePath $installedExe -ArgumentList $nativeArgs -PassThru -Wait -WindowStyle Hidden `
    -RedirectStandardOutput (Join-Path $nativeRoot 'stdout.txt') `
    -RedirectStandardError (Join-Path $nativeRoot 'stderr.txt')
$nativeObservedExitCode = $process.ExitCode
$nativePath = Join-Path $nativeRoot 'receipt.json'
if (-not (Test-Path -LiteralPath $nativePath)) {
    throw "Native probe failed to emit its receipt (exit=$nativeObservedExitCode)"
}
$native = Get-Content -LiteralPath $nativePath -Raw | ConvertFrom-Json
$exeHash = $installedExeHash
$rawChecks = @($native.checks.PSObject.Properties)
$rawBad = @($rawChecks | Where-Object {
    $_.Value.status -notin @('PASS','NOT_RUN')
})
$rawUnrun = @($rawChecks | Where-Object { $_.Value.status -eq 'NOT_RUN' } | Select-Object -ExpandProperty Name | Sort-Object)
$expectedUnrun = @('distribution_budgets','native_windows_gate')
$requiredChecks = @(
    'browser_navigation', 'debugger_target', 'extension_debugger_api',
    'extension_disabled_refusal', 'extension_install', 'extension_worlds',
    'mock_oauth_callback', 'mock_oauth_consent_denial', 'mock_oauth_consent_page',
    'mock_oauth_forgery', 'mock_oauth_profile_isolation', 'mv3_lifecycle',
    'native_cdp_target_recreation', 'native_cross_profile_cdp_denial',
    'native_debugger_after_idle', 'native_debugger_reattach',
    'native_forced_mv3_retirement', 'native_navigation_cdp_revocation',
    'native_renderer_dom_interactions', 'native_tab_close_reopen',
    'pdf_math_terminal', 'profile_directories', 'rich_editor',
    'sign_in_constraints', 'tab_profile_isolation', 'worker_message'
)
$missingChecks = @($requiredChecks | Where-Object {
    -not ($native.checks.PSObject.Properties.Name -contains $_) -or
    $native.checks.$_.status -ne 'PASS'
})
$unexpectedChecks = @($rawChecks | Where-Object {
    $requiredChecks -notcontains $_.Name -and $expectedUnrun -notcontains $_.Name
})
$runs = @($benchmark.runs)
$cold = @($runs | Where-Object { $_.mode -eq 'cold' })
$warm = @($runs | Where-Object { $_.mode -eq 'warm' })
$failedBenchmarks = @($runs | Where-Object {
    -not $_.receiptNoFail -or $_.timeout -or $_.passedCheckCount -lt 18 -or
    -not $_.receiptHash -or $_.hostPeakWorkingSetBytes -le 0 -or
    $_.sampledWebView2WorkingSetSumBytes -le 0
})
$syntheticHostOk =
    ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) -and
    ($nativeObservedExitCode -eq 0) -and
    ([Diagnostics.Process]::GetCurrentProcess().SessionId -gt 0) -and
    ($native.runtime -like 'Microsoft Windows*') -and
    ($native.syntheticOnly -eq $true) -and
    ($native.noRealProfilesOrAccounts -eq $true) -and
    ($native.executableVersion -eq "1.0.0+$expectedSource") -and
    ($native.executableSha256 -eq $exeHash) -and
    ($native.webviewRuntime -match '^\d+\.\d+\.\d+\.\d+$') -and
    ($rawBad.Count -eq 0) -and ($missingChecks.Count -eq 0) -and
    ($unexpectedChecks.Count -eq 0) -and
    (@($rawUnrun) -join ',' -eq @($expectedUnrun) -join ',')

$distributionOk =
    ($distribution.task -eq '7P-L1') -and
    ($benchmark.sourceSha -eq $expectedSource) -and
    ($benchmark.installedHostSha256 -eq $installedExeHash) -and
    ($benchmark.installerSha256 -eq $installerHash) -and
    ($distribution.installerSha256 -eq $installerHash) -and
    ([long]$distribution.installerBytes -eq $actualInstallerBytes) -and
    ([long]$benchmark.installerBytes -eq $actualInstallerBytes) -and
    ([long]$benchmark.installedFileBytes -eq $installedFileBytes) -and
    ([long]$benchmark.installedFileBytes -le (200 * 1MB)) -and
    ($actualInstallerBytes -le (80 * 1MB)) -and
    ($distribution.packagedFileCount -gt 0) -and
    ($benchmark.webview2Runtime -eq $native.webviewRuntime) -and
    ($cold.Count -eq 3) -and ($warm.Count -eq 3) -and
    ($failedBenchmarks.Count -eq 0) -and
    ($benchmark.coldBatchMedianMs -gt 0) -and
    ($benchmark.warmBatchMedianMs -gt 0)

$distributionStatus = if ($distributionOk) { 'PASS' } else { 'FAIL' }
$nativeStatus = if ($syntheticHostOk -and $distributionOk) { 'PASS' } else { 'FAIL' }
$result = [ordered]@{
    task = '7P-L1'
    sourceSha = $expectedSource
    capturedUtc = [DateTimeOffset]::UtcNow.ToString('o')
    scope = 'Native Windows embedded WebView2 synthetic FEASIBILITY; not released Rust desktop'
    nativeStartedUtc = $nativeStartedUtc
    observedNativeProcessExitCode = $nativeObservedExitCode
    nativeReceiptSha256 = File-Hash $nativePath
    publishedExecutableSha256 = $exeHash
    actualInstalledExecutableSha256 = $installedExeHash
    actualInstalledFileBytes = $installedFileBytes
    installedManifestFileCount = $installedManifest.Count
    installerSha256 = $installerHash
    benchmarkReceiptSha256 = File-Hash $benchmarkPath
    syntheticSource = $native.source
    noRealProfilesOrAccounts = $native.noRealProfilesOrAccounts
    rawHostNotRun = $rawUnrun
    rawHostUnexpectedChecks = @($unexpectedChecks | Select-Object -ExpandProperty Name)
    checks = [ordered]@{
        distribution_budgets = [ordered]@{
            status = $distributionStatus
            scope = 'FEASIBILITY_PROBE_ONLY'
            candidateInstallerLimitBytes = 80 * 1MB
            candidatePayloadLimitBytes = 200 * 1MB
            installerBytes = $actualInstallerBytes
            installedFileBytes = [long]$benchmark.installedFileBytes
            coldBatchMedianMs = $benchmark.coldBatchMedianMs
            warmBatchMedianMs = $benchmark.warmBatchMedianMs
            peakHostWorkingSetBytes = [long](($runs | Measure-Object -Property hostPeakWorkingSetBytes -Maximum).Maximum)
            sampleWebView2WorkingSetIsNotPrivateRam = $true
            exitCodesForSixHistoricalBenchmarksUnavailable = (@($runs | Where-Object { $null -eq $_.observedExitCode }).Count -gt 0)
            evidence = 'Real SHA-checked probe installer/payload and six measured Windows full synthetic batches; candidate size caps, no product claim.'
        }
        native_windows_gate = [ordered]@{
            status = $nativeStatus
            scope = 'PHYSICAL_WINDOWS_BROWSER_FEASIBILITY'
            observedNativeWebviewRuntime = $native.webviewRuntime
            requiredChecks = $requiredChecks.Count
            missingOrFailedChecks = $missingChecks
            unexpectedNotRun = @($rawUnrun | Where-Object { $expectedUnrun -notcontains $_ })
            evidence = 'Separately executes L-gate aggregation over SHA-bound real Windows WebView2 receipt and measured distribution; later product and owner gates remain separate.'
        }
        production_release_budgets_16L3 = [ordered]@{
            status = 'NOT_RUN'
            evidence = 'Actual Rust desktop package/signature, release startup-to-interactive, unique/private RSS and approved product budgets require Phase 16 production build.'
        }
        owner_provider_login_7U1_8U1 = [ordered]@{
            status = 'HOLD'
            evidence = 'Real provider OAuth/account consent requires later owner gate; no accounts accessed.'
        }
        owner_deferred_os_ime = [ordered]@{
            status = 'HOLD'
            evidence = 'Explicit owner IME-DEFER-20261022 remains scheduled at 8L1/Phase15.'
        }
    }
    native7PL1FeasibilityAccepted = ($nativeStatus -eq 'PASS')
    native7PL1Accepted = ($nativeStatus -eq 'PASS')
    productionDesktopAccepted = $false
    limitations = @(
        'Original host intentionally leaves umbrella flags NOT_RUN; this is the separately required native L-gate receipt.',
        'Published executable is synthetic .NET WebView2 adapter, not Rust GUI.',
        'Synthetic batch runtimes include tests/teardown; no time-to-interactive or private/unique RSS qualification.',
        'Six earlier benchmark exit codes are unavailable; their immutable SHA-256 ledger is pinned and the newly installed native full probe exit was directly observed.'
    )
}
$parent = Split-Path -Parent $outputPath
if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent | Out-Null }
[IO.File]::WriteAllText($outputPath, ($result | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
Write-Output ("7P-L1 distribution_budgets: $distributionStatus (synthetic feasibility)")
Write-Output ("7P-L1 native_windows_gate: $nativeStatus (physical Windows feasibility)")
Write-Output ("Production release 16L3: NOT_RUN")
Write-Output ("Receipt: $outputPath")
if ($nativeStatus -ne 'PASS') { exit 1 }
