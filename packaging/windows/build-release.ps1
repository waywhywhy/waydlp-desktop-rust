<#
.SYNOPSIS
  Builds every Windows release artifact for waydlp.

.DESCRIPTION
  Produces, in dist\:
    waydlp-<ver>-windows-<arch>-portable.zip   portable build (data kept next to the exe)
    waydlp-<ver>-windows-<arch>-setup.exe      graphical installer with the portable build embedded
    waydlp-<ver>-windows-<arch>.msi            per-machine MSI (requires the WiX Toolset v5: `dotnet tool install --global wix`)
  and appends their SHA-256 sums to dist\SHA256SUMS.txt.

.EXAMPLE
  pwsh packaging/windows/build-release.ps1 -Version 1.0.0 -Arch x64
#>
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [ValidateSet('x64', 'arm64')][string]$Arch = 'x64',
  [switch]$SkipMsi
)
$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Set-Location $Root
$Triple = if ($Arch -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'aarch64-pc-windows-msvc' }
$Bin = Join-Path $Root "target\$Triple\release"
$Dist = Join-Path $Root 'dist'
New-Item -ItemType Directory -Force $Dist | Out-Null

Write-Host "==> Building release binaries for $Triple"
cargo build --release --locked --target $Triple -p waydlp-gui -p waydlp-cli -p waydlp-installer-cli -p waydlp-installer-gui
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

# Portable zip ------------------------------------------------------------
$Name = "waydlp-$Version-windows-$Arch"
$Stage = Join-Path $Dist "stage-$Name"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force $Stage | Out-Null
Copy-Item "$Bin\waydlp.exe", "$Bin\waydlp-cli.exe", "$Bin\waydlp-installer.exe" $Stage
Copy-Item LICENSE (Join-Path $Stage 'LICENSE.txt')
Copy-Item packaging\portable\README-portable.txt (Join-Path $Stage 'README.txt')
New-Item -ItemType File (Join-Path $Stage 'waydlp.portable') | Out-Null
$Zip = Join-Path $Dist "$Name-portable.zip"
if (Test-Path $Zip) { Remove-Item $Zip }
Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Zip
Write-Host "==> $Zip"

# Offline setup.exe = GUI installer + embedded portable zip ---------------
$Setup = Join-Path $Dist "$Name-setup.exe"
& "$Bin\waydlp-installer.exe" bundle --stub "$Bin\waydlp-setup.exe" --payload $Zip --version $Version --out $Setup
if ($LASTEXITCODE -ne 0) { throw 'bundling setup.exe failed' }
Write-Host "==> $Setup"

# MSI ---------------------------------------------------------------------
if (-not $SkipMsi) {
  $Msi = Join-Path $Dist "$Name.msi"
  wix build packaging\windows\waydlp.wxs -arch $Arch -d "Version=$Version" -d "BinDir=$Bin" -d "AssetsDir=$Root\assets\icons" -d "RootDir=$Root" -o $Msi
  if ($LASTEXITCODE -ne 0) { throw 'wix build failed' }
  Write-Host "==> $Msi"
}

Remove-Item -Recurse -Force $Stage
Get-ChildItem $Dist -File | Where-Object { $_.Name -like "$Name*" } | ForEach-Object {
  $h = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower()
  "$h  $($_.Name)" | Out-File -Append -Encoding ascii (Join-Path $Dist 'SHA256SUMS.txt')
}
Write-Host '==> Done'
