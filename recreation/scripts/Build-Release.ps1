<#
.SYNOPSIS
Builds and packages a Jawjack release from committed HEAD. Never publishes.

.DESCRIPTION
Exports committed HEAD of recreation/ to target/release-src (so uncommitted
work in progress never ships), builds chat-workbench and jawjack-setup in
release mode into the EXISTING target directory, then runs
scripts/Package-Release.py --no-build to produce release-out/<version>/.

Note: this replaces target/release/chat-workbench.exe with the HEAD build.

.PARAMETER NotesFile
Reviewed user-facing Markdown release notes.

.PARAMETER SignThumbprint
Optional code-signing certificate SHA-1 thumbprint. Without it, or
JAWJACK_SIGN_THUMBPRINT, signing is skipped.
#>
param(
    [Parameter(Mandatory = $true)][string]$NotesFile,
    [string]$SignThumbprint = $env:JAWJACK_SIGN_THUMBPRINT,
    [string]$Toolchain = $(if ($env:JAWJACK_TOOLCHAIN) { $env:JAWJACK_TOOLCHAIN } else { '1.99.0' })
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$rustup = Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'
if (-not (Test-Path -LiteralPath $rustup)) { throw 'Install rustup (MSVC toolchain) first.' }
$notes = (Resolve-Path -LiteralPath $NotesFile).Path
$src = Join-Path $repo 'target\release-src'
$target = Join-Path $repo 'target'

Push-Location $repo
try {
    $head = (git rev-parse --short HEAD).Trim()
    Write-Host "Exporting committed HEAD $head (recreation/) to $src ..."
    if (Test-Path -LiteralPath $src) { Remove-Item -Recurse -Force -LiteralPath $src }
    New-Item -ItemType Directory -Force $src | Out-Null
    $archive = Join-Path $target 'release-src.tar'
    # HEAD:./ is the recreation/ subtree relative to this directory.
    git archive --format=tar -o $archive 'HEAD:./'
    if ($LASTEXITCODE -ne 0) { throw 'git archive export failed.' }
    & "$env:SystemRoot\System32\tar.exe" -xf $archive -C $src
    if ($LASTEXITCODE -ne 0) { throw 'Extracting the HEAD snapshot failed.' }
    Remove-Item -Force -LiteralPath $archive
} finally { Pop-Location }

Push-Location $src
try {
    & $rustup run $Toolchain cargo build --release --locked -p chat-workbench -p jawjack-setup --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
} finally { Pop-Location }

Push-Location $repo
try {
    $env:JAWJACK_TOOLCHAIN = $Toolchain
    $arguments = @('scripts/Package-Release.py', '--notes-file', $notes, '--no-build', '--source-commit', $head)
    if ($SignThumbprint) { $arguments += @('--sign-thumbprint', $SignThumbprint) }
    python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Release packaging failed.' }
} finally { Pop-Location }
