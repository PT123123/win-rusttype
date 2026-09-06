use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use windows::Win32::{
    Foundation::*,
    Graphics::{
        Direct2D::Common::{
            D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_COMPOSITE_MODE_SOURCE_OVER,
            D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_F,
        },
        Direct2D::{
            CLSID_D2D1GaussianBlur, D2D1CreateFactory, ID2D1BitmapRenderTarget, ID2D1DeviceContext,
            ID2D1Factory1, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
            D2D1_BITMAP_PROPERTIES1, D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
            D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
            D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT,
        },
        Direct3D::D3D_DRIVER_TYPE_WARP,
        Direct3D11::{D3D11CreateDevice, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION},
        DirectComposition::{DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget},
        DirectWrite::{
            DWriteCreateFactory, IDWriteFactory1, DWRITE_FACTORY_TYPE_SHARED,
            DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_METRICS,
        },
        Dxgi::Common::{
            DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
        },
        Dxgi::{
            IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1,
            DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            DXGI_USAGE_RENDER_TARGET_OUTPUT,
        },
        Gdi::{
            BeginPaint, EndPaint, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow,
            MONITORINFO, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT,
        },
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    UI::WindowsAndMessaging::{
        DefWindowProcW, GetWindowLongPtrW, LoadCursorW, PostMessageW,
        RegisterClassW, SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
        SWP_NOCOPYBITS,
        SW_HIDE, SW_SHOWNA, WM_DESTROY, WM_DPICHANGED, WM_NCCREATE, WM_PAINT,
        WM_USER, WM_WINDOWPOSCHANGING, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    },
};
use windows_core::{w, Interface, HSTRING, PCWSTR};
use windows_numerics::Vector2;

use crate::config::get_colors;
use xime_config::XimeConfig;
use winxime_ipc::Context;

pub const WM_SHOW_CANDIDATE: u32 = WM_USER + 1;
pub const WM_HIDE_CANDIDATE: u32 = WM_USER + 2;
pub const WM_UPDATE_CANDIDATE: u32 = WM_USER + 3;
pub const WM_SET_POSITION: u32 = WM_USER + 4;
pub const WM_SHOW_ROOT: u32 = WM_USER + 5;
pub const WM_HIDE_ROOT: u32 = WM_USER + 6;

const WINDOW_CLASS: &str = "WinximeCandidateWindow";
const ROW_SPACING: f32 = 4.0;
const COL_SPACING: f32 = 8.0;
const MARGIN: f32 = 6.0;
const MIN_WIDTH: f32 = 120.0;
const BLUR_RADIUS: f32 = 8.0;
const PAGE_STRIP_HEIGHT: f32 = 20.0;

#[derive(Debug, Clone)]
struct RenderedMetrics {
    width: f32,
    height: f32,
    hw_width: f32,
    hw_height: f32,
    item_height: f32,
    item_widths: Vec<f32>,
    selkey_widths: Vec<f32>,
    text_widths: Vec<f32>,
    comment_widths: Vec<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct CandidateModel {
    pub items: Vec<String>,
    pub comments: Vec<String>,
    pub selkeys: Vec<u16>,
    pub total_pages: u32,
    pub current_page: u32,
    pub font_family: HSTRING,
    pub font_size: f32,
    pub cand_per_row: u32,
    pub horizontal: bool,
    pub use_cursor: bool,
    pub current_sel: usize,
    pub selkey_color: D2D1_COLOR_F,
    pub fg_color: D2D1_COLOR_F,
    pub comment_color: D2D1_COLOR_F,
    pub bg_color: D2D1_COLOR_F,
    pub highlight_fg_color: D2D1_COLOR_F,
    pub highlight_bg_color: D2D1_COLOR_F,
    pub border_color: D2D1_COLOR_F,
}

#[derive(Debug, Clone, Default)]
pub struct RootModel {
    pub letter: char,
    pub root: String,
    pub font_family: HSTRING,
    pub font_size: f32,
    pub primary_color: D2D1_COLOR_F,
    pub bg_color: D2D1_COLOR_F,
    pub fg_color: D2D1_COLOR_F,
}

impl From<(char, String)> for RootModel {
    fn from((letter, root): (char, String)) -> Self {
        let config = XimeConfig::load();
        let font_family = if config.style.font_family.is_empty() {
            HSTRING::from("Microsoft YaHei UI")
        } else {
            HSTRING::from(config.style.font_family.as_str())
        };
        let (r, g, b) = config.get_primary_color();
        let color_u32 = (r as u32) << 16 | (g as u32) << 8 | b as u32;
        let (_, _, _, selkey_color, _, _, _) = get_colors(color_u32);

        Self {
            letter,
            root,
            font_family,
            font_size: config.style.font_size,
            primary_color: selkey_color,
            bg_color: D2D1_COLOR_F {
                r: 0.98,
                g: 0.98,
                b: 0.98,
                a: 1.0,
            },
            fg_color: D2D1_COLOR_F {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            },
        }
    }
}

impl From<&Context> for CandidateModel {
    fn from(ctx: &Context) -> Self {
        let config = XimeConfig::load();
        let font_family = if config.style.font_family.is_empty() {
            HSTRING::from("Microsoft YaHei UI")
        } else {
            HSTRING::from(config.style.font_family.as_str())
        };
        let (r8, g8, b8) = config.get_primary_color();
        let color_u32 = (r8 as u32) << 16 | (g8 as u32) << 8 | b8 as u32;
        let (
            bg_color,
            border_color,
            fg_color,
            selkey_color,
            comment_color,
            highlight_bg_color,
            highlight_fg_color,
        ) = get_colors(color_u32);

        let cand_per_row = if config.style.horizontal {
            config.style.candidate_count as u32
        } else {
            1
        };

        let max_cand = config.style.candidate_count as usize;
        let all_items: Vec<String> = ctx
            .candidates
            .candies
            .iter()
            .map(|c| c.str.clone())
            .collect();
        let all_comments: Vec<String> = ctx
            .candidates
            .comments
            .iter()
            .map(|c| c.str.clone())
            .collect();
        let n = all_items.len().min(max_cand);
        let items = all_items[..n].to_vec();
        let comments = all_comments[..n].to_vec();

        Self {
            items,
            comments,
            selkeys: {
                let mut keys = Vec::with_capacity(n);
                for i in 0..n {
                    if i < 9 {
                        keys.push('1' as u16 + i as u16);
                    } else if i == 9 {
                        keys.push('0' as u16);
                    } else {
                        keys.push('?' as u16);
                    }
                }
                keys
            },
            total_pages: ctx.candidates.total_pages,
            current_page: ctx.candidates.current_page + 1,
            current_sel: ctx.candidates.highlighted as usize,
            font_family: if config.style.font_family.is_empty() {
                HSTRING::from("Microsoft YaHei UI")
            } else {
                HSTRING::from(config.style.font_family.as_str())
            },
            font_size: config.style.font_size,
            cand_per_row,
            horizontal: config.style.horizontal,
            use_cursor: true,
            selkey_color,
            fg_color,
            comment_color,
            bg_color,
            highlight_fg_color,
            highlight_bg_color,
            border_color,
        }
    }
}

struct RenderedView {
    hwnd: HWND,
    _d2d_factory: ID2D1Factory1,
    dwrite_factory: IDWriteFactory1,
    d2d_context: ID2D1DeviceContext,
    swapchain: IDXGISwapChain1,
    _dcomp_target: IDCompositionTarget,
}

impl RenderedView {
    fn new(user_data: *const CandidateWindow) -> Result<Self, String> {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = HSTRING::from(WINDOW_CLASS);

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hInstance: HINSTANCE(hinstance.0),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);

            let hwnd = windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP,
                PCWSTR::from_raw(class_name.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                200,
                160,
                None,
                None,
                Some(HINSTANCE(hinstance.0)),
                Some(user_data.cast()),
            )
            .map_err(|e| format!("CreateWindowExW failed: {:?}", e))?;

            let dwrite_factory: IDWriteFactory1 =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                    .map_err(|e| format!("DWriteCreateFactory failed: {:?}", e))?;

            let mut device = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
            .map_err(|e| format!("D3D11CreateDevice failed: {:?}", e))?;
            let device = device.ok_or("D3D11 device is None")?;

            let dxgi_device: IDXGIDevice = device
                .cast()
                .map_err(|e| format!("IDXGIDevice cast failed: {:?}", e))?;
            let adapter = dxgi_device
                .GetAdapter()
                .map_err(|e| format!("GetAdapter failed: {:?}", e))?;
            let factory: IDXGIFactory2 = adapter
                .GetParent()
                .map_err(|e| format!("GetParent failed: {:?}", e))?;

            let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: 10,
                Height: 10,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                ..Default::default()
            };

            let swapchain = factory
                .CreateSwapChainForComposition(&device, &swapchain_desc, None)
                .map_err(|e| format!("CreateSwapChainForComposition failed: {:?}", e))?;

            let d2d_factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                    .map_err(|e| format!("D2D1CreateFactory failed: {:?}", e))?;
            let d2d_device = d2d_factory
                .CreateDevice(&dxgi_device)
                .map_err(|e| format!("CreateDevice failed: {:?}", e))?;
            let d2d_context = d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)
                .map_err(|e| format!("CreateDeviceContext failed: {:?}", e))?;

            Self::create_swapchain_bitmap(&swapchain, &d2d_context)?;

            let dcomp_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)
                .map_err(|e| format!("DCompositionCreateDevice failed: {:?}", e))?;
            let dcomp_target = dcomp_device
                .CreateTargetForHwnd(hwnd, true)
                .map_err(|e| format!("CreateTargetForHwnd failed: {:?}", e))?;
            let visual = dcomp_device
                .CreateVisual()
                .map_err(|e| format!("CreateVisual failed: {:?}", e))?;
            visual
                .SetContent(&swapchain)
                .map_err(|e| format!("SetContent failed: {:?}", e))?;
            dcomp_target
                .SetRoot(&visual)
                .map_err(|e| format!("SetRoot failed: {:?}", e))?;
            dcomp_device
                .Commit()
                .map_err(|e| format!("Commit failed: {:?}", e))?;

            Ok(Self {
                hwnd,
                _d2d_factory: d2d_factory,
                dwrite_factory,
                d2d_context,
                swapchain,
                _dcomp_target: dcomp_target,
            })
        }
    }

    unsafe fn create_swapchain_bitmap(
        swapchain: &IDXGISwapChain1,
        target: &ID2D1DeviceContext,
    ) -> Result<(), String> {
        let surface: windows::Win32::Graphics::Dxgi::IDXGISurface = swapchain
            .GetBuffer(0)
            .map_err(|e| format!("GetBuffer failed: {:?}", e))?;

        let bitmap_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };

        let bitmap = target
            .CreateBitmapFromDxgiSurface(&surface, Some(&bitmap_props))
            .map_err(|e| format!("CreateBitmapFromDxgiSurface failed: {:?}", e))?;
        target.SetTarget(&bitmap);
        Ok(())
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() {
                    let user_data = unsafe { (*cs).lpCreateParams };
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, user_data as isize) };
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_SHOW_CANDIDATE => {
                debug!("WM_SHOW_CANDIDATE received, hwnd={:?}", hwnd.0);
                let result = ShowWindow(hwnd, SW_SHOWNA);
                debug!("ShowWindow(SW_SHOWNA) result: {:?}", result);
                LRESULT(0)
            }
            WM_HIDE_CANDIDATE => {
                debug!("WM_HIDE_CANDIDATE received");
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            WM_UPDATE_CANDIDATE => {
                debug!("WM_UPDATE_CANDIDATE received");
                let ctx_ptr = wparam.0 as *mut Context;
                if !ctx_ptr.is_null() {
                    let ctx = Box::from_raw(ctx_ptr);
                    debug!("ctx.candidates: {} items", ctx.candidates.candies.len());
                    let model = CandidateModel::from(&*ctx);
                    info!("  model.items: {:?}", model.items);

                    let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateWindow;
                    if !this_ptr.is_null() {
                        *(*this_ptr).model.lock().unwrap_or_else(|e| e.into_inner()) = model.clone();

                        // 以 TSF 插入点所在屏为唯一基准，一次完成定位+定尺寸，DPI 同帧一致
                        let anchor = RenderedView::current_anchor(this_ptr);
                        if let Some((dpi, metrics)) =
                            RenderedView::apply_layout(this_ptr, anchor, None)
                        {
                            info!(
                                "  layout dpi={}, hw={}x{}",
                                dpi, metrics.hw_width, metrics.hw_height
                            );
                            if !model.items.is_empty() {
                                let view_guard = (*this_ptr)
                                    .view
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner());
                                if let Some(view) = view_guard.as_ref() {
                                    let _ = view.on_paint_with_metrics(&model, dpi, &metrics);
                                }
                            }
                        }
                    }
                    info!("  update complete");
                }
                LRESULT(0)
            }
            WM_SET_POSITION => {
                let x = wparam.0 as i32;
                let y = lparam.0 as i32;
                let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateWindow;
                if !this_ptr.is_null() {
                    let anchor = POINT { x, y };
                    *(*this_ptr)
                        .last_anchor
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = anchor;
                    // 跨屏移动时按目标屏 DPI 同步重定尺寸并重绘，避免尺寸停留在旧屏
                    if let Some((dpi, metrics)) =
                        RenderedView::apply_layout(this_ptr, anchor, None)
                    {
                        RenderedView::repaint_current(this_ptr, dpi, &metrics);
                    }
                }
                LRESULT(0)
            }
            WM_SHOW_ROOT => {
                debug!("WM_SHOW_ROOT received");
                let root_ptr = wparam.0 as *mut RootModel;
                if !root_ptr.is_null() {
                    let root = unsafe { Box::from_raw(root_ptr) };
                    debug!("showing root for '{}': {}", root.letter, root.root);

                    let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateWindow;
                    if !this_ptr.is_null() {
                        *(*this_ptr).root_model.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(*root.clone());

                        let anchor = RenderedView::current_anchor(this_ptr);
                        if let Some((dpi, metrics)) =
                            RenderedView::apply_layout(this_ptr, anchor, None)
                        {
                            let view_guard = (*this_ptr)
                                .view
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            if let Some(view) = view_guard.as_ref() {
                                let _ = view.on_paint_root(&root, dpi, &metrics);
                            }
                        }
                    }
                }
                let _ = ShowWindow(hwnd, SW_SHOWNA);
                LRESULT(0)
            }
            WM_HIDE_ROOT => {
                debug!("WM_HIDE_ROOT received");
                let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateWindow;
                if !this_ptr.is_null() {
                    *(*this_ptr).root_model.lock().unwrap_or_else(|e| e.into_inner()) = None;

                    // 切回候选内容：统一按插入点所在屏 DPI 重定几何，若非空则重绘
                    let anchor = RenderedView::current_anchor(this_ptr);
                    if let Some((dpi, metrics)) =
                        RenderedView::apply_layout(this_ptr, anchor, None)
                    {
                        RenderedView::repaint_current(this_ptr, dpi, &metrics);
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                info!("WM_PAINT received");
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);

                let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateWindow;
                if !this_ptr.is_null() {
                    let model = (*this_ptr).model.lock().unwrap_or_else(|e| e.into_inner());
                    info!("  painting {} items", model.items.len());
                    if !model.items.is_empty() {
                        let view = (*this_ptr).view.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(view) = view.as_ref() {
                            let _ = view.on_paint(&model);
                        }
                    }
                }

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_WINDOWPOSCHANGING => {
                // 窗口几何统一由 apply_layout 在各命令中一次性给出（位置+尺寸同 DPI）。
                // 这里不再依据“窗口当前位置”的 DPI 改写 cx/cy/x/y：旧实现不判 flags，
                // 在跨屏 SetWindowPos 的中间态会用旧屏 DPI 把尺寸锁小，导致副屏只显示左上部分。
                LRESULT(0)
            }
            WM_DPICHANGED => {
                // 进程为 PER_MONITOR_AWARE_V2：跨 DPI 屏时自行接管，绝不交给
                // DefWindowProc 做默认二次缩放，以免与 apply_layout 互相覆盖。
                let new_dpi = (wparam.0 & 0xFFFF) as f32;
                let suggested = &*(lparam.0 as *const RECT);
                info!(
                    "WM_DPICHANGED dpi={} suggested=({},{},{},{})",
                    new_dpi, suggested.left, suggested.top, suggested.right, suggested.bottom
                );
                let this_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const CandidateWindow;
                if !this_ptr.is_null() {
                    let anchor = POINT {
                        x: suggested.left,
                        y: suggested.top,
                    };
                    *(*this_ptr)
                        .last_anchor
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = anchor;
                    if let Some((dpi, metrics)) =
                        RenderedView::apply_layout(this_ptr, anchor, Some(new_dpi))
                    {
                        RenderedView::repaint_current(this_ptr, dpi, &metrics);
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    fn get_dpi_for_window(hwnd: HWND) -> f32 {
        unsafe {
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            dpi_x as f32
        }
    }

    fn get_dpi_for_point(point: POINT) -> f32 {
        unsafe {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            let mut dpi_x = 96u32;
            let mut dpi_y = 96u32;
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            dpi_x as f32
        }
    }

    // ---- 多显示器 / 逐屏 DPI 的统一布局入口 ----
    // 以 TSF 插入点所在显示器为唯一基准：同一处取 DPI、算尺寸、定位置，
    // 并用一次 SetWindowPos 同时给出位置与尺寸，保证 HWND 几何与交换链
    // 像素尺寸同帧、同一 DPI，从根上消除跨屏时尺寸被旧屏 DPI 锁死的问题。

    unsafe fn current_anchor(this: *const CandidateWindow) -> POINT {
        if let Some(w) = this.as_ref() {
            if let Ok(anchor) = w.last_anchor.lock() {
                return *anchor;
            }
        }
        POINT::default()
    }

    unsafe fn apply_layout(
        this: *const CandidateWindow,
        anchor: POINT,
        dpi_hint: Option<f32>,
    ) -> Option<(f32, RenderedMetrics)> {
        let w = this.as_ref()?;
        let dpi = dpi_hint.unwrap_or_else(|| Self::get_dpi_for_point(anchor));
        // 先克隆状态，避免持锁期间 SetWindowPos 重入 wnd_proc
        let root = w.root_model.lock().ok().and_then(|g| g.clone());

        let (hwnd, metrics, x, y) = {
            let model = w.model.lock().ok()?;
            let view_guard = w.view.lock().ok()?;
            let view = view_guard.as_ref()?;
            let metrics = match root.as_ref() {
                Some(r) => view.calculate_root_rect(r, dpi).ok()?,
                None => view.calculate_client_rect(&model, dpi).ok()?,
            };
            let (x, y) = Self::position_relative_to_cursor(
                anchor.x,
                anchor.y,
                metrics.hw_width as i32,
                metrics.hw_height as i32,
            );
            (view.hwnd, metrics, x, y)
        };

        // SWP_ASYNCWINDOWPOS: IPC 线程持引擎锁调用本函数时，定位改为投递给属主线程（主线程）
        // 异步执行，避免同步等待主线程导致持锁线程被卡死；主线程自身调用时该标志是 no-op。
        let swp_begin = std::time::Instant::now();
        debug!(
            "apply_layout: SetWindowPos begin hwnd={:?} at=({},{}) size={}x{} dpi={} thread={:?}",
            hwnd.0,
            x,
            y,
            metrics.hw_width as i32,
            metrics.hw_height as i32,
            dpi,
            std::thread::current().id()
        );
        let swp_result = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            metrics.hw_width as i32,
            metrics.hw_height as i32,
            SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_ASYNCWINDOWPOS,
        );
        debug!(
            "apply_layout: SetWindowPos done in {:?} result={:?}",
            swp_begin.elapsed(),
            swp_result.ok()
        );
        info!(
            "apply_layout: dpi={}, hw={}x{}, at=({},{})",
            dpi, metrics.hw_width, metrics.hw_height, x, y
        );
        Some((dpi, metrics))
    }

    unsafe fn repaint_current(
        this: *const CandidateWindow,
        dpi: f32,
        metrics: &RenderedMetrics,
    ) {
        let w = match this.as_ref() {
            Some(w) => w,
            None => return,
        };
        let root = w.root_model.lock().ok().and_then(|g| g.clone());
        let view_guard = match w.view.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let view = match view_guard.as_ref() {
            Some(v) => v,
            None => return,
        };
        match root.as_ref() {
            Some(r) => {
                let _ = view.on_paint_root(r, dpi, metrics);
            }
            None => {
                if let Ok(model) = w.model.lock() {
                    if !model.items.is_empty() {
                        let _ = view.on_paint_with_metrics(&model, dpi, metrics);
                    }
                }
            }
        }
    }

    fn position_relative_to_cursor(cx: i32, cy: i32, w: i32, h: i32) -> (i32, i32) {
        unsafe {
            let monitor = MonitorFromPoint(POINT { x: cx, y: cy }, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };

            if GetMonitorInfoW(monitor, &mut mi).as_bool() {
                let rc = mi.rcWork;
                let gap = 24;
                let space_below = rc.bottom - cy - gap;
                let final_y = if space_below >= h {
                    cy + gap
                } else {
                    cy - h - gap
                };
                (
                    cx.clamp(rc.left, rc.right - w),
                    final_y.clamp(rc.top, rc.bottom - h),
                )
            } else {
                (cx, cy + 24)
            }
        }
    }

    fn calculate_client_rect(
        &self,
        model: &CandidateModel,
        dpi: f32,
    ) -> Result<RenderedMetrics, String> {
        unsafe {
            let scale = dpi / 96.0;

            let text_format = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat failed: {:?}", e))?;

            let mut item_height = 0.0f32;
            let mut item_widths = Vec::new();
            let mut selkey_widths = Vec::new();
            let mut text_widths = Vec::new();
            let mut comment_widths = Vec::new();
            let mut selkey_buf = "?.".encode_utf16().collect::<Vec<_>>();

            for (i, item) in model.items.iter().enumerate() {
                let selkey = model.selkeys.get(i).copied().unwrap_or('?' as u16);
                selkey_buf[0] = selkey;

                let mut selkey_metrics = DWRITE_TEXT_METRICS::default();
                let mut item_metrics = DWRITE_TEXT_METRICS::default();
                let mut comment_metrics = DWRITE_TEXT_METRICS::default();

                self.dwrite_factory
                    .CreateTextLayout(&selkey_buf, &text_format, f32::MAX, f32::MAX)
                    .map_err(|e| format!("CreateTextLayout for selkey failed: {:?}", e))?
                    .GetMetrics(&mut selkey_metrics)
                    .map_err(|e| format!("GetMetrics for selkey failed: {:?}", e))?;

                let item_hstring = HSTRING::from(item);
                self.dwrite_factory
                    .CreateTextLayout(&item_hstring, &text_format, f32::MAX, f32::MAX)
                    .map_err(|e| format!("CreateTextLayout for item failed: {:?}", e))?
                    .GetMetrics(&mut item_metrics)
                    .map_err(|e| format!("GetMetrics for item failed: {:?}", e))?;

                let comment = model.comments.get(i).cloned().unwrap_or_default();
                let comment_hstring = HSTRING::from(&comment);
                self.dwrite_factory
                    .CreateTextLayout(&comment_hstring, &text_format, f32::MAX, f32::MAX)
                    .map_err(|e| format!("CreateTextLayout for comment failed: {:?}", e))?
                    .GetMetrics(&mut comment_metrics)
                    .map_err(|e| format!("GetMetrics for comment failed: {:?}", e))?;

                let padding_x = 6.0;
                let padding_y = 4.0;
                let selkey_width = selkey_metrics.widthIncludingTrailingWhitespace;
                let text_width = item_metrics.widthIncludingTrailingWhitespace;
                let comment_width = if comment.is_empty() {
                    0.0
                } else {
                    comment_metrics.widthIncludingTrailingWhitespace + 4.0
                };
                selkey_widths.push(selkey_width);
                text_widths.push(text_width);
                comment_widths.push(comment_width);

                let item_width = selkey_width + text_width + comment_width + 2.0 * padding_x;
                item_widths.push(item_width);
                item_height = item_height
                    .max(item_metrics.height + 2.0 * padding_y)
                    .max(selkey_metrics.height + 2.0 * padding_y);
            }

            let items_len = model.items.len() as f32;
            if items_len == 0.0 {
                return Ok(RenderedMetrics {
                    width: 100.0,
                    height: 30.0,
                    hw_width: ((100.0 + BLUR_RADIUS * 2.0) * scale).ceil(),
                    hw_height: ((30.0 + BLUR_RADIUS * 2.0) * scale).ceil(),
                    item_height: 20.0,
                    item_widths: Vec::new(),
                    selkey_widths: Vec::new(),
                    text_widths: Vec::new(),
                    comment_widths: Vec::new(),
                });
            }

            let cand_per_row = model.cand_per_row as usize;

            let mut max_row_width = MIN_WIDTH;
            for row_start in (0..model.items.len()).step_by(cand_per_row) {
                let row_end = std::cmp::min(row_start + cand_per_row, model.items.len());
                let row_width: f32 = item_widths[row_start..row_end].iter().copied().sum::<f32>()
                    + (row_end - row_start - 1) as f32 * COL_SPACING
                    + 2.0 * MARGIN;
                max_row_width = max_row_width.max(row_width);
            }

            let rows = (items_len / cand_per_row as f32).ceil().max(1.0);
            let mut height = rows * item_height + (rows - 1.0) * ROW_SPACING + 2.0 * MARGIN;
            if model.total_pages > 1 {
                height += PAGE_STRIP_HEIGHT;
            }

            let hw_width = ((max_row_width + BLUR_RADIUS * 2.0) * scale).ceil();
            let hw_height = ((height + BLUR_RADIUS * 2.0) * scale).ceil();

            Ok(RenderedMetrics {
                width: max_row_width,
                height,
                hw_width,
                hw_height,
                item_height,
                item_widths,
                selkey_widths,
                text_widths,
                comment_widths,
            })
        }
    }

    fn calculate_root_rect(&self, model: &RootModel, dpi: f32) -> Result<RenderedMetrics, String> {
        unsafe {
            let scale = dpi / 96.0;

            let text_format = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat failed: {:?}", e))?;

            let letter_buf = [model.letter as u16];

            let mut letter_metrics = DWRITE_TEXT_METRICS::default();
            self.dwrite_factory
                .CreateTextLayout(&letter_buf, &text_format, f32::MAX, f32::MAX)
                .map_err(|e| format!("CreateTextLayout for letter failed: {:?}", e))?
                .GetMetrics(&mut letter_metrics)
                .map_err(|e| format!("GetMetrics for letter failed: {:?}", e))?;

            let root_hstring = HSTRING::from(&model.root);
            let mut root_metrics = DWRITE_TEXT_METRICS::default();
            self.dwrite_factory
                .CreateTextLayout(&root_hstring, &text_format, f32::MAX, f32::MAX)
                .map_err(|e| format!("CreateTextLayout for root failed: {:?}", e))?
                .GetMetrics(&mut root_metrics)
                .map_err(|e| format!("GetMetrics for root failed: {:?}", e))?;

            let letter_width = letter_metrics.widthIncludingTrailingWhitespace;
            let root_width = root_metrics.widthIncludingTrailingWhitespace;
            let text_height = model.font_size;

            let key_bg_width = letter_width + 16.0;
            let key_bg_height = 24.0;
            let padding = 12.0;

            let width = (padding + key_bg_width + 8.0 + root_width + padding).max(80.0);
            let height = (key_bg_height + padding).max(36.0);

            let hw_width = ((width + BLUR_RADIUS * 2.0) * scale).ceil();
            let hw_height = ((height + BLUR_RADIUS * 2.0) * scale).ceil();

            Ok(RenderedMetrics {
                width,
                height,
                hw_width,
                hw_height,
                item_height: text_height,
                item_widths: vec![root_width],
                selkey_widths: vec![letter_width],
                text_widths: vec![root_width],
                comment_widths: vec![],
            })
        }
    }

    fn on_paint_root(
        &self,
        model: &RootModel,
        dpi: f32,
        metrics: &RenderedMetrics,
    ) -> Result<(), String> {
        unsafe {
            self.d2d_context.SetTarget(None);
            self.swapchain
                .ResizeBuffers(
                    0,
                    metrics.hw_width as u32,
                    metrics.hw_height as u32,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .map_err(|e| format!("ResizeBuffers failed: {:?}", e))?;

            self.d2d_context.SetDpi(dpi, dpi);
            Self::create_swapchain_bitmap(&self.swapchain, &self.d2d_context)?;

            let text_format = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat failed: {:?}", e))?;
            let _ = text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

            let text_format_centered = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat centered failed: {:?}", e))?;
            let _ = text_format_centered.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            let _ = text_format_centered.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);

            self.d2d_context.BeginDraw();

            let blur_radius = BLUR_RADIUS;
            let corner_radius = 8.0;
            let key_bg_corner_radius = 4.0;

            let bg_brush = self
                .d2d_context
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.98,
                        g: 0.98,
                        b: 0.98,
                        a: 1.0,
                    },
                    None,
                )
                .map_err(|e| format!("CreateSolidColorBrush bg failed: {:?}", e))?;

            let border_brush = self
                .d2d_context
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.88,
                        g: 0.88,
                        b: 0.88,
                        a: 1.0,
                    },
                    None,
                )
                .map_err(|e| format!("CreateSolidColorBrush border failed: {:?}", e))?;

            let key_bg_brush = self
                .d2d_context
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: model.primary_color.r,
                        g: model.primary_color.g,
                        b: model.primary_color.b,
                        a: 0.19,
                    },
                    None,
                )
                .map_err(|e| format!("CreateSolidColorBrush key_bg failed: {:?}", e))?;

            let key_border_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.primary_color, None)
                .map_err(|e| format!("CreateSolidColorBrush key_border failed: {:?}", e))?;

            let key_text_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.primary_color, None)
                .map_err(|e| format!("CreateSolidColorBrush key_text failed: {:?}", e))?;

            let root_text_brush = self
                .d2d_context
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.2,
                        g: 0.2,
                        b: 0.2,
                        a: 1.0,
                    },
                    None,
                )
                .map_err(|e| format!("CreateSolidColorBrush root_text failed: {:?}", e))?;

            let shadow_render_target: ID2D1BitmapRenderTarget = self
                .d2d_context
                .CreateCompatibleRenderTarget(
                    Some(&D2D_SIZE_F {
                        width: metrics.width + blur_radius * 2.0,
                        height: metrics.height + blur_radius * 2.0,
                    }),
                    None,
                    None,
                    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                )
                .map_err(|e| format!("CreateCompatibleRenderTarget failed: {:?}", e))?;

            shadow_render_target.BeginDraw();
            shadow_render_target.Clear(None);

            let shadow_brush = shadow_render_target
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.15,
                    },
                    None,
                )
                .map_err(|e| format!("CreateSolidColorBrush shadow failed: {:?}", e))?;

            let shadow_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: blur_radius,
                    top: blur_radius,
                    right: metrics.width + blur_radius,
                    bottom: metrics.height + blur_radius,
                },
                radiusX: corner_radius,
                radiusY: corner_radius,
            };
            shadow_render_target.FillRoundedRectangle(&shadow_rect, &shadow_brush);
            shadow_render_target
                .EndDraw(None, None)
                .map_err(|e| format!("shadow EndDraw failed: {:?}", e))?;

            let shadow_bitmap = shadow_render_target
                .GetBitmap()
                .map_err(|e| format!("GetBitmap failed: {:?}", e))?;

            let gaussian_blur_effect = self
                .d2d_context
                .CreateEffect(&CLSID_D2D1GaussianBlur)
                .map_err(|e| format!("CreateEffect failed: {:?}", e))?;
            gaussian_blur_effect.SetInput(0, &shadow_bitmap, false);
            let blur_output = gaussian_blur_effect
                .GetOutput()
                .map_err(|e| format!("GetOutput failed: {:?}", e))?;

            self.d2d_context.DrawImage(
                &blur_output,
                Some(&Vector2 { X: 0.0, Y: 0.0 }),
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );

            let bg_rounded_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: blur_radius,
                    top: blur_radius,
                    right: metrics.width + blur_radius,
                    bottom: metrics.height + blur_radius,
                },
                radiusX: corner_radius,
                radiusY: corner_radius,
            };
            self.d2d_context
                .FillRoundedRectangle(&bg_rounded_rect, &bg_brush);
            self.d2d_context
                .DrawRoundedRectangle(&bg_rounded_rect, &border_brush, 2.0, None);

            let letter_width = metrics.selkey_widths.get(0).copied().unwrap_or(20.0);
            let key_bg_width = letter_width + 16.0;
            let key_bg_height = 24.0;
            let x_start = blur_radius + 12.0;
            let y_center = blur_radius + (metrics.height - key_bg_height) / 2.0;

            let key_bg_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x_start,
                    top: y_center,
                    right: x_start + key_bg_width,
                    bottom: y_center + key_bg_height,
                },
                radiusX: key_bg_corner_radius,
                radiusY: key_bg_corner_radius,
            };
            self.d2d_context
                .FillRoundedRectangle(&key_bg_rect, &key_bg_brush);
            self.d2d_context
                .DrawRoundedRectangle(&key_bg_rect, &key_border_brush, 1.5, None);

            let letter_rect = D2D_RECT_F {
                left: x_start,
                top: y_center,
                right: x_start + key_bg_width,
                bottom: y_center + key_bg_height,
            };

            let letter_buf = [model.letter as u16];
            self.d2d_context.DrawText(
                &letter_buf,
                &text_format_centered,
                &letter_rect,
                &key_text_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            let root_hstring = HSTRING::from(&model.root);
            let root_x_start = x_start + key_bg_width + 8.0;
            let root_rect = D2D_RECT_F {
                left: root_x_start,
                top: blur_radius + (metrics.height - key_bg_height) / 2.0,
                right: metrics.width + blur_radius - 12.0,
                bottom: blur_radius + (metrics.height + key_bg_height) / 2.0,
            };
            self.d2d_context.DrawText(
                &root_hstring,
                &text_format,
                &root_rect,
                &root_text_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            self.d2d_context
                .EndDraw(None, None)
                .map_err(|e| format!("EndDraw failed: {:?}", e))?;
            let _ = self.swapchain.Present(1, DXGI_PRESENT(0)).ok();

            Ok(())
        }
    }

    fn on_paint(&self, model: &CandidateModel) -> Result<(), String> {
        let dpi = Self::get_dpi_for_window(self.hwnd);
        let metrics = self.calculate_client_rect(model, dpi)?;
        self.on_paint_with_metrics(model, dpi, &metrics)
    }

    fn on_paint_with_metrics(
        &self,
        model: &CandidateModel,
        dpi: f32,
        metrics: &RenderedMetrics,
    ) -> Result<(), String> {
        unsafe {
            info!(
                "on_paint: dpi={}, width={}, height={}, hw_width={}, hw_height={}",
                dpi, metrics.width, metrics.height, metrics.hw_width, metrics.hw_height
            );
            info!("on_paint: item_widths={:?}", metrics.item_widths);

            self.d2d_context.SetTarget(None);
            self.swapchain
                .ResizeBuffers(
                    0,
                    metrics.hw_width as u32,
                    metrics.hw_height as u32,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .map_err(|e| format!("ResizeBuffers failed: {:?}", e))?;

            self.d2d_context.SetDpi(dpi, dpi);
            Self::create_swapchain_bitmap(&self.swapchain, &self.d2d_context)?;

            let text_format = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat failed: {:?}", e))?;

            let selected_text_format = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat selected failed: {:?}", e))?;

            let page_text_format = self
                .dwrite_factory
                .CreateTextFormat(
                    &model.font_family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    model.font_size * 0.72,
                    w!("zh-CN"),
                )
                .map_err(|e| format!("CreateTextFormat page failed: {:?}", e))?;
            page_text_format
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)
                .map_err(|e| format!("SetTextAlignment page failed: {:?}", e))?;
            page_text_format
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
                .map_err(|e| format!("SetParagraphAlignment page failed: {:?}", e))?;

            self.d2d_context.BeginDraw();

            let blur_radius = BLUR_RADIUS;
            let corner_radius = 10.0;

            let bg_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.bg_color, None)
                .map_err(|e| format!("CreateSolidColorBrush bg failed: {:?}", e))?;

            let border_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.border_color, None)
                .map_err(|e| format!("CreateSolidColorBrush border failed: {:?}", e))?;

            let selkey_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.selkey_color, None)
                .map_err(|e| format!("CreateSolidColorBrush selkey failed: {:?}", e))?;

            let text_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.fg_color, None)
                .map_err(|e| format!("CreateSolidColorBrush text failed: {:?}", e))?;

            let highlight_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.highlight_bg_color, None)
                .map_err(|e| format!("CreateSolidColorBrush highlight failed: {:?}", e))?;

            let selected_text_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.highlight_fg_color, None)
                .map_err(|e| format!("CreateSolidColorBrush selected_text failed: {:?}", e))?;

            let shadow_render_target: ID2D1BitmapRenderTarget = self
                .d2d_context
                .CreateCompatibleRenderTarget(
                    Some(&D2D_SIZE_F {
                        width: metrics.width + blur_radius * 2.0,
                        height: metrics.height + blur_radius * 2.0,
                    }),
                    None,
                    None,
                    D2D1_COMPATIBLE_RENDER_TARGET_OPTIONS_NONE,
                )
                .map_err(|e| format!("CreateCompatibleRenderTarget failed: {:?}", e))?;

            shadow_render_target.BeginDraw();
            shadow_render_target.Clear(None);

            let shadow_brush = shadow_render_target
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.15,
                    },
                    None,
                )
                .map_err(|e| format!("CreateSolidColorBrush shadow failed: {:?}", e))?;

            let shadow_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: blur_radius,
                    top: blur_radius,
                    right: metrics.width + blur_radius,
                    bottom: metrics.height + blur_radius,
                },
                radiusX: corner_radius,
                radiusY: corner_radius,
            };
            shadow_render_target.FillRoundedRectangle(&shadow_rect, &shadow_brush);
            shadow_render_target
                .EndDraw(None, None)
                .map_err(|e| format!("shadow EndDraw failed: {:?}", e))?;

            let shadow_bitmap = shadow_render_target
                .GetBitmap()
                .map_err(|e| format!("GetBitmap failed: {:?}", e))?;

            let gaussian_blur_effect = self
                .d2d_context
                .CreateEffect(&CLSID_D2D1GaussianBlur)
                .map_err(|e| format!("CreateEffect failed: {:?}", e))?;
            gaussian_blur_effect.SetInput(0, &shadow_bitmap, false);
            let blur_output = gaussian_blur_effect
                .GetOutput()
                .map_err(|e| format!("GetOutput failed: {:?}", e))?;

            self.d2d_context.DrawImage(
                &blur_output,
                Some(&Vector2 { X: 0.0, Y: 0.0 }),
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );

            let bg_rounded_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: blur_radius,
                    top: blur_radius,
                    right: metrics.width + blur_radius,
                    bottom: metrics.height + blur_radius,
                },
                radiusX: corner_radius,
                radiusY: corner_radius,
            };
            self.d2d_context
                .FillRoundedRectangle(&bg_rounded_rect, &bg_brush);

            let border_rounded_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: blur_radius + 0.5,
                    top: blur_radius + 0.5,
                    right: metrics.width + blur_radius - 0.5,
                    bottom: metrics.height + blur_radius - 0.5,
                },
                radiusX: corner_radius,
                radiusY: corner_radius,
            };
            self.d2d_context
                .DrawRoundedRectangle(&border_rounded_rect, &border_brush, 0.5, None);

            let comment_brush = self
                .d2d_context
                .CreateSolidColorBrush(&model.comment_color, None)
                .map_err(|e| format!("CreateSolidColorBrush comment failed: {:?}", e))?;

            let mut col = 0usize;
            let mut x = MARGIN + blur_radius;
            let mut y = MARGIN + blur_radius;
            let padding_x = 6.0;
            let padding_y = 4.0;

            for (i, item) in model.items.iter().enumerate() {
                let selkey = model.selkeys.get(i).copied().unwrap_or('?' as u16);
                let mut selkey_buf = [0u16; 3];
                selkey_buf[0] = selkey;
                selkey_buf[1] = '.' as u16;

                let item_width = metrics.item_widths.get(i).copied().unwrap_or(60.0);
                let selkey_width = metrics.selkey_widths.get(i).copied().unwrap_or(20.0);
                let text_width = metrics.text_widths.get(i).copied().unwrap_or(40.0);
                let comment_width = metrics.comment_widths.get(i).copied().unwrap_or(0.0);

                info!(
                    "  item {}: x={}, item_width={}, text='{}'",
                    i, x, item_width, item
                );
                info!(
                    "  bg_rect: left={}, right={}",
                    blur_radius,
                    metrics.width + blur_radius
                );
                info!("  item_right={}", x + item_width);

                let selkey_rect = D2D_RECT_F {
                    left: x + padding_x,
                    top: y + padding_y,
                    right: x + selkey_width + padding_x,
                    bottom: y + metrics.item_height - padding_y,
                };

                let text_rect = D2D_RECT_F {
                    left: x + selkey_width + padding_x,
                    top: y + padding_y,
                    right: x + selkey_width + text_width + padding_x,
                    bottom: y + metrics.item_height - padding_y,
                };

                let comment_rect = D2D_RECT_F {
                    left: x + selkey_width + text_width + padding_x + 4.0,
                    top: y + padding_y,
                    right: x + item_width - padding_x,
                    bottom: y + metrics.item_height - padding_y,
                };

                let item_hstring = HSTRING::from(item);
                let comment = model.comments.get(i).cloned().unwrap_or_default();
                let comment_hstring = HSTRING::from(&comment);

                if model.use_cursor && i == model.current_sel {
                    let highlight_rounded_rect = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: x,
                            top: y,
                            right: x + item_width,
                            bottom: y + metrics.item_height,
                        },
                        radiusX: 8.0,
                        radiusY: 8.0,
                    };
                    self.d2d_context
                        .FillRoundedRectangle(&highlight_rounded_rect, &highlight_brush);

                    self.d2d_context.DrawText(
                        &selkey_buf[..2],
                        &selected_text_format,
                        &selkey_rect,
                        &selected_text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    self.d2d_context.DrawText(
                        &item_hstring,
                        &selected_text_format,
                        &text_rect,
                        &selected_text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    if !comment.is_empty() {
                        self.d2d_context.DrawText(
                            &comment_hstring,
                            &selected_text_format,
                            &comment_rect,
                            &selected_text_brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                } else {
                    self.d2d_context.DrawText(
                        &selkey_buf[..2],
                        &text_format,
                        &selkey_rect,
                        &selkey_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    self.d2d_context.DrawText(
                        &item_hstring,
                        &text_format,
                        &text_rect,
                        &text_brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    if !comment.is_empty() {
                        self.d2d_context.DrawText(
                            &comment_hstring,
                            &text_format,
                            &comment_rect,
                            &comment_brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }

                col += 1;
                if col >= model.cand_per_row as usize {
                    col = 0;
                    x = MARGIN + blur_radius;
                    y += metrics.item_height + ROW_SPACING;
                } else {
                    x += item_width + COL_SPACING;
                }
            }

            if model.total_pages > 1 {
                let page_text = format!("{}/{}", model.current_page, model.total_pages);
                let page_hstring = HSTRING::from(&page_text);
                let page_y = MARGIN + blur_radius + (metrics.height - PAGE_STRIP_HEIGHT) + 3.0;
                let page_rect = D2D_RECT_F {
                    left: blur_radius,
                    top: page_y,
                    right: metrics.width + blur_radius,
                    bottom: page_y + PAGE_STRIP_HEIGHT,
                };
                self.d2d_context.DrawText(
                    &page_hstring,
                    &page_text_format,
                    &page_rect,
                    &comment_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }

            self.d2d_context
                .EndDraw(None, None)
                .map_err(|e| format!("EndDraw failed: {:?}", e))?;

            let _ = self.swapchain.Present(1, DXGI_PRESENT(0)).ok();
        }

        Ok(())
    }
}

pub struct CandidateWindow {
    model: Mutex<CandidateModel>,
    root_model: Mutex<Option<RootModel>>,
    view: Mutex<Option<RenderedView>>,
    // 最近一次 TSF 插入点屏幕坐标（物理像素），作为跨屏布局/重算的统一锚点
    last_anchor: Mutex<POINT>,
}

unsafe impl Send for CandidateWindow {}
unsafe impl Sync for CandidateWindow {}

impl CandidateWindow {
    pub fn new() -> Arc<Self> {
        let window = Arc::new(Self {
            model: Mutex::new(CandidateModel::default()),
            root_model: Mutex::new(None),
            view: Mutex::new(None),
            last_anchor: Mutex::new(POINT::default()),
        });

        // Initialize UI immediately in the thread that will run message loop
        window.ensure_view_initialized();

        window
    }

    fn ensure_view_initialized(&self) {
        if self.view.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            let user_data_ptr = self as *const Self;
            match RenderedView::new(user_data_ptr.cast()) {
                Ok(view) => {
                    info!("UI initialized successfully");
                    *self.view.lock().unwrap_or_else(|e| e.into_inner()) = Some(view);
                }
                Err(e) => {
                    info!("Failed to initialize UI: {}", e);
                }
            }
        }
    }

    pub fn show(&self, x: i32, y: i32) {
        self.ensure_view_initialized();

        let anchor = POINT { x, y };
        *self
            .last_anchor
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = anchor;

        let hwnd = {
            let view_guard = self.view.lock().unwrap_or_else(|e| e.into_inner());
            match view_guard.as_ref() {
                Some(v) => v.hwnd,
                None => {
                    info!("  show: view is None!");
                    return;
                }
            }
        };

        unsafe {
            // 跨屏弹出时当场按目标屏 DPI 一次定好位置与尺寸（不再 SWP_NOSIZE），
            // 从源头避免窗口带着旧屏尺寸跨 DPI 而被系统二次缩放。
            RenderedView::apply_layout(self as *const Self, anchor, None);
            info!("  show: posting WM_SHOW_CANDIDATE");
            let result = PostMessageW(Some(hwnd), WM_SHOW_CANDIDATE, WPARAM(0), LPARAM(0));
            info!("  show: PostMessageW result: {:?}", result);
        }
    }

    pub fn hide(&self) {
        if let Some(view) = self.view.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            unsafe {
                let _ = PostMessageW(Some(view.hwnd), WM_HIDE_CANDIDATE, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn update(&self, ctx: &Context) {
        if let Some(view) = self.view.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            info!(
                "  update: hwnd={:?}, posting WM_UPDATE_CANDIDATE",
                view.hwnd.0
            );
            unsafe {
                let ctx_ptr = Box::into_raw(Box::new(ctx.clone()));
                let result = PostMessageW(
                    Some(view.hwnd),
                    WM_UPDATE_CANDIDATE,
                    WPARAM(ctx_ptr as usize),
                    LPARAM(0),
                );
                info!("  update: PostMessageW result: {:?}", result);
                if result.is_err() {
                    let _ = Box::from_raw(ctx_ptr);
                    info!("  update: PostMessageW failed, freed memory");
                }
            }
        } else {
            info!("  update: view is None!");
        }
    }

    pub fn show_root(&self, letter: char, root: &str) -> Result<(), String> {
        self.ensure_view_initialized();
        if let Some(view) = self.view.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let root_model = RootModel::from((letter, root.to_string()));
            *self.root_model.lock().unwrap_or_else(|e| e.into_inner()) = Some(root_model.clone());

            unsafe {
                let root_ptr = Box::into_raw(Box::new(root_model));
                let _ = PostMessageW(
                    Some(view.hwnd),
                    WM_SHOW_ROOT,
                    WPARAM(root_ptr as usize),
                    LPARAM(0),
                );
            }
            Ok(())
        } else {
            Err("view is None".to_string())
        }
    }

    pub fn hide_root(&self) {
        if let Some(view) = self.view.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            *self.root_model.lock().unwrap_or_else(|e| e.into_inner()) = None;
            unsafe {
                let _ = PostMessageW(Some(view.hwnd), WM_HIDE_ROOT, WPARAM(0), LPARAM(0));
            }
        }
    }
}
