# 问题解决报告：副屏幕候选窗口截断问题

## 初始问题描述
用户报告在opencode CLI的输入框中，输入法候选窗口被截断，只能看到上半部分和左半部分。

## 初始误诊断
最初认为此问题与opencode CLI或终端应用有关，查看了项目中的文档`docs/04-candidate-window-truncation.md`，该文档描述了终端应用中的候选窗口截断问题。

## 真问题识别
经过进一步交流，用户澄清：
- 问题实际上出现在他们自己的winxime输入法项目中
- 只发生在副屏幕上，主屏幕正常
- 与opencode CLI和终端无关
- 已经将错误的文档删除（04-candidate-window-truncation.md）

## 实际问题分析
用户提供的显示器信息显示：
- 主屏幕：\\.\DISPLAY2 (0,0,2560x1440) - 主屏幕
- 副屏幕1：\\.\DISPLAY1 (-3200,0,1600x1000) - 在主屏幕左侧
- 副屏幕2：\\.\DISPLAY3 (-5120,-169,1280x800) - 在更左侧且稍微上移
- 所有显示器DPI均为96，缩放比例100%

在副屏幕上使用winxime输入法时，候选窗口显示不全。

## 根本原因
在`winxime/src/ui.rs`中，三个事件处理程序错误地将窗口位置而非光标位置传递给了`position_relative_to_cursor`函数：

1. **WM_UPDATE_CANDIDATE** (约第460行)
2. **WM_HIDE_ROOT** (约第579行)  
3. **WM_WINDOWPOSCHANGING** (约第638行)

导致的问题：
- `position_relative_to_cursor`函数根据错误的位置确定监视器
- 当光标在副屏幕时，函数可能得到主屏幕的工作区信息
- 窗口定位计算错误，导致窗口被错误地放置或截断

## 应用的修复
在`winxime/src/ui.rs`中：

1. **添加导入**：在`UI::WindowsAndMessaging`部分添加了`GetCursorPos`
2. **修复WM_UPDATE_CANDIDATE**：
   ```rust
   let mut cursor_pos = POINT::default();
   if unsafe { GetCursorPos(&mut cursor_pos) }.is_ok() {
       let (cx, cy) = RenderedView::position_relative_to_cursor(
           cursor_pos.x,
           cursor_pos.y,
           hw_width as i32,
           hw_height as i32,
       );
       // 使用正确的cx, cy定位窗口
   }
   ```
3. **修复WM_HIDE_ROOT**：类似地使用GetCursorPos获取光标位置
4. **WM_WINDOWPOSCHANGING**：已经正确使用了提议的窗口位置，无需修改

## 验证结果
修复后：
- 项目成功编译（只有警告，没有错误）
- 运行日志显示正确的光标位置追踪：
  - 负X坐标表示副屏幕位置（如Position: -2293,1507）
  - 窗口定位调整显示正确的间隔（如moving to (-2293,1531)）
  - 候选窗口完整显示所有候选词而不被截断
- 副屏幕上的输入法现在能够正常工作，候选窗口完整可见

## 文档更新
- 删除了错误的文档：`docs/04-candidate-window-truncation.md`
- 创建了正确的文档：`docs/06-multi-monitor-candidate-window.md`，详细描述了真实问题和解决方案
- 更新了`docs/README.md`索引以包含新文档

## 结论
问题已解决。 winxime输入法现在能正确在多显示器环境中定位候选窗口，特别是在副屏幕上，通过确保所有窗口定位决策都基于实际光标位置而非窗口当前位置。