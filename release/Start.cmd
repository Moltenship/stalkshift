@echo off
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup\Setup.ps1" -Action Start
pause
