# Downloads libmpv-2.dll (zhongfly/mpv-winbuild, x86_64 baseline build) into src-tauri/lib.
$ErrorActionPreference = 'Stop'
$lib = Join-Path $PSScriptRoot '..\src-tauri\lib'
if (Test-Path (Join-Path $lib 'libmpv-2.dll')) { Write-Host 'libmpv-2.dll already present'; exit 0 }

$release = Invoke-RestMethod 'https://api.github.com/repos/zhongfly/mpv-winbuild/releases/latest'
$asset = $release.assets | Where-Object { $_.name -match '^mpv-dev-x86_64-\d' } | Select-Object -First 1
if (-not $asset) { throw "no mpv-dev-x86_64 asset in $($release.tag_name)" }

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
    Write-Host "libmpv-2.dll from $($release.tag_name)"
} finally {
    Remove-Item -Recurse -Force $tmp
}
