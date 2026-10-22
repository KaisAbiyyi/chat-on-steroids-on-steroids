@echo off
setlocal
if not exist D:\ (echo D: drive required & exit /b 9)
if not defined COS_7PL1_INSTALL_ROOT set "COS_7PL1_INSTALL_ROOT=D:\Apps\CoSoS-7PL1-Synthetic-Probe"
set "TEMP=D:\Apps\CoSoS-7PL1-Temp"
set "TMP=%TEMP%"
if not exist "%TEMP%" mkdir "%TEMP%"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install-Probe.ps1" -PayloadZip "%~dp0payload.zip" -PayloadSha256 "%~dp0payload.sha256" -Destination "%COS_7PL1_INSTALL_ROOT%"
exit /b %ERRORLEVEL%
