$ErrorActionPreference = 'Stop'
# Stub downloads in this scope; the bootstrap must reject them before execution.
function Invoke-WebRequest {
    param([switch] $UseBasicParsing, $Uri, $OutFile)
    if ($Uri.EndsWith('/SHA256SUMS.txt')) {
        $asset = ($script:arch -eq 'ARM64') ? 'arm64' : 'x64'
        $line = ('0' * 64) + "  waydlp-0.1.0-windows-$asset-portable.zip"
        switch ($script:checksumCase) {
            'missing' { $line = ('0' * 64) + '  wrong-file' }
            'duplicate' { $line = "$line`n$line" }
        }
        Set-Content -LiteralPath $OutFile -Value $line
    } else {
        Set-Content -LiteralPath $OutFile -Value 'This is not an executable.'
    }
}
$script:arch = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment').PROCESSOR_ARCHITECTURE
foreach ($script:checksumCase in 'corrupt', 'missing', 'duplicate') {
    $caught = $false
    try { & "$PSScriptRoot/../../install.ps1" -Version 0.1.0 }
    catch {
        if ($_.Exception.Message -notmatch 'checksum') { throw }
        $caught = $true
    }
    if (-not $caught) { throw "Installer accepted $script:checksumCase checksum" }
}
Write-Host 'Checksum rejection tests passed.'
