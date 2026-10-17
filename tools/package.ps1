# Builds mcview in release mode and assembles a portable game folder (dist\MinecraftRS) that can be zipped and sent to a friend.
# Run from the mc-rs folder:  powershell -ExecutionPolicy Bypass -File tools\package.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
cargo build --release -p mc-client --bin mcview
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$src = Join-Path (Split-Path -Parent $root) "Minecraft.Client"
$out = Join-Path $root "dist\MinecraftRS"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force "$out\Minecraft.Client\Common" | Out-Null
New-Item -ItemType Directory -Force "$out\Minecraft.Client\Windows64Media" | Out-Null

Copy-Item "target\release\mcview.exe" "$out\MinecraftRS.exe"
# textures / fonts / gui (read at run time)
robocopy "$src\Common\res" "$out\Minecraft.Client\Common\res" /E /NFL /NDL /NJH /NJS | Out-Null
# sounds
robocopy "$src\Windows64Media\Sound" "$out\Minecraft.Client\Windows64Media\Sound" /E /NFL /NDL /NJH /NJS | Out-Null

Compress-Archive -Path "$out\*" -DestinationPath "$root\dist\MinecraftRS.zip" -Force
Write-Host "Done: $out"
Write-Host "Zip : $root\dist\MinecraftRS.zip"
