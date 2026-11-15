# Installs the built VST3 bundle into the machine-wide VST3 folder Bitwig scans.
# Needs administrator rights (it is started elevated by the caller).
$ErrorActionPreference = 'Stop'
$src = Join-Path $PSScriptRoot 'target\bundled\ELABS RLX.vst3'
$dst = 'C:\Program Files\Common Files\VST3'

if (-not (Test-Path $src)) { throw "not built yet: $src" }
New-Item -ItemType Directory -Force -Path $dst | Out-Null
if (Test-Path (Join-Path $dst 'ELABS RLX.vst3')) { Remove-Item (Join-Path $dst 'ELABS RLX.vst3') -Recurse -Force }
Copy-Item $src $dst -Recurse -Force
Write-Output ("installed: " + (Get-Item (Join-Path $dst 'ELABS RLX.vst3\Contents\x86_64-win\ELABS RLX.vst3')).FullName)
