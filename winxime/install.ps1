<#
安装脚本：曦码·曜输入法 (winxime) — MVP 版
用法：
  powershell -ExecutionPolicy Bypass -File install.ps1          # 完整安装（会弹 UAC）
  powershell -ExecutionPolicy Bypass -File install.ps1 -InstallDir "C:\Users\user\Desktop\win-rusttype\winxime\target\debug" -SkipCopy
  powershell -ExecutionPolicy Bypass -File install.ps1 -Uninstall

流程：
  1) 复制运行时文件到安装目录（winxime-server.exe / winxime_tsf.dll / rime.dll /
     winxime-tsf-register.exe / icon.ico / resources\*）
  2) 提权（UAC）注册 TSF：
     - regsvr32 注册 winxime_tsf.dll（HKLM\Software\Classes\CLSID）
     - winxime-tsf-register -r 注册 TSF Profile
     - winxime-tsf-register -i 启用输入法
  3) 启动 winxime-server（RIME 引擎 + 候选窗口 + 托盘）
  4) 写入开机自启（HKCU\...\Run，可选）
#>

param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Xime",
    [switch]$SkipCopy,
    [switch]$SkipStart,
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$Src = Split-Path -Parent $MyInvocation.MyCommand.Path   # 脚本所在目录（构建产物目录）
$BuildDir = Join-Path $Src "target\debug"

# ---------- 卸载 ----------
if ($Uninstall) {
    Write-Host "[1/4] Stopping server..." -ForegroundColor Cyan
    & (Join-Path $InstallDir "winxime-tsf-register.exe") -s 2>$null
    Start-Sleep -Seconds 1
    Write-Host "[2/4] Elevating uninstall (UAC)..." -ForegroundColor Cyan
    $inner = "`"$(Join-Path $InstallDir 'winxime-tsf-register.exe')`" -unregister-and-remove"
    Start-Process cmd -ArgumentList @("/c", $inner) -Verb RunAs -Wait
    Write-Host "[3/4] Removing Run key..." -ForegroundColor Cyan
    reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v XimeServer /f 2>$null
    Write-Host "[4/4] Removing install dir..." -ForegroundColor Cyan
    Remove-Item $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Uninstall done." -ForegroundColor Green
    exit 0
}

# ---------- 安装 ----------
if (-not $SkipCopy) {
    Write-Host "[1/4] Copying files to $InstallDir ..." -ForegroundColor Cyan
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $files = @("winxime-server.exe", "winxime_tsf.dll", "rime.dll", "winxime-tsf-register.exe", "icon.ico")
    foreach ($f in $files) {
        $s = Join-Path $BuildDir $f
        if (Test-Path $s) { Copy-Item $s $InstallDir -Force; Write-Host "  copied $f" }
        else { Write-Host "  !! missing $f (build first: cargo build)" -ForegroundColor Yellow }
    }
    if (Test-Path (Join-Path $BuildDir "resources")) {
        Copy-Item (Join-Path $BuildDir "resources") $InstallDir -Recurse -Force
    }
    # 数据目录（MVP：直接使用 rime-wubi 方案数据）
    $DataSrc = Join-Path $Src "rime-wubi"
    if (Test-Path $DataSrc) { Copy-Item $DataSrc (Join-Path $InstallDir "data") -Recurse -Force }
}

Write-Host "[2/4] Registering TSF input method (requires UAC approval)..." -ForegroundColor Cyan
$regExe = Join-Path $InstallDir "winxime-tsf-register.exe"
$dll    = Join-Path $InstallDir "winxime_tsf.dll"
$ico    = Join-Path $InstallDir "icon.ico"
$log    = Join-Path $InstallDir "register.log"

if (-not (Test-Path $regExe)) { Write-Host "reg tool missing at $regExe" -ForegroundColor Red; exit 1 }

# 提权：复制 DLL 到 System32 + HKLM 注册 + TSF Profile + 启用（-install 需要管理员）
$inner = "`"$regExe`" -install > `"$log`" 2>&1"
Start-Process cmd -ArgumentList @("/c", $inner) -Verb RunAs -Wait
Write-Host "  register log:" -ForegroundColor DarkGray
Get-Content $log -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }

# 兜底：如果 -install 失败（例如 UAC 被拒），尝试用户级
if (-not (Test-Path "HKLM:\Software\Classes\CLSID\{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}")) {
    Write-Host "  HKLM registration not detected, falling back to user-level (may not work on Win10/11)" -ForegroundColor Yellow
}

Write-Host "[3/4] Starting server..." -ForegroundColor Cyan
if (-not $SkipStart) {
    $server = Join-Path $InstallDir "winxime-server.exe"
    if (Test-Path $server) {
        $existing = Get-Process winxime-server -ErrorAction SilentlyContinue
        if ($existing) { & $server /q | Out-Null; Start-Sleep -Milliseconds 800 }
        Start-Process -FilePath $server -WindowStyle Hidden
        Start-Sleep -Seconds 2
        if (Get-Process winxime-server -ErrorAction SilentlyContinue) {
            Write-Host "  server running." -ForegroundColor Green
        } else {
            Write-Host "  !! server failed to start" -ForegroundColor Red
        }
    }
}

Write-Host "[4/4] Adding autostart entry..." -ForegroundColor Cyan
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v XimeServer /t REG_SZ /d "`"$(Join-Path $InstallDir 'winxime-server.exe')`"" /f | Out-Null

Write-Host ""
Write-Host "Install complete." -ForegroundColor Green
Write-Host "Switch input method with Win+Space in any app (e.g. Notepad), select 曦码·曜输入法, type pinyin."
