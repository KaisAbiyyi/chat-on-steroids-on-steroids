param([ValidateRange(1,5)][int]$Runs = 2)
$ErrorActionPreference='Stop'
$root='D:\projects\CoS-Rust'
$repo=(& git -C $PSScriptRoot rev-parse --show-toplevel).Trim().Replace('/','\')
$project=Join-Path $repo 'probes\perf-01-webview2'
$evidence=Join-Path $root '.test-tmp\perf01-webview2'
if (-not $repo.StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase) -or
    -not [IO.Path]::GetFullPath($project).Equals([IO.Path]::GetFullPath($PSScriptRoot),[StringComparison]::OrdinalIgnoreCase)) {
  throw 'Expected source-controlled PERF-01 probe inside D: project worktree'
}
foreach($folder in @($evidence,(Join-Path $evidence 'temp'),(Join-Path $evidence 'dotnet-home'),(Join-Path $evidence 'nuget'))){
  [void][IO.Directory]::CreateDirectory($folder)
}
$env:DOTNET_CLI_HOME=Join-Path $evidence 'dotnet-home'
$env:NUGET_PACKAGES=Join-Path $evidence 'nuget'
$env:TEMP=Join-Path $evidence 'temp'
$env:TMP=$env:TEMP
$env:DOTNET_CLI_TELEMETRY_OPTOUT='1'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE='1'
$stamp=[DateTimeOffset]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')
$fixture=Join-Path $evidence ('fixture-'+$stamp+'.json')
$receipt=(& node (Join-Path $project 'create-fixture.mjs') $fixture | ConvertFrom-Json)
if($LASTEXITCODE -ne 0 -or $receipt.messages -ne 6000){throw 'Synthetic fixture generator failed'}
& dotnet publish (Join-Path $project 'NativeBrowserProbe.csproj') -c Release -o (Join-Path $evidence 'publish') /p:BaseIntermediateOutputPath="$evidence\obj\"
if($LASTEXITCODE -ne 0){throw 'Native WebView2 probe build failed'}
$binary=Join-Path $evidence 'publish\NativeBrowserProbe.exe'
$sha=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$gitSha=(git -C $repo rev-parse HEAD).Trim()
foreach($i in 1..$Runs){
  $out=Join-Path $evidence ('native-'+$stamp+'-'+$i)
  $proc=Start-Process -FilePath $binary -ArgumentList @(('"' + $out + '"'),('"' + $fixture + '"'),$receipt.sha256,$gitSha,('"' + (Join-Path $project 'Program.cs') + '"')) -WorkingDirectory (Join-Path $evidence 'publish') -PassThru -Wait
  $file=Join-Path $out 'native-probe-receipt.json'
  if(-not(Test-Path -LiteralPath $file)){throw "Missing native receipt run $i"}
  $data=Get-Content -LiteralPath $file -Raw|ConvertFrom-Json
  if($data.Status -ne 'PASS_SYNTHETIC_WEBVIEW2_ONLY' -or $data.RenderedMessages -ne 6000){
    throw "Native probe run $i failed: $($data.Error)"
  }
  [pscustomobject]@{run=$i;backendRef=$gitSha;nativeExeSha256=$sha;receipt=$file;startupMs=$data.StartupToContentMs;scrollP95Ms=$data.P95ScrollMs;maxTreeWsMiB=$data.MaxWholeTreeWorkingSetMiB} | ConvertTo-Json -Compress
}
