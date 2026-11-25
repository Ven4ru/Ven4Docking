<#
.SYNOPSIS
  Local build of Ven4Docking: agent, panel, editor, launcher APK, NSIS installer and portable zip -> dist\

.DESCRIPTION
  dist\stage\ mirrors the installed folder (%LOCALAPPDATA%\Programs\Ven4Docking):
    ven4docking-agent.exe, Ven4Docking.exe + editor files, panel\ (built page), apk\ven4docking.apk + latest.json
  Results: dist\Ven4Docking-Setup-<ver>.exe, dist\Ven4Docking-<ver>.zip (portable copy of the installed folder),
  dist\ven4docking-launcher-<ver>.apk, dist\SHA256SUMS.
  Releases for users are built only in GitHub Actions (.github/workflows/build.yml); this script is for local tests.

  Needs: Rust (cargo; agent\.cargo\config.toml selects x86_64-pc-windows-gnu + rust-lld), Node.js + npm,
  .NET 10 SDK, JDK 17+ and Android SDK (ANDROID_HOME, or C:\Android\Sdk), NSIS 3 (makensis).

  The APK is the *release* build type (ports 8765/8766 like the installed agent). Without
  ANDROID_KEYSTORE_FILE / ANDROID_KEYSTORE_PASS / ANDROID_KEY_ALIAS it is signed with the debug key.

.PARAMETER Test
  Also run checks: cargo test, npm run lint, dotnet test, launcher JVM tests (gradlew testReleaseUnitTest).

.PARAMETER Clean
  Reinstall panel node_modules (npm ci) even if they exist.

.PARAMETER AgentTarget
  Rust target triple for the agent. Empty = the default from agent\.cargo\config.toml (gnu, local builds);
  CI passes x86_64-pc-windows-msvc.
