# 03 · 托盘菜单「输入法设置 / 关于」点击无反应 — 排查与修复

> 日期：2026-09-05
> 现象：右键托盘图标，点「输入法设置」「关于」均无任何窗口弹出，也无报错；其它菜单项（如退出）正常。
> 结论：**已修复**。根因有两层，第二层（UAC 安装程序检测，os error 740）是关键。

---

## 1. 调用链回顾

托盘菜单事件链（`winxime-server/src/tray.rs` + `main.rs`）：

```
右键托盘 → WM_RBUTTONUP → TrackPopupMenu
菜单项点击 → WM_COMMAND → handle_menu_command
          → on_action(TrayAction::OpenSettings / About)
          → spawn 同目录 winxime-setup.exe（About 额外带 --about）
```

设置/关于窗口不是 server 自己绘制，而是另起 `winxime-setup.exe` 进程。

## 2. 排查过程（判别式定位）

| 步骤 | 实验 | 结果 | 结论 |
|------|------|------|------|
| 1 | 命令行直接 `Start-Process winxime-setup.exe --about` | 窗口正常弹出 | setup 程序本身没坏 |
| 2 | `FindWindow(XimeTrayWindow)` + `PostMessage WM_COMMAND 1002(设置)` | setup 未启动 | 复现，且绕过了 TrackPopupMenu |
| 3 | `PostMessage WM_COMMAND 1005(退出)` | server 正常退出 | 消息通路与 on_action 闭包正常，问题在 OpenSettings 分支内部 |
| 4 | 查 Application 事件日志近 40 分钟崩溃记录 | 无 | setup 不是「启动即崩溃」，而是**根本没被创建** |
| 5 | 在 OpenSettings/About 分支加日志（打印路径、exists、spawn 结果）重编译复现 | `exists=true`，`spawn failed: 请求的操作需要提升。(os error 740)` | **锁定根因** |

> 关键教训：原代码 `spawn().ok()` / `let _ = spawn()` 把错误静默吞掉，导致问题无任何痕迹。已改为 `match` 并 `info!/error!` 记录路径与错误。

## 3. 根因

### 3.1 第一层：安装清单漏文件（已先暴露并修复）

`install.ps1` 的复制清单 `$files` 原本不含 `winxime-setup.exe`，安装目录里压根没有 setup 程序，`exists()` 为 false 时静默返回。
**修复**：`install.ps1` 复制清单加入 `winxime-setup.exe`。

### 3.2 第二层（关键）：UAC 安装程序检测 → os error 740

补齐文件后仍打不开，日志报 `ERROR_ELEVATION_REQUIRED (os error 740)`：

- `winxime-setup.exe` **没有内嵌任何 application manifest**（二进制中 asInvoker / requestedExecutionLevel / trustInfo 计数均为 0，build.rs 当时只用 winres 嵌了图标）。
- 文件名包含 **`setup`** 关键字，触发 Windows UAC 的 **Installer Detection（安装程序启发式检测）**：对「无 manifest 且文件名像安装器」的 exe，默认按 `requireAdministrator` 对待。
- server 是普通权限进程，Rust `Command::spawn` 底层走 `CreateProcessW`，对被判定需提权的 exe **直接失败返回 740，不会弹 UAC**。
- 而 PowerShell `Start-Process` 底层走 `ShellExecuteEx`，会自动弹 UAC 提权——这解释了步骤 1「手动能开、托盘点不开」的矛盾。

「设置 / 关于」是普通用户态功能，本不应需要管理员权限。

## 4. 修复内容

1. **新增** `crates/winxime-setup/winxime-setup.manifest`，显式声明 `requestedExecutionLevel level="asInvoker"`，关闭 Installer Detection 的误判：

   ```xml
   <requestedPrivileges>
     <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
   </requestedPrivileges>
   ```

2. **修改** `crates/winxime-setup/build.rs`，用 winres 把 manifest 嵌入 exe：

   ```rust
   res.set_manifest_file("winxime-setup.manifest");
   ```

3. **修改** `crates/winxime-server/src/main.rs`：OpenSettings/About 分支补诊断日志，spawn 结果用 `match` 显式记录，不再静默吞错。

4. **修改** `winxime/install.ps1`：复制清单补 `winxime-setup.exe`。

## 5. 验证

- 新 exe 二进制内 `asInvoker` 计数 = 1。
- 带日志重启 server 后模拟菜单命令：
  - 修复前：`OpenSettings: spawn failed: ... (os error 740)`
  - 修复后：`OpenSettings: spawned setup pid=64152`、`About: spawned setup pid=64192`，窗口标题「凑合 设置」，Responding=True。
- `just install` 全量重装后复测：安装目录 server 成功拉起安装目录 setup（窗口正常、响应正常）。

## 6. 通用经验

- **任何要被普通权限进程 `CreateProcess` 拉起的 exe，都应内嵌显式 `requestedExecutionLevel` 的 manifest**；否则一旦文件名命中 setup/install/update 等关键字，就会被 Installer Detection 要求提权而失败。
- 若确实要提权启动子进程，不能用 `Command::spawn`，需走 `ShellExecuteW` + `runas`（会弹 UAC）。
- 不要用 `.ok()` / `let _ =` 吞掉 `spawn` 的 `io::Error`，至少打日志，否则此类问题零线索。
