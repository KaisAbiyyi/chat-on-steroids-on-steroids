# 7D-FIX native 7L2 qualification; run ONLY on a separately authorized
# physical Windows host with a clean exact-SHA checkout on D:.
# No account login, provider Send or production browser profile is exercised.
param(
    [Parameter(Mandatory=$true)][string]$Repo,
    [Parameter(Mandatory=$true)][string]$ArtifactRoot,
    [Parameter(Mandatory=$true)][string]$Out
)
$ErrorActionPreference = 'Stop'
function Check-ProjectPath([string]$inputPath) {
    $resolved = [System.IO.Path]::GetFullPath($inputPath)
    if (-not $resolved.StartsWith('D:\projects\CoS-Rust\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "7L2 requires an isolated D:\projects\CoS-Rust path"
    }
    return $resolved
}
$Repo = Check-ProjectPath $Repo
$ArtifactRoot = Check-ProjectPath $ArtifactRoot
$Out = Check-ProjectPath $Out
if ((Test-Path -LiteralPath $Out) -and @(Get-ChildItem -LiteralPath $Out -Force).Count -gt 0) {
    throw 'Output directory must be empty; never reuse a prior acceptance receipt'
}
$expected = (Get-Content -Raw -LiteralPath (Join-Path $ArtifactRoot 'source.sha')).Trim()
$actual = (git -C $Repo rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $actual -ne $expected) {
    throw "Artifact/checkout source SHA mismatch: $actual vs $expected"
}
$binary = Join-Path $ArtifactRoot 'host\WindowsHost.dll'
$rust = Join-Path $ArtifactRoot '7l2_bridge.exe'
if (-not (Test-Path -LiteralPath $binary) -or -not (Test-Path -LiteralPath $rust)) {
    throw 'Source-pinned WebView2 host or Rust binary missing'
}
dotnet $binary --7l2 --out $Out --repo $Repo --bridge $rust --artifact-root $ArtifactRoot
$exit = $LASTEXITCODE
$receipt = Join-Path $Out 'receipt.json'
if (-not (Test-Path $receipt)) { throw 'Physical 7L2 receipt missing' }
Get-Content -Raw $receipt
if ($exit -ne 0) { throw "Physical 7L2 process returned exit $exit" }
