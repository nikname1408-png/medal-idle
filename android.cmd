@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Проверяю подключённые Android-устройства...
adb devices
echo.
echo Если список пуст — запусти эмулятор в Android Studio (Device Manager).
echo Сборка начнётся через 5 секунд...
timeout /t 5 >nul
dx serve --android
pause
