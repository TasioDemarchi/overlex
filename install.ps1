# OverLex Installer
# Descarga e instala la ultima version de OverLex desde GitHub Releases
# Uso: irm https://raw.githubusercontent.com/TasioDemarchi/overlex/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$repo = "TasioDemarchi/overlex"
$appName = "OverLex"
$installDir = "$env:LOCALAPPDATA\OverLex"

Write-Host ""
Write-Host "  OverLex Installer" -ForegroundColor Cyan
Write-Host "  ==================" -ForegroundColor Cyan
Write-Host ""

# Obtener la ultima release
Write-Host "Buscando la ultima version..." -ForegroundColor Gray
try {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers @{ "User-Agent" = "OverLex-Installer" }
} catch {
    Write-Host "Error: No se pudo conectar a GitHub. Verifica tu conexion a internet." -ForegroundColor Red
    exit 1
}

$version = $release.tag_name
Write-Host "Version encontrada: $version" -ForegroundColor Green

# Buscar el instalador .exe en los assets
$asset = $release.assets | Where-Object { $_.name -like "*setup*.exe" } | Select-Object -First 1

if (-not $asset) {
    Write-Host "Error: No se encontro el instalador en la release $version." -ForegroundColor Red
    Write-Host "Intenta descargar manualmente desde: https://github.com/$repo/releases/latest" -ForegroundColor Yellow
    exit 1
}

$installerUrl = $asset.browser_download_url
$installerName = $asset.name
$tempPath = "$env:TEMP\$installerName"

Write-Host "Descargando $installerName..." -ForegroundColor Gray
try {
    Invoke-WebRequest -Uri $installerUrl -OutFile $tempPath -UseBasicParsing
} catch {
    Write-Host "Error al descargar el instalador: $_" -ForegroundColor Red
    exit 1
}

Write-Host "Instalando $appName $version..." -ForegroundColor Gray
try {
    # /S = instalacion silenciosa (NSIS)
    Start-Process -FilePath $tempPath -ArgumentList "/S" -Wait
} catch {
    Write-Host "Error durante la instalacion: $_" -ForegroundColor Red
    exit 1
}

# Limpiar el instalador temporal
Remove-Item $tempPath -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "  $appName $version instalado correctamente!" -ForegroundColor Green
Write-Host "  Busca OverLex en el menu de inicio o en la bandeja del sistema." -ForegroundColor Cyan
Write-Host ""
