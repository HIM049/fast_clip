param(
    [Parameter(Mandatory = $true)]
    [string]$VcpkgRoot
)

$ErrorActionPreference = "Stop"

$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$packageName = "FastClip-windows-x86_64-static"
$stageDirectory = Join-Path $projectRoot "dist\$packageName"
$binaryPath = Join-Path $projectRoot "target\release-static\fast_clip.exe"
$licensePath = Join-Path $projectRoot "LICENSE"
$ffmpegShare = Join-Path $VcpkgRoot "installed\x64-windows-fastclip-static-md\share"

function Find-Dumpbin {
    $command = Get-Command "dumpbin.exe" -CommandType Application -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $vswhereCandidates = @(
        (Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)) "Microsoft Visual Studio\Installer\vswhere.exe")
        (Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) "Microsoft Visual Studio\Installer\vswhere.exe")
    ) | Where-Object { Test-Path -LiteralPath $_ }

    foreach ($vswhere in $vswhereCandidates) {
        $matches = @(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe')
        if ($LASTEXITCODE -ne 0) {
            continue
        }

        $match = $matches | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        if ($null -ne $match) {
            return $match
        }
    }

    throw "dumpbin.exe was not found. Install the Visual Studio C++ build tools or run from an x64 Native Tools shell."
}

foreach ($requiredPath in @($binaryPath, $licensePath, $ffmpegShare)) {
    if (-not (Test-Path -LiteralPath $requiredPath)) {
        throw "Required packaging input does not exist: $requiredPath"
    }
}

$dumpbinPath = Find-Dumpbin
$dependencyLines = & $dumpbinPath /DEPENDENTS $binaryPath 2>&1
$dumpbinExitCode = $LASTEXITCODE
if ($dumpbinExitCode -ne 0) {
    throw "dumpbin.exe failed with exit code $dumpbinExitCode.`n$($dependencyLines | Out-String)"
}

$dependencies = $dependencyLines | Out-String
if ($dependencies -match "(?i)(avcodec|avdevice|avfilter|avformat|avutil|postproc|swresample|swscale|dav1d).*\.dll") {
    throw "The static artifact still depends on an FFmpeg DLL."
}

Remove-Item -LiteralPath $stageDirectory -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stageDirectory | Out-Null

Copy-Item -LiteralPath $binaryPath -Destination $stageDirectory
Copy-Item -LiteralPath $licensePath -Destination (Join-Path $stageDirectory "LICENSE-fast_clip.txt")

# Dynamic package alternative retained for future use:
# Get-ChildItem -LiteralPath "$VcpkgRoot\installed\x64-windows\bin" -Filter "*.dll" | Copy-Item -Destination $stageDirectory

$licensesDirectory = Join-Path $stageDirectory "licenses"
New-Item -ItemType Directory -Path $licensesDirectory | Out-Null
$licenseCount = 0
Get-ChildItem -LiteralPath $ffmpegShare -Directory | ForEach-Object {
    $copyright = Join-Path $_.FullName "copyright"
    if (Test-Path -LiteralPath $copyright) {
        Copy-Item -LiteralPath $copyright -Destination (Join-Path $licensesDirectory "$($_.Name).txt")
        $licenseCount++
    }
}

if ($licenseCount -eq 0) {
    throw "No vcpkg dependency licenses were found under $ffmpegShare."
}

foreach ($packagedPath in @(
    (Join-Path $stageDirectory "fast_clip.exe"),
    (Join-Path $stageDirectory "LICENSE-fast_clip.txt")
)) {
    if (-not (Test-Path -LiteralPath $packagedPath -PathType Leaf)) {
        throw "The packaged file was not created: $packagedPath"
    }
}
