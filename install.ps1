<#
.SYNOPSIS
    Install ctrlr from GitHub releases.

.DESCRIPTION
    Windows counterpart to install.sh. Unlike that one it verifies the
    download against the release's checksums.txt, because there is no package
    manager in front of it here.

.PARAMETER Version
    Tag to install, e.g. v0.11.0. Defaults to the latest release.

.PARAMETER InstallDir
    Where to put ctrlr.exe. Defaults to $env:LOCALAPPDATA\Programs\ctrlr.

.EXAMPLE
    irm https://github.com/capydev42/ctrlr/releases/latest/download/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Version v0.11.0 -InstallDir C:\tools\ctrlr
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$Repo = 'capydev42/ctrlr'
)

$ErrorActionPreference = 'Stop'
$asset = 'ctrlr-x86_64-pc-windows-msvc.zip'

# Not a param default: that is evaluated while binding, so a missing
# LOCALAPPDATA would abort before the functions below are even defined.
function Get-DefaultInstallDir() {
    $base = $env:LOCALAPPDATA
    if (-not $base) { $base = $env:APPDATA }
    if (-not $base) { $base = Join-Path $HOME '.local' }
    Join-Path $base 'Programs\ctrlr'
}

function Get-ReleaseBase([string]$repo, [string]$version) {
    if ($version) {
        "https://github.com/$repo/releases/download/$version"
    } else {
        "https://github.com/$repo/releases/latest/download"
    }
}

# The checksums file is "<sha256>  <asset>" lines, one per release artifact.
# A missing entry is a hard error rather than a skipped check: shipping an
# unverified binary quietly is worse than failing loudly.
function Get-ExpectedHash([string]$checksums, [string]$asset) {
    foreach ($line in $checksums -split "`r?`n") {
        $fields = $line.Trim() -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].Trim() -eq $asset) {
            return $fields[0].Trim().ToLowerInvariant()
        }
    }
    throw "checksums.txt has no entry for $asset"
}

function Add-ToUserPath([string]$dir) {
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($current -split ';' | Where-Object { $_ })
    if ($entries -contains $dir) { return $false }

    $updated = (@($entries) + $dir) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $updated, 'User')
    return $true
}

# Dot-sourcing for tests stops here; running the file goes on to install.
if ($MyInvocation.InvocationName -eq '.') { return }

if (-not $InstallDir) { $InstallDir = Get-DefaultInstallDir }
$base = Get-ReleaseBase $Repo $Version
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("ctrlr-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work -Force | Out-Null

try {
    $zip = Join-Path $work $asset
    Write-Host "Downloading $asset..."
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $zip -UseBasicParsing

    Write-Host "Verifying checksum..."
    $checksums = (Invoke-WebRequest -Uri "$base/checksums.txt" -UseBasicParsing).Content
    $expected = Get-ExpectedHash $checksums $asset
    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "checksum mismatch for ${asset}: expected $expected, got $actual"
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    # -Force so an upgrade over an existing install works.
    Expand-Archive -LiteralPath $zip -DestinationPath $InstallDir -Force

    $exe = Join-Path $InstallDir 'ctrlr.exe'
    if (-not (Test-Path -LiteralPath $exe)) { throw "the archive did not contain ctrlr.exe" }

    Write-Host "Installed $exe" -ForegroundColor Green

    if (Add-ToUserPath $InstallDir) {
        Write-Host "Added $InstallDir to your user PATH." -ForegroundColor Green
        Write-Host "It applies to new shells, not this one."
    }

    Write-Host ""
    Write-Host "Next: ctrlr init    (adds the Ctrl+R binding to your PowerShell profile)"
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
