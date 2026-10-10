# Update this checkout and launch Jawjack with its existing local profile.
[CmdletBinding()]
param([string]$ProfileRoot)

$ErrorActionPreference = 'Stop'
if ($ProfileRoot) {
    if (!(Test-Path -LiteralPath $ProfileRoot -PathType Container)) {
        throw 'The requested profile directory does not exist.'
    }
    $ProfileRoot = (Resolve-Path -LiteralPath $ProfileRoot).ProviderPath
}
$branch = 'recreation/gpui-foundation'
$repo = Split-Path -Parent $PSScriptRoot
$oldLocation = Get-Location
$oldLocalAppData = $env:LOCALAPPDATA

try {
    Set-Location $repo
    $currentBranch = & git branch --show-current
    if ($LASTEXITCODE -ne 0 -or $currentBranch -ne $branch) {
        throw "Expected branch $branch. No branch was changed."
    }
    $dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the checkout.' }
    if ($dirty) { throw 'Uncommitted changes found. Save them before updating; nothing was discarded.' }

    $exe = Join-Path $PSScriptRoot 'target\release\chat-workbench.exe'
    $running = Get-Process -Name 'chat-workbench' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $exe }
    if ($running) { throw 'Close this checkout''s Jawjack window, then run this script again.' }

    $rustup = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo\bin\rustup.exe'
    if (!(Test-Path -LiteralPath $rustup)) { throw 'Rustup was not found in your user .cargo\bin folder.' }
    if (!$ProfileRoot) {
        $ProfileRoot = [Environment]::GetFolderPath('LocalApplicationData')
        # Preserve the original development preview's profile when it exists.
        $preview = Join-Path $ProfileRoot 'JawjackManual-b92a50a5-20261009'
        if (Test-Path -LiteralPath (Join-Path $preview 'ChatWorkbench\workspace.json')) {
            $ProfileRoot = $preview
        }
    }
    $ProfileRoot = [IO.Path]::GetFullPath($ProfileRoot)

    Write-Host 'Updating Jawjack...' -ForegroundColor Cyan
    & git pull --ff-only origin $branch
    if ($LASTEXITCODE -ne 0) { throw 'Pull failed. No build or launch was attempted.' }
    Set-Location $PSScriptRoot
    $drive = Get-PSDrive -Name ([IO.Path]::GetPathRoot($PSScriptRoot).TrimEnd('\').TrimEnd(':')) -ErrorAction SilentlyContinue
    if ($drive -and $null -ne $drive.Free -and $drive.Free -lt 25GB) {
        if (!(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'target\release\deps'))) {
            throw 'Less than 25 GB is free and no release build cache exists. Free space before this heavy build.'
        }
        Write-Warning 'Less than 25 GB is free. Existing release artifacts will be reused where possible; rebuilding dependencies may still need substantial space.'
    }
    Write-Host 'Building Jawjack...' -ForegroundColor Cyan
    & $rustup run 1.99.0 cargo build -p chat-workbench --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build failed. The app was not launched.' }
    $env:LOCALAPPDATA = $ProfileRoot
    Write-Host "Launching with profile: $ProfileRoot" -ForegroundColor Cyan
    Start-Process -FilePath $exe -WorkingDirectory $PSScriptRoot
} finally {
    $env:LOCALAPPDATA = $oldLocalAppData
    Set-Location $oldLocation
}
