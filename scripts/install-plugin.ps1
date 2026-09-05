param(
    [Parameter(Mandatory = $true)][string]$GameDirectory,
    [string]$PluginPath = (Join-Path $PSScriptRoot '../target/release/stalkshift_plugin.dll')
)
$ErrorActionPreference = 'Stop'
if (Get-Process -Name eurotrucks2 -ErrorAction SilentlyContinue) {
    throw 'Close ETS2 completely before installing the plugin.'
}
$gameRoot = (Resolve-Path -LiteralPath $GameDirectory).Path
$binaryDirectory = Join-Path $gameRoot 'bin/win_x64'
if (-not (Test-Path -LiteralPath (Join-Path $binaryDirectory 'eurotrucks2.exe') -PathType Leaf)) {
    throw 'GameDirectory must contain bin/win_x64/eurotrucks2.exe.'
}
$pluginSource = (Resolve-Path -LiteralPath $PluginPath).Path
$pluginBytes = [System.IO.File]::ReadAllBytes($pluginSource)
if ($pluginBytes.Length -lt 64 -or $pluginBytes[0] -ne 77 -or $pluginBytes[1] -ne 90) { throw 'Plugin is not a PE binary.' }
$peOffset = [BitConverter]::ToInt32($pluginBytes, 60)
if ($peOffset -lt 0 -or $peOffset -gt $pluginBytes.Length - 24) { throw 'Invalid PE header.' }
if ([BitConverter]::ToUInt32($pluginBytes, $peOffset) -ne 17744 -or [BitConverter]::ToUInt16($pluginBytes, $peOffset + 4) -ne 34404) {
    throw 'Plugin must be a Windows x64 PE binary.'
}
$pluginDirectory = Join-Path $binaryDirectory 'plugins'
$destination = Join-Path $pluginDirectory 'stalkshift_plugin.dll'
New-Item -ItemType Directory -Path $pluginDirectory -Force | Out-Null
if (Test-Path -LiteralPath $destination) {
    $backupDirectory = Join-Path $PSScriptRoot '../backups'
    New-Item -ItemType Directory -Path $backupDirectory -Force | Out-Null
    $backupName = 'stalkshift_plugin-' + [guid]::NewGuid().ToString('N') + '.dll'
    Copy-Item -LiteralPath $destination -Destination (Join-Path $backupDirectory $backupName)
}
Copy-Item -LiteralPath $pluginSource -Destination $destination -Force
$sourceHash = (Get-FileHash -LiteralPath $pluginSource -Algorithm SHA256).Hash
$installedHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
if ($sourceHash -ne $installedHash) { throw 'Installed plugin checksum differs from the build.' }
Write-Output "Installed: $destination"
Write-Output "SHA256: $installedHash"
