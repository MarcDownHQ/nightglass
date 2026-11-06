# Downloads libmpv-2.dll (zhongfly/mpv-winbuild, LGPL x86_64 baseline build) into src-tauri/lib.
# The LGPL build, loaded at runtime, leaves Nightglass free to keep its own licence; the default build is GPL.
$ErrorActionPreference = 'Stop'
$lib = Join-Path $PSScriptRoot '..\src-tauri\lib'
if (Test-Path (Join-Path $lib 'libmpv-2.dll')) { Write-Host 'libmpv-2.dll already present'; exit 0 }

$api = @{ Headers = $(if ($env:GH_TOKEN) { @{ Authorization = "Bearer $env:GH_TOKEN" } } else { @{} }) }
$release = Invoke-RestMethod @api 'https://api.github.com/repos/zhongfly/mpv-winbuild/releases/latest'
$asset = $release.assets | Where-Object { $_.name -match '^mpv-dev-lgpl-x86_64-\d' } | Select-Object -First 1
if (-not $asset) { throw "no mpv-dev-lgpl-x86_64 asset in $($release.tag_name)" }

$tmp = Join-Path ([IO.Path]::GetTempPath()) "nightglass-libmpv-$([guid]::NewGuid())"
New-Item -ItemType Directory $tmp | Out-Null
try {
    $archive = Join-Path $tmp $asset.name
    Write-Host "Downloading $($asset.name)"
    Invoke-WebRequest $asset.browser_download_url -OutFile $archive
    # Windows' bsdtar reads 7z.
    & "$env:SystemRoot\System32\tar.exe" -xf $archive -C $tmp libmpv-2.dll
    New-Item -ItemType Directory -Force $lib | Out-Null
    Copy-Item (Join-Path $tmp 'libmpv-2.dll') $lib
    # Shipped with the installer, next to the licences: which build this is and the exact sources it came from.
    # mpv-winbuild keeps releases 30 days only, so the commits (kept for good upstream) are what to point at.
    $mpv = [regex]::Match($release.body, 'mpv-player/mpv/commit/([0-9a-f]{40})').Groups[1].Value
    $ffmpeg = ($release.assets | ForEach-Object { [regex]::Match($_.name, '^ffmpeg-lgpl-x86_64-git-([0-9a-f]+)\.') } | Where-Object Success | Select-Object -First 1).Groups[1].Value
    $scripts = (Invoke-RestMethod @api "https://api.github.com/repos/zhongfly/mpv-winbuild/commits/$($release.tag_name)").sha
    if (-not ($mpv -and $ffmpeg -and $scripts)) { throw "could not read the mpv, FFmpeg and build script commits of $($release.tag_name)" }
    @(
        "libmpv-2.dll: $($asset.name) (mpv-winbuild release $($release.tag_name))",
        "mpv source: https://github.com/mpv-player/mpv/tree/$mpv",
        "FFmpeg source: https://git.ffmpeg.org/gitweb/ffmpeg.git/commit/$ffmpeg",
        "Build scripts and versions of the other libraries: https://github.com/zhongfly/mpv-winbuild/tree/$scripts"
    ) | Set-Content -Encoding utf8 (Join-Path $lib 'libmpv-version.txt')
    # tauri-build keeps an older copy next to the exe: drop it and rerun the build script so the next build copies this one.
    Remove-Item (Join-Path $PSScriptRoot '..\target\*\libmpv-2.dll') -ErrorAction SilentlyContinue
    (Get-Item (Join-Path $PSScriptRoot '..\src-tauri\build.rs')).LastWriteTime = Get-Date
    Write-Host "libmpv-2.dll from $($release.tag_name)"
} finally {
    Remove-Item -Recurse -Force $tmp
}
