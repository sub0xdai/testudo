# install.ps1 — PowerShell installer for testudo (Windows native)
#
# Usage:
#   powershell -c "irm https://api.testudo.vip/install.ps1 | iex"
#
# Detects architecture, downloads the latest .zip from GitHub Releases,
# extracts to ~\bin, and adds to PATH.

param()

$ErrorActionPreference = "Stop"

# ── Configuration ──────────────────────────────────────────────

$Repo = "sub0xdai/testudo"
$Binary = "testudo"
$InstallDir = "$env:USERPROFILE\bin"
$ReleaseUrl = "https://github.com/$Repo/releases/latest/download"

# ── Architecture detection ─────────────────────────────────────

$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }
$Target = "${Arch}-pc-windows-msvc"
$Archive = "${Binary}-${Target}.zip"
$DownloadUrl = "${ReleaseUrl}/${Archive}"
$TmpZip = "$env:TEMP\$Archive"

Write-Host "→ Downloading testudo for Windows $Arch..."
Write-Host "  $DownloadUrl"

try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TmpZip -ErrorAction Stop
} catch {
    Write-Host ""
    Write-Host "Error: Failed to download testudo." -ForegroundColor Red
    Write-Host "Check https://github.com/$Repo/releases for available builds."
    exit 1
}

# ── Extract ────────────────────────────────────────────────────

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

try {
    Expand-Archive -Path $TmpZip -DestinationPath $InstallDir -Force
} catch {
    Write-Host "Error: Failed to extract zip." -ForegroundColor Red
    Remove-Item -Force $TmpZip -ErrorAction SilentlyContinue
    exit 1
}

Remove-Item -Force $TmpZip -ErrorAction SilentlyContinue

# ── Verify ─────────────────────────────────────────────────────

$BinPath = Join-Path $InstallDir "$Binary.exe"
if (-not (Test-Path $BinPath)) {
    Write-Host "Error: Binary not found after install: $BinPath" -ForegroundColor Red
    exit 1
}

try {
    $version = & $BinPath --version 2>$null
    Write-Host "   Installed: $version"
} catch {
    Write-Host "   Installed: (version check failed)"
}

# ── Add to PATH ────────────────────────────────────────────────

$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
    Write-Host "   Added to user PATH"
} else {
    Write-Host "   PATH already configured"
}

# ── Done ───────────────────────────────────────────────────────

Write-Host ""
Write-Host "══════════════════════════════════════════════════════════════"
Write-Host "  testudo installed successfully!"
Write-Host "     Binary: $BinPath"
Write-Host "══════════════════════════════════════════════════════════════"
Write-Host ""
Write-Host "  Next steps:"
Write-Host "    testudo init        Complete setup wizard"
Write-Host "    testudo --help      See all commands"
Write-Host ""
