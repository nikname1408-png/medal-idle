@echo off
chcp 65001 >nul
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-apk.ps1" %*
if errorlevel 1 (
    pause
    exit /b 1
)
if "%~1"=="" pause
