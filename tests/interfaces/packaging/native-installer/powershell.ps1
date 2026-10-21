param([string]$Root, [string]$Mode, [string]$Archive, [string]$Work)
$ErrorActionPreference = 'Stop'
try {
    $owner = Join-Path $Root 'src/installer/_1_windows'
    if ($Mode -eq 'parse') {
        foreach ($file in @((Join-Path $Root 'install.ps1')) + @(Get-ChildItem -LiteralPath $owner -Filter '*.ps1' | ForEach-Object { $_.FullName })) {
            $tokens=$null; $errors=$null
            [System.Management.Automation.Language.Parser]::ParseFile($file,[ref]$tokens,[ref]$errors) | Out-Null
            if ($errors.Count) { throw ($errors | Out-String) }
        }
        & (Join-Path $Root 'install.ps1') -Help
        exit 0
    }
    . ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $owner '_0_archive.ps1'))))
    . ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $owner '_1_install.ps1'))))
    if ($Mode -eq 'launcher') {
        $profile = 'C:\Users\' + [char]0x017C + '\svm-replay'
        $text = Get-NativeLauncher "$profile\installations\0.2.0\bin\svm-replay.exe" "$profile\bin"
        if (!$text.Contains('"%~dp0..\installations\0.2.0\bin\svm-replay.exe" %*')) { throw "wrong relative launcher: $text" }
        if ($text -match '[^\x00-\x7F]') { throw 'launcher must not depend on the console code page' }
        $text = Get-NativeLauncher 'C:\app%~dp0\bin\svm-replay.exe' 'C:\commands'
        if (!$text.Contains('%~dp0..\app%%~dp0\bin\svm-replay.exe')) { throw 'literal percent path was expanded' }
        $text = Get-NativeLauncher 'D:\other drive\bin\svm-replay.exe' 'C:\commands'
        if (!$text.Contains('"D:\other drive\bin\svm-replay.exe" %*')) { throw 'wrong cross-drive launcher' }
        exit 0
    }
    Expand-NativeArchive $Archive $Work
    if ($Mode -eq 'missing') {
        Install-NativeBundle (Join-Path $Work 'payload') (Join-Path $Work 'installed') (Join-Path $Work 'bin') (Join-Path $Work 'bin/svm-replay.cmd')
    }
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
