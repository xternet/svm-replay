# Thin entry; implementation lives in src/installer/_1_windows/.
[CmdletBinding()]
param([string]$Archive, [string]$Sha256, [string]$Prefix, [string]$BinDir, [switch]$Help)
$ErrorActionPreference = 'Stop'
$options = @{}
foreach ($name in 'Archive','Sha256','Prefix','BinDir','Help') {
    if ($PSBoundParameters.ContainsKey($name)) { $options[$name] = $PSBoundParameters[$name] }
}
$owner = if ($PSScriptRoot) { Join-Path $PSScriptRoot 'src/installer/_1_windows' } else { $null }
$temporary = $null
try {
    if (!$owner) {
        $temporary = Join-Path ([IO.Path]::GetTempPath()) ('svm-replay-bootstrap-' + [guid]::NewGuid())
        [IO.Directory]::CreateDirectory($temporary) | Out-Null
        $owner = $temporary
        $base = 'https://raw.githubusercontent.com/xternet/svm-replay/v0.2.0/src/installer/_1_windows'
        foreach ($file in 'main.ps1','_0_archive.ps1','_1_install.ps1') {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile (Join-Path $owner $file) -TimeoutSec 120
        }
    }
    & ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $owner 'main.ps1')))) -Owner $owner @options
} finally {
    if ($temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
