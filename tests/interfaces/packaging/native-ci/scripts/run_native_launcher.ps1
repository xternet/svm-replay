param(
    [Parameter(Mandatory=$true)][string]$Launcher,
    [Parameter(ValueFromRemainingArguments=$true)][string[]]$CommandArgs
)
$ErrorActionPreference = 'Stop'
& $Launcher @CommandArgs
if ($null -eq $LASTEXITCODE) { throw 'Native launcher did not report an exit code' }
exit $LASTEXITCODE
