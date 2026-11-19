# Builds the two Windows deliverables in dist\:
#   MailCraft-<version>-Portable.exe   single exe, settings kept in a MailCraft-data folder beside it
#   MailCraft-Setup-<version>.exe      Inno Setup installer (needs Inno Setup 6)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build a échoué" }

New-Item -ItemType Directory -Force dist | Out-Null
# Only the previous builds: a MailCraft-data folder left by a portable run is kept.
Get-ChildItem dist -File -Filter "MailCraft-*.exe" | Remove-Item -Force
# "Portable" in the name switches the exe to portable mode (see platform::portable_dir).
Copy-Item target\release\mailcraft.exe "dist\MailCraft-$version-Portable.exe"

$iscc = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($iscc) {
    & $iscc /Q "/DAppVersion=$version" installer\mailcraft.iss
    if ($LASTEXITCODE -ne 0) { throw "ISCC a échoué" }
} else {
    Write-Warning "Inno Setup 6 introuvable : installateur non généré (seule la version portable est dans dist\)"
}
Get-ChildItem dist | Select-Object Name, Length
