param(
    [ValidateSet('Install','Start','Uninstall')][string]$Action = 'Install',
    [ValidateSet('en','ru')][string]$Language,
    [string]$GameDirectory,
    [string]$ProfilePath,
    [string]$DataDirectory = (Join-Path $env:LOCALAPPDATA 'StalkShift'),
    [switch]$NonInteractive
)
$ErrorActionPreference = 'Stop'
$packageRoot = Split-Path -Parent $PSScriptRoot
$utf8 = New-Object System.Text.UTF8Encoding($false, $true)
$recordPath = Join-Path $DataDirectory 'install.json'
$record = $null
if (Test-Path -LiteralPath $recordPath) { $record = Get-Content -LiteralPath $recordPath -Raw -Encoding UTF8 | ConvertFrom-Json }
if (-not $Language) {
    if ($record -and $record.Language) { $Language = $record.Language }
    elseif ($NonInteractive) { $Language = 'en' }
    else {
        $choice = Read-Host 'Language: 1 English / 2 Russian'
        $Language = if ($choice -eq '2') { 'ru' } else { 'en' }
    }
}
$messages = Get-Content -LiteralPath (Join-Path $PSScriptRoot "$Language.json") -Raw -Encoding UTF8 | ConvertFrom-Json
function Say([string]$key) { Write-Host $messages.$key }
function Hash([string]$path) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
function Write-Preserved([string]$path, [byte[]]$bytes) {
    $attributes = [System.IO.File]::GetAttributes($path)
    try {
        [System.IO.File]::SetAttributes($path, ($attributes -band (-bnot [System.IO.FileAttributes]::ReadOnly)))
        [System.IO.File]::WriteAllBytes($path, $bytes)
    } finally { [System.IO.File]::SetAttributes($path, $attributes) }
}
function Valid-Game([string]$path) {
    $path -and (Test-Path -LiteralPath (Join-Path $path 'bin/win_x64/eurotrucks2.exe') -PathType Leaf)
}
function Find-Games {
    $steamRoots = @()
    $steam = Get-ItemProperty -LiteralPath 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue
    if ($steam -and $steam.SteamPath) { $steamRoots += $steam.SteamPath }
    if (${env:ProgramFiles(x86)}) { $steamRoots += Join-Path ${env:ProgramFiles(x86)} 'Steam' }
    $libraries = @($steamRoots)
    foreach ($root in $steamRoots) {
        $vdf = Join-Path $root 'steamapps/libraryfolders.vdf'
        if (Test-Path -LiteralPath $vdf) {
            $text = Get-Content -LiteralPath $vdf -Raw
            foreach ($match in [regex]::Matches($text, '"path"\s+"([^"\r\n]+)"')) {
                $libraries += $match.Groups[1].Value.Replace('\\','\')
            }
        }
    }
    foreach ($library in ($libraries | Select-Object -Unique)) {
        $candidate = Join-Path $library 'steamapps/common/Euro Truck Simulator 2'
        if (Valid-Game $candidate) { (Resolve-Path -LiteralPath $candidate).Path }
    }
}
function Select-Game {
    if ($GameDirectory) { return $GameDirectory }
    if ($record -and (Valid-Game $record.GameDirectory)) { return $record.GameDirectory }
    $games = @(Find-Games | Select-Object -Unique)
    if ($games.Count -eq 1) { Write-Host "$($messages.gameFound): $($games[0])"; return $games[0] }
    if ($NonInteractive) { throw $messages.badGame }
    if ($games.Count -gt 1) {
        for ($i=0; $i -lt $games.Count; $i++) { Write-Host "$($i+1). $($games[$i])" }
        $index = 0
        if (-not [int]::TryParse((Read-Host $messages.chooseGame), [ref]$index) -or $index -lt 1 -or $index -gt $games.Count) { throw $messages.badChoice }
        return $games[$index-1]
    }
    (Read-Host $messages.gamePath).Trim('"')
}
function Select-Profile {
    if ($ProfilePath) { return (Resolve-Path -LiteralPath $ProfilePath).Path }
    if ($NonInteractive) { return $null }
    $docs = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Euro Truck Simulator 2'
    $profiles = @()
    foreach ($kind in @('profiles','steam_profiles')) {
        $parent = Join-Path $docs $kind
        if (Test-Path -LiteralPath $parent) {
            $profiles += @(Get-ChildItem -LiteralPath $parent -Directory | ForEach-Object {
                $controls = Join-Path $_.FullName 'controls.sii'
                if (Test-Path -LiteralPath $controls) { Get-Item -LiteralPath $controls }
            })
        }
    }
    $profiles = @($profiles | Sort-Object LastWriteTime -Descending)
    if ($profiles.Count -eq 0) { return $null }
    Say profiles
    for ($i=0; $i -lt $profiles.Count; $i++) {
        $name = Split-Path -Leaf (Split-Path -Parent $profiles[$i].FullName)
        try {
            if ($name -match '^(?:[0-9A-Fa-f]{2})+$') {
                $bytes = [byte[]]@(for ($j=0; $j -lt $name.Length; $j+=2) { [Convert]::ToByte($name.Substring($j,2),16) })
                $name = $utf8.GetString($bytes)
            }
        } catch { }
        Write-Host "$($i+1). $name"
    }
    $index = 0
    $answer = Read-Host $messages.chooseProfile
    if (-not $answer -or $answer -eq '0') { return $null }
    if (-not [int]::TryParse($answer,[ref]$index) -or $index -lt 1 -or $index -gt $profiles.Count) { throw $messages.badChoice }
    $profiles[$index-1].FullName
}
function Check-Package {
    $manifest = Get-Content -LiteralPath (Join-Path $packageRoot 'version.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    foreach ($name in @('stalkshift.exe','stalkshift_plugin.dll')) {
        $file = Join-Path $packageRoot $name
        if (-not (Test-Path -LiteralPath $file) -or (Hash $file) -ne $manifest.files.$name) { throw $messages.badPackage }
    }
    $dll = [System.IO.File]::ReadAllBytes((Join-Path $packageRoot 'stalkshift_plugin.dll'))
    if ($dll.Length -lt 64) { throw $messages.badPackage }
    $pe = [BitConverter]::ToInt32($dll,60)
    if ($pe -lt 0 -or $pe -gt $dll.Length-24 -or [BitConverter]::ToUInt32($dll,$pe) -ne 17744 -or [BitConverter]::ToUInt16($dll,$pe+4) -ne 34404) { throw $messages.badPackage }
    $manifest
}
try {
    Say title
    if ($Action -ne 'Start' -and (Get-Process eurotrucks2 -ErrorAction SilentlyContinue)) { throw $messages.closeGame }
    if (Get-Process stalkshift -ErrorAction SilentlyContinue) { throw $messages.closeBridge }
    if ($Action -eq 'Uninstall') {
        if (-not $record -or -not $record.Installed) { throw $messages.installFirst }
        $game = $record.GameDirectory
    } else { $game = Select-Game }
    if (-not (Valid-Game $game)) { throw $messages.badGame }
    $game = (Resolve-Path -LiteralPath $game).Path
    $destination = Join-Path $game 'bin/win_x64/plugins/stalkshift_plugin.dll'
    if ($Action -eq 'Start') {
        $manifest = Check-Package
        if (-not (Test-Path -LiteralPath $destination) -or (Hash $destination) -ne $manifest.files.'stalkshift_plugin.dll') { throw $messages.installFirst }
        $exe = Join-Path $packageRoot 'stalkshift.exe'
        $lines = @(& $exe list)
        $devices = @($lines | Where-Object { $_ -match '^\[\d+\].*346e:0024.*usage=0001:0004.*interface=2' })
        if ($LASTEXITCODE -ne 0 -or $devices.Count -eq 0) { throw $messages.noDevice }
        $index = [int]([regex]::Match($devices[0], '^\[(\d+)\]').Groups[1].Value)
        if ($devices.Count -gt 1) {
            $devices | ForEach-Object { Write-Host $_ }
            if ($NonInteractive) { throw $messages.badChoice }
            $choice = Read-Host $messages.device
            if (-not [int]::TryParse($choice,[ref]$index) -or -not ($devices | Where-Object { $_ -match "^\[$index\]" })) { throw $messages.badChoice }
        }
        $logs = Join-Path $DataDirectory 'logs'
        New-Item -ItemType Directory -Path $logs -Force | Out-Null
        $log = Join-Path $logs ('session-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
        Say running
        Write-Host "$($messages.log): $log"
        $ErrorActionPreference = 'Continue'
        & $exe bridge --device $index 2>&1 | Tee-Object -FilePath $log
        exit $LASTEXITCODE
    }
    if ($Action -eq 'Uninstall') {
        if (Test-Path -LiteralPath $destination) {
            if ((Hash $destination) -ne $record.PluginHash) { throw $messages.pluginChanged }
            Remove-Item -LiteralPath $destination
        }
        foreach ($profile in @($record.Profiles)) {
            if (-not $profile) { continue }
            if ((Test-Path -LiteralPath $profile.Path) -and (Hash $profile.Path) -eq $profile.ModifiedHash -and (Test-Path -LiteralPath $profile.Backup) -and (Hash $profile.Backup) -eq $profile.OriginalHash) {
                Write-Preserved $profile.Path ([System.IO.File]::ReadAllBytes($profile.Backup))
                Say restored
            } elseif ((Test-Path -LiteralPath $profile.Path) -and (Hash $profile.Path) -eq $profile.OriginalHash) { }
            else { Say profileChanged; Write-Host $profile.Backup }
        }
        $record.Installed = $false
        [System.IO.File]::WriteAllText($recordPath,($record | ConvertTo-Json -Depth 6),$utf8)
        Say removed
        exit 0
    }
    $manifest = Check-Package
    $profilePathSelected = Select-Profile
    $backup = Join-Path $DataDirectory ('backups/' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $backup -Force | Out-Null
    $oldPlugin = $null
    if (Test-Path -LiteralPath $destination) {
        $oldPlugin = Join-Path $backup 'stalkshift_plugin.dll'
        Copy-Item -LiteralPath $destination -Destination $oldPlugin
    }
    $profilesToRestore = @()
    # Preserve earlier restoration records when reinstalling/upgrading the same game.
    if ($record -and $record.Installed -and $record.GameDirectory -eq $game) { $profilesToRestore = @($record.Profiles) }
    $editedProfile = $null
    try {
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $packageRoot 'stalkshift_plugin.dll') -Destination $destination -Force
        if ((Hash $destination) -ne $manifest.files.'stalkshift_plugin.dll') { throw $messages.badPackage }
        if ($profilePathSelected) {
            if ((Split-Path -Leaf $profilePathSelected) -ne 'controls.sii') { throw $messages.badChoice }
            $original = [System.IO.File]::ReadAllBytes($profilePathSelected)
            $text = $utf8.GetString($original)
            $updated = $text
            $count = 0
            foreach ($match in [regex]::Matches($text, 'device (\w+) `[^\r\n]*\{0024346E-0000-0000-0000-504944564944\}')) {
                $pattern = '\b' + [regex]::Escape($match.Groups[1].Value) + '\.b\d+\?0'
                $count += [regex]::Matches($updated,$pattern).Count
                $updated = [regex]::Replace($updated,$pattern,'unbound?0')
            }
            if ($count -gt 0) {
                $profileBackup = Join-Path $backup 'controls.sii'
                [System.IO.File]::WriteAllBytes($profileBackup,$original)
                $editedProfile = @{ Path=$profilePathSelected; Backup=$profileBackup; OriginalHash=(Hash $profileBackup) }
                Write-Preserved $profilePathSelected ($utf8.GetBytes($updated))
                $editedProfile.ModifiedHash = Hash $profilePathSelected
                if ([System.IO.File]::ReadAllText($profilePathSelected,$utf8) -cne $updated) { throw $messages.badPackage }
                $profilesToRestore = @($profilesToRestore | Where-Object { $_ -and $_.Path -ne $profilePathSelected }) + @($editedProfile)
                Write-Host "$($messages.changedProfile): $count"
            } else { Say noBindings }
        }
        $newRecord = @{ Installed=$true; Language=$Language; Version=$manifest.version; GameDirectory=$game; PluginHash=(Hash $destination); Backup=$backup; Profiles=@($profilesToRestore) }
        [System.IO.File]::WriteAllText($recordPath,($newRecord | ConvertTo-Json -Depth 6),$utf8)
    } catch {
        if ($editedProfile) { Write-Preserved $editedProfile.Path ([System.IO.File]::ReadAllBytes($editedProfile.Backup)) }
        if ($oldPlugin) { Copy-Item -LiteralPath $oldPlugin -Destination $destination -Force }
        elseif (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination }
        throw
    }
    Say installed
    Write-Host "$($messages.backup): $backup"
} catch {
    Write-Host "$($messages.failed): $($_.Exception.Message)" -ForegroundColor Red
    Say access
    exit 1
}
