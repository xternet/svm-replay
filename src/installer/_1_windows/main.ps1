[CmdletBinding()]
param([Parameter(Mandatory)][string]$Owner, [string]$Archive, [string]$Sha256,
    [string]$Prefix, [string]$BinDir, [switch]$Help)
$ErrorActionPreference = 'Stop'
foreach ($file in '_0_archive.ps1','_1_install.ps1') {
    . ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $Owner $file))))
}
if ($Help) {
    Write-Output 'SVM Replay 0.2.0: -Prefix NEW_DIRECTORY -BinDir DIRECTORY; offline: -Archive FILE -Sha256 TRUSTED_SHA256'
    return
}
if ($env:OS -ne 'Windows_NT') { throw 'INSTALL_ERROR: use install.sh on Unix' }
if (!$Prefix) { $Prefix = Join-Path $env:LOCALAPPDATA 'svm-replay\installations\0.2.0' }
if (!$BinDir) { $BinDir = Join-Path $env:LOCALAPPDATA 'svm-replay\bin' }
$Prefix = [IO.Path]::GetFullPath($Prefix)
$BinDir = [IO.Path]::GetFullPath($BinDir)
if ($Prefix -match '[\r\n"]' -or $BinDir -match '[\r\n"]') { throw 'INSTALL_ERROR: invalid destination' }
if (Test-Path -LiteralPath $Prefix) { throw "INSTALL_ERROR: installation already exists: $Prefix" }
$launcher = Join-Path $BinDir 'svm-replay.cmd'
if (Test-Path -LiteralPath $launcher) { throw "INSTALL_ERROR: launcher already exists: $launcher" }
if ($Archive -or $Sha256) {
    if (!$Archive -or $Sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'INSTALL_ERROR: -Archive and a 64-digit -Sha256 are required together' }
    if (!(Test-Path -LiteralPath $Archive -PathType Leaf)) { throw 'INSTALL_ERROR: archive not found' }
}
$work = Join-Path ([IO.Path]::GetTempPath()) ('svm-replay-install-' + [guid]::NewGuid())
[IO.Directory]::CreateDirectory($work) | Out-Null
try {
    $privateArchive = Join-Path $work 'archive.tar.gz'
    if ($Archive) { Copy-Item -LiteralPath $Archive -Destination $privateArchive } else {
        $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
        $cpu = switch ($arch) { 'AMD64' { 'x86_64' } 'ARM64' { 'aarch64' } default { throw 'INSTALL_ERROR: unsupported CPU' } }
        $file = "svm-replay-0.2.0-windows-$cpu.tar.gz"
        $base = 'https://github.com/xternet/svm-replay/releases/download/v0.2.0'
        $sums = Join-Path $work 'SHA256SUMS'
        Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums -TimeoutSec 120
        $matches = @(Get-Content -LiteralPath $sums | Where-Object { $_ -cmatch ('^[0-9a-f]{64}  ' + [regex]::Escape($file) + '$') })
        if ($matches.Count -ne 1) { throw 'INSTALL_ERROR: missing or ambiguous checksum' }
        $Sha256 = $matches[0].Substring(0,64)
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile $privateArchive -TimeoutSec 1800
    }
    if ((Get-FileHash -LiteralPath $privateArchive -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Sha256) { throw 'INSTALL_ERROR: archive integrity mismatch' }
    Expand-NativeArchive $privateArchive $work
    Install-NativeBundle (Join-Path $work 'payload') $Prefix $BinDir $launcher
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force
}
