<#
.SYNOPSIS
Installs a checksum-verified waydlp release for the current Windows user.
.EXAMPLE
& .\install.ps1 -Version 0.1.0 -InstallDir 'C:\Tools\waydlp'
#>
[CmdletBinding()]
param(
    [string] $Version = 'latest',
    [string] $InstallDir,
    [switch] $NoMenu,
    [switch] $DesktopShortcut,
    [switch] $Help
)
$ErrorActionPreference = 'Stop'
# Windows PowerShell 5.1's per-chunk progress rendering slows large downloads.
$ProgressPreference = 'SilentlyContinue'
if ($Help) {
    Write-Output 'Usage: .\install.ps1 [-Version VERSION] [-InstallDir DIRECTORY] [-NoMenu] [-DesktopShortcut]'
    Write-Output 'Installs the latest stable waydlp for the current user, without administrator access.'
    return
}
if ($env:OS -ne 'Windows_NT') { throw 'This installer requires Windows.' }
# PROCESSOR_ARCHITEW6432 identifies the native OS when using a 32-bit shell.
$nativeArch = $env:PROCESSOR_ARCHITEW6432
if (-not $nativeArch) { $nativeArch = $env:PROCESSOR_ARCHITECTURE }
# ARM64 Windows can also emulate an AMD64 shell.
$systemArch = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment').PROCESSOR_ARCHITECTURE
if ($systemArch) { $nativeArch = $systemArch }
$arch = switch ($nativeArch) {
    'AMD64' { 'x64' }
    'ARM64' { 'arm64' }
    default { throw 'Supported architectures: x64 and ARM64.' }
}
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$repo = 'https://api.github.com/repos/waywhywhy/waydlp-desktop-rust'
$Version = $Version -creplace '^v', ''
if ($Version -eq 'latest') {
    $release = Invoke-RestMethod -Uri "$repo/releases/latest" -Headers @{ 'User-Agent' = 'waydlp-bootstrap' }
    $Version = $release.tag_name -creplace '^v', ''
}
if ($Version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$') { throw 'Invalid release version.' }
$tempDir = Join-Path ([IO.Path]::GetTempPath()) ('waydlp-bootstrap-' + [Guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $tempDir
try {
    $asset = "waydlp-$Version-windows-$arch-portable.zip"
    $base = "https://github.com/waywhywhy/waydlp-desktop-rust/releases/download/v$Version"
    $manifest = Join-Path $tempDir 'SHA256SUMS.txt'
    $archive = Join-Path $tempDir $asset
    Write-Host "Downloading waydlp $Version ($arch)..."
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS.txt" -OutFile $manifest
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $archive
    $pattern = '^([0-9a-fA-F]{64})\s+' + [regex]::Escape($asset) + '$'
    $checksums = @(Get-Content -LiteralPath $manifest | ForEach-Object {
        if ($_ -cmatch $pattern) { $Matches[1] }
    })
    if ($checksums.Count -ne 1) { throw 'Missing or invalid installer checksum.' }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $checksums[0]) {
        throw 'Installer checksum mismatch; refusing to execute.'
    }
    # Use the matching installer in the archive, including on older releases.
    $payloadDir = Join-Path $tempDir 'payload'
    Expand-Archive -LiteralPath $archive -DestinationPath $payloadDir
    $installer = Join-Path $payloadDir 'waydlp-installer.exe'
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { throw 'Archive is missing its installer.' }
    $installArgs = @('install', '--format', 'portable', '--yes', '--file', $archive)
    if ($InstallDir) { $installArgs += @('--dir', $InstallDir) }
    if ($NoMenu) { $installArgs += '--no-menu' }
    if ($DesktopShortcut) { $installArgs += '--desktop-shortcut' }
    & $installer @installArgs
    if ($LASTEXITCODE -ne 0) { throw "Native installer failed (exit $LASTEXITCODE)." }
    if (-not $InstallDir) { $InstallDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs\waydlp' }
    Write-Host "Installed. Launch: $(Join-Path $InstallDir 'waydlp.exe')"
    Write-Host "CLI: & '$(Join-Path $InstallDir 'waydlp-cli.exe')' --help"
    Write-Host "Verify: & '$(Join-Path $InstallDir 'waydlp-installer.exe')' verify"
    Write-Host "Uninstall: & '$(Join-Path $InstallDir 'waydlp-installer.exe')' uninstall"
}
finally {
    # Only remove the unique temporary directory created by this invocation.
    if ((Split-Path -Parent $tempDir) -eq [IO.Path]::GetTempPath().TrimEnd('\', '/')) {
        Remove-Item -LiteralPath $tempDir -Recurse -Force
    }
}
