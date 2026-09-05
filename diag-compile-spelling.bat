@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
set RIME_ROOT=%~dp0libximecore\librime
set BOOST_ROOT=%RIME_ROOT%\deps\boost-1.89.0
cd /d %RIME_ROOT%
echo === Compiling spelling.cc with full include paths ===
cl /nologo /c /std:c++17 /utf-8 /MT /O2 /Ob2 /D NDEBUG /DWIN32 /D_WINDOWS /DNDEBUG^
 /I"%RIME_ROOT%\build\src" /I"%RIME_ROOT%\src" /I"%RIME_ROOT%\include" /I"%BOOST_ROOT%"^
 /I"%RIME_ROOT%\deps\marisa-trie\include" /I"%RIME_ROOT%\deps\opencc\src" /I"%RIME_ROOT%\deps\yaml-cpp\include"^
 /I"%RIME_ROOT%\deps\glog\src" /I"%RIME_ROOT%\deps\leveldb\include" /I"%RIME_ROOT%\deps\googletest\googletest\include"^
 /Fo"%TEMP%\spelling_test.obj" "%RIME_ROOT%\src\rime\algo\spelling.cc" 2>&1
echo EXITCODE=%errorlevel%
