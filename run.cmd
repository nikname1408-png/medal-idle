@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Запускаю игру в десктопном окне...
dx serve --desktop
pause
