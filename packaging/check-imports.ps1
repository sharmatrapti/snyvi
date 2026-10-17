# Fails when a Windows executable imports a DLL a clean Windows does not have.
# The Visual C++ runtime is the one that got through: every runner and most
# desktops have the Redistributable installed by something else, so nothing
# here noticed until winget's test machine, which has only Windows.
# .cargo/config.toml links it statically; this keeps it that way.
param([string[]]$Exes = @("target/release/snyvi.exe", "target/release/snyvi-app.exe"))
$ErrorActionPreference = "Stop"
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$dumpbin = & $vswhere -latest -products * -find "VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe" | Select-Object -First 1
if (-not $dumpbin) { throw "no dumpbin.exe on this runner" }
foreach ($exe in $Exes) {
  $deps = & $dumpbin /nologo /dependents $exe | Out-String
  if ($deps -match "(?i)\b(vcruntime\d+(_\d+)?|msvcp\d+|ucrtbase)\.dll") {
    Write-Output $deps
    throw "$exe needs the Visual C++ runtime ($($Matches[0])), which a clean Windows does not have"
  }
  Write-Output "$exe imports nothing outside Windows"
}
