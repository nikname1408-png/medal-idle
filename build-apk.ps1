[CmdletBinding()]
param(
    [switch]$Install,
    [string]$Serial = ""
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

function Stop-Build([string]$message) {
    Write-Host ""
    Write-Host "ОШИБКА: $message" -ForegroundColor Red
    exit 1
}

function Invoke-Step([string]$title, [scriptblock]$action) {
    Write-Host ""
    Write-Host "== $title ==" -ForegroundColor Cyan
    & $action
    if ($LASTEXITCODE -ne 0) {
        Stop-Build "шаг «$title» завершился с кодом $LASTEXITCODE"
    }
}

if (-not (Get-Command dx -ErrorAction SilentlyContinue)) {
    Stop-Build "не найден dx. Установи Dioxus CLI: cargo install dioxus-cli"
}

$sdk = $env:ANDROID_HOME
if (-not $sdk) { $sdk = $env:ANDROID_SDK_ROOT }
if (-not $sdk) { $sdk = Join-Path $env:LOCALAPPDATA "Android\Sdk" }
if (-not (Test-Path $sdk)) {
    Stop-Build "не найден Android SDK. Задай ANDROID_HOME."
}

if (-not $env:JAVA_HOME -and -not (Get-Command java -ErrorAction SilentlyContinue)) {
    Stop-Build "не найден Java. Задай JAVA_HOME (JBR из Android Studio)."
}

# 1. Rust -> libmain.so для телефона (arm64-v8a).
Invoke-Step "1/4 Сборка Rust под arm64 (телефон)" {
    dx build --platform android --release --target aarch64-linux-android
}

$gradleRoot = Join-Path $root "target\dx\medieval-idle\release\android\app"
if (-not (Test-Path $gradleRoot)) {
    Stop-Build "не найден сгенерированный Android-проект: $gradleRoot"
}

# Gradle ищет SDK в local.properties.
$sdkEscaped = $sdk -replace "\\", "\\\\" -replace ":", "\:"
Set-Content -Path (Join-Path $gradleRoot "local.properties") -Value "sdk.dir=$sdkEscaped" -Encoding ASCII

# 2. Kotlin/Java + assets + libmain.so -> APK.
Invoke-Step "2/4 Упаковка APK (Gradle)" {
    & (Join-Path $gradleRoot "gradlew.bat") -p $gradleRoot clean assembleDebug
}

$builtApk = Join-Path $gradleRoot "app\build\outputs\apk\debug\app-debug.apk"
if (-not (Test-Path $builtApk)) {
    Stop-Build "Gradle не создал APK: $builtApk"
}

$distDir = Join-Path $root "dist"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null
$apk = Join-Path $distDir "pole.apk"
Copy-Item $builtApk $apk -Force

# Проверяем, что внутри есть библиотека именно под телефон.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($apk)
try {
    $arm64 = $zip.Entries | Where-Object { $_.FullName -eq "lib/arm64-v8a/libmain.so" }
} finally {
    $zip.Dispose()
}
if (-not $arm64) {
    Stop-Build "в APK нет lib/arm64-v8a/libmain.so — телефон её не запустит."
}

Write-Host ""
Write-Host "== 3/4 Готово ==" -ForegroundColor Cyan
Write-Host "APK: $apk" -ForegroundColor Green
Write-Host ("Размер: {0:N1} МБ" -f ((Get-Item $apk).Length / 1MB))

if (-not $Install) {
    Write-Host ""
    Write-Host "Установить на подключённый телефон:" -ForegroundColor Yellow
    Write-Host "  .\build-apk.cmd -Install"
    exit 0
}

if (-not (Get-Command adb -ErrorAction SilentlyContinue)) {
    Stop-Build "не найден adb. Добавь platform-tools в PATH."
}

$deviceList = & adb devices
$online = $deviceList | Where-Object { $_ -match "^\S+\s+device$" }
if (-not $online) {
    Stop-Build "телефон не подключён. Проверь USB и «Отладку по USB» (adb devices)."
}

$adbArgs = @()
if ($Serial) { $adbArgs += @("-s", $Serial) }

Invoke-Step "4/4 Установка на телефон" {
    adb @adbArgs install -r $apk
}

Write-Host ""
Write-Host "Запускаю игру..." -ForegroundColor Cyan
adb @adbArgs shell am start -n club.tooken.pole/dev.dioxus.main.MainActivity | Out-Null

Write-Host "Готово: APK установлен и запущен." -ForegroundColor Green

