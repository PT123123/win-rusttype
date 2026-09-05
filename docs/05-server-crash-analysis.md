# 05-server-crash-analysis

## 问题

Server 进程在运行过程中 crash，无 panic 或 abort 日志输出。

## 日志分析

### server-stderr.log

启动时的非致命警告（不导致 crash）：

```
E20260905 10:41:12.720922 engine.cc:312] error creating translator: 'lua_translator'
E20260905 10:41:12.757435 dict_compiler.cc:174] neither pack source file '...user_simp.dict.yaml' nor a prebuilt table exists
E20260905 10:41:12.766278 engine.cc:312] error creating translator: 'lua_translator'
```

### server-stdout.log

进程在 `03:41:51` 正常处理 `ShowTrayIcon` IPC 请求后日志中断，无 panic/abort 输出。说明进程被异常终止。

## 原因分析

源码审查发现以下高风险 crash 点：

### 1. CandidateWindow 数据竞争（最高风险）

`ui.rs:1550-1551` 中 `CandidateWindow` 被标记为 `Send + Sync`，但内部包含 `RefCell`：

```rust
unsafe impl Send for CandidateWindow {}
unsafe impl Sync for CandidateWindow {}
```

IPC 线程调用 `window.update()` 和 `window.show()` 时，主线程可能正在 `wnd_proc` 的 `WM_PAINT` 中通过 `RefCell::borrow()` 访问同一数据。**RefCell 双借用会导致 runtime panic**。

### 2. IPC 安全描述符初始化（启动时 crash）

`ipc_server.rs:35`：

```rust
SecurityDescriptor::deserialize(...).expect("Failed to create security descriptor")
```

如果反序列化失败，IPC 线程直接 panic，server 对所有 TSF 客户端失联。

### 3. DllMain 中的 unwrap（DLL crash）

`dll.rs:108`：

```rust
let log_dir = log_path.parent().unwrap();
```

在 `DllMain`（DLL_PROCESS_ATTACH）中调用，若 panic 会直接终止宿主进程。

### 4. wnd_proc 原始指针解引用

`ui.rs` 的 `wnd_proc` 回调中通过 `GetWindowLongPtrW(hwnd, GWLP_USERDATA)` 获取原始指针。若指针过期或为 null，直接 crash。虽有 `is_null()` 检查，但无法防御 use-after-free。

## 非崩溃问题（stderr 中的警告）

| 问题 | 原因 | 影响 |
|------|------|------|
| `lua_translator` 创建失败 | librime 找不到 lua translator 插件 | 无法使用 lua 扩展输入方案 |
| `user_simp.dict.yaml` 不存在 | 词典文件缺失或未预编译 | 依赖该词典的功能不可用 |
| DllRegisterServer 失败 | 需要管理员权限写注册表 | TSF 注册不完整 |

## 下一步

- [ ] 修复 `CandidateWindow` 数据竞争：用 `Mutex` 替代 `RefCell`，或确保 `update`/`show` 只在主线程调用
- [ ] 将 `ipc_server.rs:35` 的 `.expect()` 改为优雅错误处理
- [ ] 将 `dll.rs:108` 的 `.unwrap()` 改为安全处理
- [ ] 复现 crash 并获取 backtrace（设置 `RUST_BACKTRACE=1`）
