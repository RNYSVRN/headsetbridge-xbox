<#
.SYNOPSIS
  Baut HeadsetBridge: Audio-Engine (Rust), WinUI-3-App (.NET), Setup (NSIS) und portable ZIP.

.DESCRIPTION
  Benötigt: Rust (GNU-Toolchain), CMake, Ninja, MinGW-w64 (gcc), .NET 8 SDK, NSIS.
  Installation z. B. mit winget:
    winget install Rustlang.Rustup Kitware.CMake Ninja-build.Ninja BrechtSanders.WinLibs.POSIX.UCRT NSIS.NSIS Microsoft.DotNet.SDK.8
    rustup toolchain install stable-x86_64-pc-windows-gnu
  Ergebnis unter dist\: HeadsetBridge-Setup-<Version>.exe und HeadsetBridge-<Version>-portable.zip.

.PARAMETER SkipTests
  Rust- und .NET-Tests überspringen.
#>
param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'dist'),
    [switch]$SkipTests
)
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$version = '2.0.0'

# Mit winget installierte Werkzeuge sind erst nach einer neuen Anmeldung im PATH.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:Path
foreach ($nsis in "${env:ProgramFiles(x86)}\NSIS", "$env:ProgramFiles\NSIS") { if (Test-Path "$nsis\makensis.exe") { $env:Path += ";$nsis" } }
foreach ($tool in 'cargo', 'cmake', 'ninja', 'gcc', 'dotnet', 'makensis', 'tar') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "Build-Werkzeug fehlt: $tool" }
}
function Invoke-Checked([string]$what, [scriptblock]$command) {
    # Native Werkzeuge (cargo, cmake) schreiben Fortschritt auf stderr; nur der Exit-Code zählt.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & $command 2>&1 | ForEach-Object { "$_" } } finally { $ErrorActionPreference = $previous }
    if ($LASTEXITCODE -ne 0) { throw "$what fehlgeschlagen (Exit-Code $LASTEXITCODE)" }
}

$build = Join-Path $root 'build'
New-Item -ItemType Directory -Force -Path $build, $OutputDirectory | Out-Null

# 1. libopus 1.5.2 (statisch, mit Prüfsumme).
$opusArchive = Join-Path $build 'opus-1.5.2.tar.gz'
$opusSource = Join-Path $build 'opus-1.5.2'
$opusInstall = Join-Path $build 'opus-install'
if (-not (Test-Path (Join-Path $opusInstall 'lib\libopus.a'))) {
    if (-not (Test-Path $opusArchive)) { Invoke-WebRequest 'https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz' -OutFile $opusArchive }
    if ((Get-FileHash $opusArchive).Hash -ne '65C1D2F78B9F2FB20082C38CBE47C951AD5839345876E46941612EE87F9A7CE1') { throw 'Opus-Prüfsumme stimmt nicht' }
    Invoke-Checked 'Opus entpacken' { tar -xzf $opusArchive -C $build }
    Invoke-Checked 'Opus konfigurieren' { cmake -S $opusSource -B (Join-Path $build 'opus-build') -G Ninja -DCMAKE_BUILD_TYPE=Release "-DCMAKE_INSTALL_PREFIX=$opusInstall" -DCMAKE_C_COMPILER=gcc -DOPUS_BUILD_TESTING=OFF -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_SHARED_LIBRARY=OFF }
    Invoke-Checked 'Opus bauen' { cmake --build (Join-Path $build 'opus-build') --parallel }
    Invoke-Checked 'Opus installieren' { cmake --install (Join-Path $build 'opus-build') }
}

# 2. Audio-Engine.
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
$env:RUSTFLAGS = '-C link-self-contained=yes'
$env:LIBOPUS_LIB_DIR = $opusInstall; $env:LIBOPUS_STATIC = '1'; $env:LIBOPUS_NO_PKG = '1'
$engineManifest = Join-Path $root 'engine\Cargo.toml'
if (-not $SkipTests) { Invoke-Checked 'Engine-Tests' { cargo test --locked --manifest-path $engineManifest } }
Invoke-Checked 'Engine-Build' { cargo build --locked --release --manifest-path $engineManifest }

# 3. Lizenzhinweise aus den tatsächlich verwendeten Komponenten.
$appProject = Join-Path $root 'src\HeadsetBridge.App\HeadsetBridge.App.csproj'
Invoke-Checked 'NuGet-Restore' { dotnet restore $appProject -r win-x64 }
& (Join-Path $root 'tools\Update-Licenses.ps1') -OpusSource $opusSource

# 4. App: Tests und Veröffentlichung (self-contained, x64). Die Engine kopiert das csproj nach engine\.
if (-not $SkipTests) { Invoke-Checked '.NET-Tests' { dotnet test (Join-Path $root 'tests\HeadsetBridge.Core.Tests') } }
$appOut = Join-Path $build 'app'
if (Test-Path $appOut) { Remove-Item -Recurse -Force $appOut }
Invoke-Checked 'App-Veröffentlichung' { dotnet publish $appProject -c Release -r win-x64 -p:Platform=x64 --self-contained -o $appOut }
if (-not (Test-Path (Join-Path $appOut 'engine\headsetbridge-engine.exe'))) { throw 'Die Engine fehlt im veröffentlichten App-Ordner' }
Get-ChildItem $appOut -Recurse -Filter *.pdb | Remove-Item

# 5. Setup: Liste der installierten Dateien für die Deinstallation, dann NSIS.
$lines = foreach ($f in Get-ChildItem $appOut -Recurse -File) { "  Delete `"`$INSTDIR\$($f.FullName.Substring($appOut.Length + 1))`"" }
$lines += foreach ($d in Get-ChildItem $appOut -Recurse -Directory | Sort-Object { $_.FullName.Length } -Descending) { "  RMDir `"`$INSTDIR\$($d.FullName.Substring($appOut.Length + 1))`"" }
[System.IO.File]::WriteAllLines((Join-Path $root 'installer\uninstall-files.nsh'), [string[]]$lines, [System.Text.UTF8Encoding]::new($true))
$setup = Join-Path $OutputDirectory "HeadsetBridge-Setup-$version.exe"
Invoke-Checked 'Setup-Build' { makensis /V2 "-DVERSION=$version" "-DAPP_DIR=$appOut" "-DOUTPUT_FILE=$setup" (Join-Path $root 'installer\HeadsetBridge.nsi') }

# 6. Portable ZIP.
$zip = Join-Path $OutputDirectory "HeadsetBridge-$version-portable.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path (Join-Path $appOut '*') -DestinationPath $zip

Get-ChildItem $setup, $zip | ForEach-Object { '{0}  {1:N1} MB  {2}' -f $_.Name, ($_.Length / 1MB), (Get-FileHash $_.FullName).Hash }
Write-Output "Fertig: $OutputDirectory"
