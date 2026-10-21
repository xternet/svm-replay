function Expand-NativeArchive([string]$Archive, [string]$Work) {
    $names = @(& tar -tzf $Archive)
    if ($LASTEXITCODE -ne 0 -or !$names.Count) { throw 'INSTALL_ERROR: invalid archive listing' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in $names) {
        $name = $entry.TrimEnd('/')
        $parts = $name.Split('/')
        if ($name -cmatch '[^a-zA-Z0-9_.+/-]' -or $parts[0] -cnotmatch '^(bundle|runtime-sources|coordinator-license-additions|LICENSE|NOTICE)$' -or
            @($parts | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -or !$seen.Add($name)) {
            throw "INSTALL_ERROR: unsafe archive path: $entry"
        }
    }
    $types = @(& tar -tvzf $Archive)
    if ($LASTEXITCODE -ne 0) { throw 'INSTALL_ERROR: invalid archive types' }
    if (@($types | Where-Object { $_ -cnotmatch '^[-d]' }).Count) { throw 'INSTALL_ERROR: archive links/special files are forbidden' }
    $payload = Join-Path $Work 'payload'
    [IO.Directory]::CreateDirectory($payload) | Out-Null
    & tar -xzf $Archive -C $payload --no-same-owner --no-same-permissions
    if ($LASTEXITCODE -ne 0) { throw 'INSTALL_ERROR: archive extraction failed' }
}
