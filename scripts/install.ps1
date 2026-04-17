#Requires -Version 7
$ErrorActionPreference = "Stop"

$releaseRepo = if ($env:MAESTRO_RELEASE_REPO) { $env:MAESTRO_RELEASE_REPO } else { "ReinaMacCredy/maestro" }
$requestedVersion = if ($env:MAESTRO_VERSION) { $env:MAESTRO_VERSION } else { "latest" }
$installDir = if ($env:MAESTRO_INSTALL_DIR) { $env:MAESTRO_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\maestro" }
$targetBin = Join-Path $installDir "maestro.exe"

function Write-Info { param($msg) Write-Host "[ok] $msg" }
function Write-Warn { param($msg) Write-Host "[!] $msg" }
function Write-Fail { param($msg) Write-Host "[!] $msg" -ForegroundColor Red; exit 1 }

Write-Host "maestro release installer"
Write-Host ""

$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64"  { $archSlug = "x64" }
    "x86_64" { $archSlug = "x64" }
    default  { Write-Fail "Unsupported architecture: $arch. Release installs support x64." }
}

$asset = "maestro-windows-$archSlug.exe"
$oldBin = "$targetBin.old"
$restored = $false

$baseUrl = "https://github.com/$releaseRepo/releases"
if ($requestedVersion -eq "latest") {
    $url = "$baseUrl/latest/download/$asset"
} else {
    $tag = $requestedVersion
    if (-not $tag.StartsWith("v")) { $tag = "v$tag" }
    $url = "$baseUrl/download/$tag/$asset"
}

Write-Host "Installing asset: $asset"
Write-Host "Download URL: $url"
Write-Host ""

New-Item -ItemType Directory -Force -Path $installDir | Out-Null

$tempBin = Join-Path $installDir ".maestro.tmp.$([guid]::NewGuid().ToString('N')).exe"
try {
    Invoke-WebRequest -Uri $url -OutFile $tempBin -UseBasicParsing

    if (Test-Path $targetBin) {
        if (Test-Path $oldBin) { Remove-Item $oldBin -Force -ErrorAction SilentlyContinue }
        Move-Item -Force $targetBin $oldBin
    }
    Move-Item -Force $tempBin $targetBin

    $version = & $targetBin --version
    if ($LASTEXITCODE -ne 0) { Write-Fail "Installation verification failed" }

    Write-Info "Installed maestro $version to $targetBin"
} catch {
    if (Test-Path $tempBin) { Remove-Item $tempBin -Force -ErrorAction SilentlyContinue }
    if (Test-Path $oldBin) {
        if (Test-Path $targetBin) { Remove-Item $targetBin -Force -ErrorAction SilentlyContinue }
        try {
            Move-Item -Force $oldBin $targetBin
            $restored = $true
        } catch {
            Write-Warn "Failed to restore previous binary from $oldBin"
        }
    }
    if ($restored) { Write-Warn "Restored previous maestro.exe after installation failure" }
    throw
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not ($userPath -split ";" | Where-Object { $_.TrimEnd("\") -eq $installDir.TrimEnd("\") })) {
    Write-Host ""
    Write-Warn "$installDir is not in your user PATH"
    Write-Host "    Add it manually via System Properties > Environment Variables,"
    Write-Host "    or run this in PowerShell:"
    Write-Host "    [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$installDir`", 'User')"
    Write-Host "    Then open a new terminal to pick up the change."
}

Write-Host ""
Write-Info "Running maestro install..."
& $targetBin install
