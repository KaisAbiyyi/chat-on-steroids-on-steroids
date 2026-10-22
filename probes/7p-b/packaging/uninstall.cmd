@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Uninstall-Probe.ps1" -Destination "%~dp0"
exit /b %ERRORLEVEL%
