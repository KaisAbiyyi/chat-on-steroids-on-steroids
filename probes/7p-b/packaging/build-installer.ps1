param(
    [string]$PublishedRoot = 'D:\projects\CoS-Rust\.test-tmp\7p-l1\installer\publish',
    [string]$OutputRoot = 'D:\projects\CoS-Rust\.test-tmp\7p-l1\installer\release-v2'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Assert-SafeDDrivePath.ps1')
$PublishedRoot = [IO.Path]::GetFullPath($PublishedRoot)
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
foreach ($path in @($PublishedRoot,$OutputRoot)) {
    if ([IO.Path]::GetPathRoot($path) -ne 'D:\') { throw "D: path required: $path" }
    Assert-SafeDDrivePath $path
}
$base = Split-Path $PSScriptRoot -Parent
$renderer = Join-Path $base 'renderer\dist'
$extension = Join-Path $base 'extension'
if (!(Test-Path -LiteralPath (Join-Path $PublishedRoot 'WindowsHost.exe')) -or
    !(Test-Path -LiteralPath (Join-Path $renderer 'index.html')) -or
    !(Test-Path -LiteralPath (Join-Path $extension 'manifest.json'))) {
    throw 'Build publish and renderer first; required fixtures absent.'
}
$work = Join-Path $OutputRoot 'build'
New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
Assert-SafeDDrivePath $OutputRoot
if (Test-Path $work) { throw "Build directory already exists: $work. Use a new OutputRoot." }
New-Item -ItemType Directory -Force -Path $work | Out-Null
Assert-SafeDDrivePath $work
$stage = Join-Path $work 'stage'
$extract = Join-Path $work 'iexpress-source'
New-Item -ItemType Directory -Force -Path $stage,$extract | Out-Null
Copy-Item -Path (Join-Path $PublishedRoot '*') -Destination $stage -Recurse -Force
Copy-Item -LiteralPath $renderer -Destination (Join-Path $stage 'renderer') -Recurse
Copy-Item -LiteralPath $extension -Destination (Join-Path $stage 'extension') -Recurse
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Uninstall-Probe.ps1') -Destination $stage
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Assert-SafeDDrivePath.ps1') -Destination $stage
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'uninstall.cmd') -Destination $stage

# A file-only SHA manifest cannot own empty directories shipped by .NET publish.
# Preserve a hashed list of packaged directories so uninstall can safely remove
# those only when empty, without traversing or deleting unowned directories.
$packagedDirs = @(Get-ChildItem -LiteralPath $stage -Directory -Recurse -Force |
    Sort-Object FullName | ForEach-Object {
        $_.FullName.Substring($stage.Length + 1).Replace('\','/')
    })
[IO.File]::WriteAllLines((Join-Path $stage 'probe-dirs.txt'),$packagedDirs,[Text.UTF8Encoding]::new($false))

$lines = @(Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object FullName | ForEach-Object {
    $rel = $_.FullName.Substring($stage.Length + 1).Replace('\','/')
    "$( (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash ) $rel"
})
[IO.File]::WriteAllLines((Join-Path $stage 'probe-files.sha256'),$lines,[Text.UTF8Encoding]::new($false))
$zip = Join-Path $extract 'payload.zip'
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
[IO.File]::WriteAllText((Join-Path $extract 'payload.sha256'),
    ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash + ' payload.zip'),[Text.UTF8Encoding]::new($false))
$installer = Join-Path $OutputRoot 'CoSoS-7PL1-Synthetic-Probe-Setup.exe'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
if (!(Test-Path -LiteralPath $compiler)) { throw 'Windows .NET Framework C# compiler missing.' }
$args = @(
    '/nologo','/target:exe','/platform:x64','/optimize+',
    ('/out:'+ $installer),
    ('/resource:'+ $zip + ',CoSoS7PL1.Payload'),
    ('/resource:'+ (Join-Path $extract 'payload.sha256') + ',CoSoS7PL1.PayloadHash'),
    '/r:System.IO.Compression.dll','/r:System.IO.Compression.FileSystem.dll',
    (Join-Path $PSScriptRoot 'Installer.cs')
)
& $compiler @args
if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $installer)) { throw "Compiler did not output installer. exit=$LASTEXITCODE" }
& $installer --verify
if ($LASTEXITCODE -ne 0) { throw 'Setup EXE embedded-payload self-verification failed.' }
$sizes=[ordered]@{
    task='7P-L1';type='.NET Framework offline Windows self-extracting setup for SYNTHETIC adapter';
    installer=$installer;installerBytes=(Get-Item $installer).Length;
    installerSha256=(Get-FileHash $installer -Algorithm SHA256).Hash;
    zipBytes=(Get-Item $zip).Length;zipSha256=(Get-FileHash $zip -Algorithm SHA256).Hash;
    publishedBytes=[long](Get-ChildItem -LiteralPath $PublishedRoot -Recurse -File | Measure-Object Length -Sum).Sum;
    packagedFileCount=$lines.Count;needsWebView2Evergreen=$true;
    signed=$false;native7PL1Accepted=$false
}
[IO.File]::WriteAllText((Join-Path $OutputRoot 'distribution.json'),($sizes|ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
Write-Output ($sizes | ConvertTo-Json -Depth 4)
