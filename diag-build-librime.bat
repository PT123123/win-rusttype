@echo off
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
set RIME_ROOT=C:\Users\user\Desktop\win-rusttype\libximecore\librime
set BOOST_ROOT=%RIME_ROOT%\deps\boost-1.89.0
cd /d %RIME_ROOT%
call build.bat librime shared 2>&1
echo EXITCODE=%errorlevel%
