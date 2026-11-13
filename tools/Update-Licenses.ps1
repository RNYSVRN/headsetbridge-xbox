<#
.SYNOPSIS
  Erzeugt src/HeadsetBridge.App/Lizenzhinweise.txt aus den tatsächlich verwendeten Komponenten.

.DESCRIPTION
  - HeadsetBridge selbst (LICENSE)
  - Rust-Crates, die in die Engine gelinkt werden (cargo tree für x86_64-pc-windows-gnu),
    mit den Lizenzdateien aus dem Cargo-Registry-Cache
  - libopus (COPYING aus dem Quellarchiv, wenn -OpusSource angegeben)
  - NuGet-Pakete der WinUI-App (obj/project.assets.json nach einem Restore)
  - .NET-Laufzeit (LICENSE.txt und ThirdPartyNotices.txt der installierten .NET-SDK)

  Abschnitte beginnen mit "===== Name =====", so wie LicenseParser.cs sie erwartet.
  Voraussetzung: cargo (GNU-Toolchain) und ein ausgeführtes "dotnet restore" der App.
#>
param(
    [string]$OpusSource,
    [string]$Output = (Join-Path $PSScriptRoot '..\src\HeadsetBridge.App\Lizenzhinweise.txt')
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$sb = [System.Text.StringBuilder]::new()
$licensePattern = '^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT|UNLICENSE)([-._].*)?$'

function Add-Section([string]$name, [string]$text) {
    [void]$sb.AppendLine("===== $name =====")
    [void]$sb.AppendLine($text.Trim())
    [void]$sb.AppendLine()
}

# Kopf: HeadsetBridge selbst (vor dem ersten Abschnitt = "Projekt und allgemeine Hinweise").
[void]$sb.AppendLine("HeadsetBridge – Lizenzhinweise / License notices")
[void]$sb.AppendLine()
[void]$sb.AppendLine((Get-Content -Raw (Join-Path $root 'LICENSE')).Trim())
[void]$sb.AppendLine()
[void]$sb.AppendLine("Engine: derived from HeadsetBridge for Xbox (https://github.com/MGPersonalProfile/headsetbridge-for-xbox), MIT License.")
[void]$sb.AppendLine("Not an official Microsoft application. Xbox is a trademark of Microsoft.")
[void]$sb.AppendLine()

# Rust-Crates der Engine.
$manifest = Join-Path $root 'engine\Cargo.toml'
$env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
$metadata = cargo metadata --format-version 1 --manifest-path $manifest --locked | ConvertFrom-Json
$dirs = @{}
foreach ($p in $metadata.packages) { $dirs["$($p.name) v$($p.version)"] = @{ Dir = Split-Path $p.manifest_path; License = $p.license } }
$used = cargo tree --manifest-path $manifest --locked --target x86_64-pc-windows-gnu -e normal --prefix none --format '{p}' |
    ForEach-Object { ($_ -replace '\s*\(.*\)\s*$', '').Trim() } | Where-Object { $_ -and $_ -notlike 'headsetbridge-engine*' } | Sort-Object -Unique
foreach ($crate in $used) {
    $info = $dirs[$crate]
    if (-not $info) { continue }
    $files = Get-ChildItem -LiteralPath $info.Dir -File | Where-Object { $_.Name -match $licensePattern } | Sort-Object Name
    $id = $crate -replace ' v', '-'
    if ($files) {
        foreach ($f in $files) { Add-Section "$id / $($f.Name)" (Get-Content -Raw -LiteralPath $f.FullName) }
    } else {
        Add-Section "$id / Cargo.toml" "License: $($info.License) (the package contains no separate license file)"
    }
}

# libopus.
if ($OpusSource -and (Test-Path (Join-Path $OpusSource 'COPYING'))) {
    Add-Section 'libopus-1.5.2 / COPYING' (Get-Content -Raw (Join-Path $OpusSource 'COPYING'))
}

# NuGet-Pakete der App.
$assets = Join-Path $root 'src\HeadsetBridge.App\obj\project.assets.json'
if (Test-Path $assets) {
    $json = Get-Content -Raw $assets | ConvertFrom-Json
    $nugetRoot = $json.project.restore.packagesPath
    foreach ($lib in $json.libraries.PSObject.Properties) {
        if ($lib.Value.type -ne 'package') { continue }
        $dir = Join-Path $nugetRoot $lib.Value.path
        $files = Get-ChildItem -LiteralPath $dir -File -ErrorAction SilentlyContinue | Where-Object { $_.BaseName -match '^(LICEN[CS]E|NOTICE|ThirdPartyNotices)' -and $_.Extension -in '.txt', '.md', '' }
        if ($files) {
            foreach ($f in $files) { Add-Section "NuGet $($lib.Name) / $($f.Name)" (Get-Content -Raw -LiteralPath $f.FullName) }
        } else {
            $nuspec = Get-ChildItem -LiteralPath $dir -Filter *.nuspec -ErrorAction SilentlyContinue | Select-Object -First 1
            $expr = if ($nuspec) { ([xml](Get-Content -Raw $nuspec.FullName)).package.metadata.license.'#text' } else { $null }
            if ($expr) { Add-Section "NuGet $($lib.Name) / license" "License: $expr" }
        }
    }
}

# .NET-Laufzeit (die App wird self-contained veröffentlicht).
$dotnet = Split-Path (Get-Command dotnet).Source
foreach ($name in 'LICENSE.txt', 'ThirdPartyNotices.txt') {
    $f = Join-Path $dotnet $name
    if (Test-Path $f) { Add-Section ".NET Runtime / $name" (Get-Content -Raw $f) }
}

[System.IO.File]::WriteAllText((Resolve-Path -LiteralPath (Split-Path $Output)).Path + '\' + (Split-Path $Output -Leaf), $sb.ToString(), [System.Text.UTF8Encoding]::new($false))
Write-Output "Lizenzhinweise: $Output ($($used.Count) Crates)"