#>
[CmdletBinding()]
param(
    [switch]$Test,
    [switch]$Clean,
    [string]$AgentTarget = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = $PSScriptRoot
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'stage'
$sw = [Diagnostics.Stopwatch]::StartNew()

function Step([string]$name) {
    Write-Host ''
    Write-Host ("==> {0}  [{1:mm\:ss}]" -f $name, $sw.Elapsed) -ForegroundColor Cyan
}

# Runs a native command and stops the build on a non-zero exit code.
function Run([string]$exe, [string[]]$argList, [string]$dir = $root) {
    Push-Location $dir
    try {
        & $exe @argList
        if ($LASTEXITCODE -ne 0) { throw "$exe $($argList -join ' ') failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
}

function Find-Tool([string]$name, [string[]]$extra) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($p in $extra) { if ($p -and (Test-Path $p)) { return $p } }
    throw "$name not found. See the requirements at the top of build.ps1."
}

# ---------------------------------------------------------------- tools and version
$cargo = Find-Tool 'cargo' @("$env:USERPROFILE\.cargo\bin\cargo.exe")
$cargoBin = Split-Path $cargo
if (-not ($env:PATH -split ';' | Where-Object { $_ -eq $cargoBin })) { $env:PATH = "$cargoBin;$env:PATH" }
$npm = Find-Tool 'npm.cmd' @("$env:ProgramFiles\nodejs\npm.cmd")
$dotnet = Find-Tool 'dotnet' @("$env:ProgramFiles\dotnet\dotnet.exe")
$makensis = Find-Tool 'makensis' @("${env:ProgramFiles(x86)}\NSIS\makensis.exe", "$env:ProgramFiles\NSIS\makensis.exe")
if (-not $env:ANDROID_HOME -and -not $env:ANDROID_SDK_ROOT -and (Test-Path 'C:\Android\Sdk')) {
    $env:ANDROID_HOME = 'C:\Android\Sdk'
}
if (-not $env:ANDROID_HOME -and -not $env:ANDROID_SDK_ROOT) { throw 'Android SDK not found: set ANDROID_HOME.' }

$version = (Select-String -Path (Join-Path $root 'agent\Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
$gradleFile = Get-Content (Join-Path $root 'android\app\build.gradle') -Raw
$apkVersion = [regex]::Match($gradleFile, "versionName\s+'([^']+)'").Groups[1].Value
$apkCode = [int][regex]::Match($gradleFile, 'versionCode\s+(\d+)').Groups[1].Value
Write-Host "Ven4Docking $version (launcher $apkVersion, versionCode $apkCode)"

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage, (Join-Path $stage 'panel'), (Join-Path $stage 'apk') | Out-Null

# ---------------------------------------------------------------- agent
Step 'Agent (cargo build --release)'
$agentDir = Join-Path $root 'agent'
$targetArgs = if ($AgentTarget) { @('--target', $AgentTarget) } else { @() }
Run $cargo (@('build', '--release') + $targetArgs) $agentDir
$agentExe = @(
    $(if ($AgentTarget) { Join-Path $agentDir "target\$AgentTarget\release\ven4docking-agent.exe" }),
    (Join-Path $agentDir 'target\x86_64-pc-windows-gnu\release\ven4docking-agent.exe'),
    (Join-Path $agentDir 'target\release\ven4docking-agent.exe')
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $agentExe) { throw 'ven4docking-agent.exe not found after cargo build' }
Copy-Item $agentExe $stage
if ($Test) { Step 'Agent tests (cargo test)'; Run $cargo (@('test') + $targetArgs) $agentDir }

# ---------------------------------------------------------------- panel
Step 'Panel (npm run build)'
$panelDir = Join-Path $root 'panel'
if ($Clean -or -not (Test-Path (Join-Path $panelDir 'node_modules'))) { Run $npm @('ci', '--no-audit', '--no-fund') $panelDir }
Run $npm @('run', 'build') $panelDir
if ($Test) { Step 'Panel lint'; Run $npm @('run', 'lint') $panelDir }
Copy-Item (Join-Path $panelDir 'dist\*') (Join-Path $stage 'panel') -Recurse

# ---------------------------------------------------------------- editor
Step 'Editor (dotnet publish, self-contained win-x64)'
$editorOut = Join-Path $dist 'editor-publish'
if (Test-Path $editorOut) { Remove-Item $editorOut -Recurse -Force }
Run $dotnet @('publish', 'editor\Ven4Docking.Editor\Ven4Docking.Editor.csproj', '-c', 'Release', '-r', 'win-x64',
    '--self-contained', 'true', '-p:DebugType=none', '-p:GenerateDocumentationFile=false', '-o', $editorOut, '--nologo')
if ($Test) { Step 'Editor tests (dotnet test)'; Run $dotnet @('test', 'editor\Ven4Docking.Editor.sln', '-c', 'Release', '--nologo') }
Copy-Item (Join-Path $editorOut '*') $stage -Recurse

# ---------------------------------------------------------------- launcher APK
Step 'Launcher APK (gradlew assembleRelease)'
$androidDir = Join-Path $root 'android'
Run (Join-Path $androidDir 'gradlew.bat') @('assembleRelease', '--no-daemon', '--console=plain') $androidDir
if ($Test) { Step 'Launcher tests (gradlew testReleaseUnitTest)'; Run (Join-Path $androidDir 'gradlew.bat') @('testReleaseUnitTest', '--no-daemon', '--console=plain') $androidDir }
$apk = Join-Path $androidDir 'app\build\outputs\apk\release\app-release.apk'
if (-not (Test-Path $apk)) { throw "APK not found: $apk" }
$apkTarget = Join-Path $stage 'apk\ven4docking.apk'
Copy-Item $apk $apkTarget
$apkDist = Join-Path $dist "ven4docking-launcher-$apkVersion.apk"
Copy-Item $apk $apkDist -Force
$apkHash = (Get-FileHash $apkTarget -Algorithm SHA256).Hash.ToLowerInvariant()
$latest = [ordered]@{ version = $apkVersion; versionCode = $apkCode; url = '/apk/ven4docking.apk'; sha256 = $apkHash }
[IO.File]::WriteAllText((Join-Path $stage 'apk\latest.json'), ($latest | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))

# ---------------------------------------------------------------- installer and zip
Step 'Installer (makensis)'
$setup = Join-Path $dist "Ven4Docking-Setup-$version.exe"
$zip = Join-Path $dist "Ven4Docking-$version.zip"
Remove-Item $setup, $zip -ErrorAction SilentlyContinue
# /WX: any NSIS warning fails the build
Run $makensis @('/V3', '/WX', "/DVERSION=$version", "/DSTAGE=$stage", "/DOUTFILE=$setup", (Join-Path $root 'installer\Ven4Docking.nsi'))

Step 'Portable zip'
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

$sums = foreach ($f in @($setup, $zip, $apkDist)) {
    '{0}  {1}' -f (Get-FileHash $f -Algorithm SHA256).Hash.ToLowerInvariant(), (Split-Path $f -Leaf)
}
[IO.File]::WriteAllText((Join-Path $dist 'SHA256SUMS'), (($sums -join "`n") + "`n"), [Text.UTF8Encoding]::new($false))

Step 'Done'
foreach ($f in @($setup, $zip, $apkDist)) {
    Write-Host ('{0,-40} {1,8:N1} MB' -f (Split-Path $f -Leaf), ((Get-Item $f).Length / 1MB))
}
Write-Host ('Total time {0:mm\:ss}' -f $sw.Elapsed)
