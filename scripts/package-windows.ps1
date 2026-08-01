param(
    [Parameter(Mandatory = $true)]
    [string]$VcpkgRoot
)

$ErrorActionPreference = "Stop"

$packageName = "FastClip-windows-x86_64-dynamic"
$stageDirectory = Join-Path $PSScriptRoot "..\dist\$packageName"
$archivePath = Join-Path $PSScriptRoot "..\dist\$packageName.zip"
$ffmpegBin = Join-Path $VcpkgRoot "installed\x64-windows\bin"
$ffmpegShare = Join-Path $VcpkgRoot "installed\x64-windows\share"

Remove-Item -LiteralPath $stageDirectory -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stageDirectory | Out-Null

Copy-Item -LiteralPath "target\release\fast_clip.exe" -Destination $stageDirectory
Get-ChildItem -LiteralPath $ffmpegBin -Filter "*.dll" | Copy-Item -Destination $stageDirectory
Copy-Item -LiteralPath "LICENSE" -Destination (Join-Path $stageDirectory "LICENSE-fast_clip.txt")

$licensesDirectory = Join-Path $stageDirectory "licenses"
New-Item -ItemType Directory -Path $licensesDirectory | Out-Null
Get-ChildItem -LiteralPath $ffmpegShare -Directory | ForEach-Object {
    $copyright = Join-Path $_.FullName "copyright"
    if (Test-Path -LiteralPath $copyright) {
        Copy-Item -LiteralPath $copyright -Destination (Join-Path $licensesDirectory "$($_.Name).txt")
    }
}

Compress-Archive -Path $stageDirectory -DestinationPath $archivePath -CompressionLevel Optimal
