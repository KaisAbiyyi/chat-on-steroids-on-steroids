# Cloud-owned 7U1-PREP Windows gate. NO provider launch/login or Send.
param(
    [Parameter(Mandatory=$true)][string]$Repo,
    [Parameter(Mandatory=$true)][string]$ArtifactRoot,
    [Parameter(Mandatory=$true)][string]$Out,
    [Parameter(Mandatory=$true)][ValidateSet('chatgpt','claude','gemini')][string]$Provider,
    [Parameter(Mandatory=$true)][string]$ProfileId,
    [switch]$OwnerConsent
)
$ErrorActionPreference = 'Stop'
function Check-IsolatedD([string]$path) {
    $full = [IO.Path]::GetFullPath($path)
    if (-not $full.StartsWith('D:\projects\CoS-Rust\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Only isolated paths below D:\projects\CoS-Rust are allowed'
    }
    $part = $full
    while ($part) {
        if (Test-Path -LiteralPath $part) {
            $item = Get-Item -LiteralPath $part -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw 'Symlink/junction ancestor refused'
            }
        }
        $parent = [IO.Path]::GetDirectoryName($part)
        if ($parent -eq $part) { break }
        $part = $parent
    }
    return $full
}
$Repo = Check-IsolatedD $Repo
$ArtifactRoot = Check-IsolatedD $ArtifactRoot
$Out = Check-IsolatedD $Out
if (Test-Path -LiteralPath $Out) {
    if (@(Get-ChildItem -LiteralPath $Out -Force).Count -gt 0) {
        throw 'Output must be fresh and empty; no receipt reuse'
    }
}
if ($ProfileId -notmatch '^7u1-[a-zA-Z0-9_-]{8,48}$') {
    throw 'Require a new 7u1-* test profile, not the owner default'
}
$sha = (git -C $Repo rev-parse HEAD).Trim().ToLowerInvariant()
if ($LASTEXITCODE -ne 0 -or $sha -notmatch '^[0-9a-f]{40}$') {
    throw 'Invalid Git source checkout SHA'
}
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$arguments = @((Join-Path $Repo 'probes\7u1\preflight.py'),
    '--repo', $Repo, '--artifact', $ArtifactRoot, '--expected-sha', $sha,
    '--provider', $Provider, '--profile', $ProfileId,
    '--receipt', (Join-Path $Out 'receipt.json'))
if ($OwnerConsent) { $arguments += '--owner-consent' }
python @arguments
if ($LASTEXITCODE -ne 0) {
    throw 'BLOCKED: 7U1 product integration is not qualified. Read sanitized receipt.'
}
Write-Host 'PREP metadata only: real owner 7U1 is still NOT_RUN and needs code/security review.'
