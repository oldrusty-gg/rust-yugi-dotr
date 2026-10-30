@echo off
rem Runs play-online.ps1 for this one launch without changing the system's script policy.
rem Usage: scripts\play-online.cmd [-Mode bot|host|join] [other play-online.ps1 options]
pushd "%~dp0.."
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0play-online.ps1" %*
popd
