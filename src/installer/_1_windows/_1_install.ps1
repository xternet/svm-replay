function Install-NativeBundle([string]$Payload, [string]$Prefix, [string]$BinDir, [string]$Launcher) {
    $binary = Join-Path $Payload 'bundle/bin/svm-replay.exe'
    $bundle = Join-Path $Payload 'bundle/bundle.json'
    if (!(Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'INSTALL_ERROR: missing native CLI binary' }
    if (!(Test-Path -LiteralPath $bundle -PathType Leaf)) { throw 'INSTALL_ERROR: missing bundle manifest' }
    $pin = (Get-FileHash -LiteralPath $bundle -Algorithm SHA256).Hash.ToLowerInvariant()
    & $binary doctor --bundle $bundle --bundle-sha256 $pin
    if ($LASTEXITCODE -ne 0) { throw 'INSTALL_ERROR: bundle verification failed' }
    [IO.Directory]::CreateDirectory((Split-Path -Parent $Prefix)) | Out-Null
    & $binary bundle install --bundle $bundle --bundle-sha256 $pin --output $Prefix
    if ($LASTEXITCODE -ne 0) { throw "INSTALL_ERROR: installation failed; inspect partial directory: $Prefix" }
    foreach ($item in 'runtime-sources','coordinator-license-additions','LICENSE','NOTICE') {
        $source = Join-Path $Payload $item
        if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $Prefix $item) -Recurse }
    }
    $installed = Join-Path $Prefix 'bin/svm-replay.exe'
    & $installed doctor
    if ($LASTEXITCODE -ne 0) { throw 'INSTALL_ERROR: installed bundle verification failed; installation not activated' }
    [IO.Directory]::CreateDirectory($BinDir) | Out-Null
    $text = Get-NativeLauncher $installed $BinDir
    $stream = [IO.File]::Open($Launcher, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try {
        $bytes = [Text.Encoding]::ASCII.GetBytes($text)
        $stream.Write($bytes, 0, $bytes.Length)
    } finally { $stream.Dispose() }
    Write-Output "Installed SVM Replay 0.2.0: $Prefix"
    Write-Output "Add this directory to PATH if needed: $BinDir"
    Write-Output 'Then run: svm-replay demo'
}

function Get-NativeLauncher([string]$Installed, [string]$BinDir) {
    # Relative paths support Unicode user profiles without changing code pages.
    $base = [uri]($BinDir.TrimEnd('\') + '\')
    $relative = $base.MakeRelativeUri([uri]$Installed)
    $escaped = if ($relative.IsAbsoluteUri -or $relative.ToString() -match '^[a-zA-Z]:') { $Installed.Replace('%','%%') } else {
        '%~dp0' + [uri]::UnescapeDataString($relative.ToString()).Replace('/','\').Replace('%','%%')
    }
    if ($escaped -match '[^\x00-\x7F]') { throw 'INSTALL_ERROR: use ASCII directory names below the common install/bin parent' }
    return "@echo off`r`nsetlocal DisableDelayedExpansion`r`n`"$escaped`" %*`r`nexit /b %errorlevel%`r`n"
}
