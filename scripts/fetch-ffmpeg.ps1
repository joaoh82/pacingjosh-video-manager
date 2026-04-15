# Fetch FFmpeg / FFprobe sidecar binaries for Windows Tauri bundle.
#
# Usage (from repo root):
#   .\scripts\fetch-ffmpeg.ps1
#
# Downloads gyan.dev "essentials" build, extracts ffmpeg.exe and ffprobe.exe,
# and renames them to the target-triple-suffixed names Tauri expects.

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$binDir   = Join-Path $repoRoot "src-tauri\binaries"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null

$target = & rustc -vV | Select-String "^host: " | ForEach-Object { $_.ToString().Substring(6).Trim() }
if (-not $target) { $target = "x86_64-pc-windows-msvc" }
Write-Host "Target: $target"

# gyan.dev essentials build (smaller than "full")
$url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
$tmp = New-Item -ItemType Directory -Force -Path (Join-Path $env:TEMP "ffmpeg-fetch")
$zip = Join-Path $tmp "ffmpeg.zip"

Write-Host "Downloading $url..."
Invoke-WebRequest -Uri $url -OutFile $zip

Write-Host "Extracting..."
Expand-Archive -Path $zip -DestinationPath $tmp -Force

$ffmpeg  = Get-ChildItem -Path $tmp -Recurse -Filter "ffmpeg.exe"  | Select-Object -First 1
$ffprobe = Get-ChildItem -Path $tmp -Recurse -Filter "ffprobe.exe" | Select-Object -First 1

if (-not $ffmpeg -or -not $ffprobe) {
    Write-Error "Could not locate ffmpeg.exe / ffprobe.exe inside the archive"
    exit 1
}

Copy-Item $ffmpeg.FullName  (Join-Path $binDir "ffmpeg-$target.exe")  -Force
Copy-Item $ffprobe.FullName (Join-Path $binDir "ffprobe-$target.exe") -Force

Remove-Item -Recurse -Force $tmp
Write-Host "Installed:"
Get-ChildItem -Path $binDir -Filter "ff*-$target.exe"
