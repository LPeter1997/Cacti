
#![cfg(target_os = "windows")]
#![allow(non_snake_case)]

use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::mem;
use super::*;

// ////////////////////////////////////////////////////////////////////////// //
//                              Win API bindings                              //
// ////////////////////////////////////////////////////////////////////////// //

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

#[link(name = "user32")]
extern "system" {
    // Monitor stuff
    fn EnumDisplayMonitors(
        hdc: *mut c_void,
        clip_rect: *mut RECT,
        proc: Option<extern "system" fn(*mut c_void, *mut c_void, *mut RECT, isize) -> i32>,
        data: isize,
    ) -> i32;
    fn GetMonitorInfoW(hmonitor: *mut c_void, info: *mut MONITORINFOEXW) -> i32;
    fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
    // Window class
    fn RegisterClassW(class: *const WNDCLASSW) -> u16;
    // Window creation
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: *mut c_void,
        menu: *mut c_void,
        hinstance: *mut c_void,
        param: *mut c_void,
    ) -> *mut c_void;
    fn DestroyWindow(hwnd: *mut c_void) -> i32;
    // Window attributes
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    fn SetWindowTextW(hwnd: *mut c_void, title: *const u16) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        hwnd_after: *mut c_void,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        flags: u32,
    ) -> i32;
    fn SetLayeredWindowAttributes(
        hwnd: *mut c_void,
        color: u32,
        alpha: u8,
        flags: u32,
    ) -> i32;
    fn AdjustWindowRectEx(
        rect: *mut RECT,
        style: u32,
        menu: i32,
        ex_style: u32,
    ) -> i32;
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut RECT) -> i32;
    fn GetClientRect(hwnd: *mut c_void, rect: *mut RECT) -> i32;
    fn GetWindowPlacement(hwnd: *mut c_void, placement: *mut WINDOWPLACEMENT) -> i32;
    // Custom window properties
    fn GetWindowLongW(hwnd: *mut c_void, index: i32) -> i32;
    fn SetWindowLongW(hwnd: *mut c_void, index: i32, new: i32) -> i32;
    fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, new: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
    // Event handling
    fn DefWindowProcW(
        hwnd: *mut c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    fn GetMessageW(
        msg: *mut MSG,
        hwnd: *mut c_void,
        min: u32,
        max: u32,
    ) -> i32;
    fn PeekMessageW(
        msg: *mut MSG,
        hwnd: *mut c_void,
        min: u32,
        max: u32,
        action: u32,
    ) -> i32;
    fn SendMessageW(
        hwnd: *mut c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    fn PostQuitMessage(code: i32);
    fn TranslateMessage(msg: *const MSG) -> i32;
    fn DispatchMessageW(msg: *const MSG) -> i32;
}

#[link(name = "shcore")]
extern "system" {
    fn GetDpiForMonitor(
        hmonitor: *mut c_void,
        dpity: u32,
        dpix: *mut u32,
        dpiy: *mut u32,
    ) -> i32;
    fn GetScaleFactorForMonitor(hmonitor: *mut c_void, factor: *mut u32) -> i32;
}

const MONITORINFOF_PRIMARY: u32 = 1;

const MONITOR_DEFAULTTONEAREST: u32 = 2;

const MDT_EFFECTIVE_DPI: u32 = 0;
const MDT_ANGULAR_DPI: u32 = 1;
const MDT_RAW_DPI: u32 = 2;

const DEVICE_SCALE_FACTOR_INVALID: u32 = 0;

const WS_OVERLAPPED: u32 = 0x00000000;
const WS_THICKFRAME: u32 = 0x00040000;
const WS_CAPTION: u32 = 0x00C00000;
const WS_SYSMENU: u32 = 0x00080000;
const WS_MINIMIZEBOX: u32 = 0x00020000;
const WS_MAXIMIZEBOX: u32 = 0x00010000;
const WS_OVERLAPPEDWINDOW: u32 =
      WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU
    | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;

const WS_EX_LAYERED: u32 = 0x00080000;
const WS_EX_DLGMODALFRAME: u32 = 0x00000001;
const WS_EX_WINDOWEDGE: u32 = 0x00000100;
const WS_EX_CLIENTEDGE: u32 = 0x00000200;
const WS_EX_STATICEDGE: u32 = 0x00020000;

const CW_USEDEFAULT: i32 = 0x80000000u32 as i32;

const SW_HIDE: i32 = 0;
const SW_MAXIMIZE: i32 = 3;
const SW_SHOW: i32 = 5;

const HWND_TOP: *mut c_void = 0 as *mut c_void;
const HWND_TOPMOST: *mut c_void = (-1isize) as *mut c_void;
const HWND_NOTOPMOST: *mut c_void = (-2isize) as *mut c_void;

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;

const CS_OWNDC: u32 = 0x0020;

const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;

const GWLP_WNDPROC: i32 = -4;
const GWLP_USERDATA: i32 = -21;

const PM_REMOVE: u32 = 0x0001;

const LWA_ALPHA: u32 = 0x00000002;

const WM_CREATE: u32 = 0x0001;
const WM_CLOSE: u32 = 0x0010;
const WM_QUIT: u32 = 0x0012;
const WM_DESTROY: u32 = 0x0002;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_SETFOCUS: u32 = 0x0007;
const WM_SIZING: u32 = 0x0214;
const WM_SIZE: u32 = 0x0005;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RECT {
    left  : i32,
    top   : i32,
    right : i32,
    bottom: i32,
}

impl RECT {
    fn new() -> Self { unsafe{ mem::zeroed() } }

    fn width(&self) -> i32 { self.right - self.left }
    fn height(&self) -> i32 { self.bottom - self.top }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MONITORINFO {
    cbSize      : u32 ,
    monitor_rect: RECT,
    work_rect   : RECT,
    flags       : u32 ,
}

impl MONITORINFO {
    fn new() -> Self {
        let mut res: Self = unsafe{ mem::zeroed() };
        res.cbSize = mem::size_of::<Self>() as u32;
        res
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MONITORINFOEXW {
    cb_size     : u32 ,
    monitor_rect: RECT,
    work_rect   : RECT,
    flags       : u32 ,
    dev_name    : [u16; 32],
}

impl MONITORINFOEXW {
    fn new() -> Self {
        let mut res: Self = unsafe{ mem::zeroed() };
        res.cb_size = mem::size_of::<Self>() as u32;
        res
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct WNDCLASSW {
    style      : u32,
    wnd_proc   : Option<extern "system" fn(*mut c_void, u32, usize, isize) -> isize>,
    cls_extra  : i32,
    wnd_extra  : i32,
    hinstance  : *mut c_void,
    hicon      : *mut c_void,
    hcursor    : *mut c_void,
    hbackground: *mut c_void,
    menu_name  : *const u16,
    class_name : *const u16,
}

impl WNDCLASSW {
    fn new() -> Self { unsafe{ mem::zeroed() } }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct MSG {
    hwnd: *mut c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: POINT,
    private: u32,
}

impl MSG {
    fn new() -> Self { unsafe{ mem::zeroed() } }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct WINDOWPLACEMENT {
    length: u32,
    flags: u32,
    show: u32,
    min: POINT,
    max: POINT,
    normal_pos: RECT,
    device: RECT,
}

impl WINDOWPLACEMENT {
    fn new() -> Self {
        let mut res: Self = unsafe{ mem::zeroed() };
        res.length = mem::size_of::<Self>() as u32;
        res
    }
}

#[repr(C)]
struct CREATESTRUCTW {
    param: *mut c_void,
    hinstance: *mut c_void,
    menu: *mut c_void,
    parent: *mut c_void,
    height: i32,
    width: i32,
    y: i32,
    x: i32,
    style: i32,
    name: *const u16,
    class: *const u16,
    exstyle: u32,
}

/// Converts the Rust &OsStr into a WinAPI `WCHAR` string.
fn to_wstring(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0).into_iter()).collect()
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// TODO: Check return values

#[derive(Debug)]
pub struct Win32Monitor {
    hmonitor: *mut c_void,
}

impl Win32Monitor {
    extern "system" fn monitor_enum_proc(
        hmonitor: *mut c_void,
        _hdc: *mut c_void,
        _lprect: *mut RECT,
        lparam: isize,
    ) -> i32 {
        let monitors = unsafe{ &mut *(lparam as *mut Vec<Self>) };
        monitors.push(Self{ hmonitor });
        1
    }
}

impl MonitorTrait for Win32Monitor {
    fn all_monitors() -> Vec<Self> {
        let mut monitors: Vec<Self> = Vec::new();
        unsafe{ EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null_mut(),
            Some(Self::monitor_enum_proc),
            &mut monitors as *mut Vec<Self> as isize) };
        monitors
    }

    fn handle_ptr(&self) -> *mut c_void { self.hmonitor }

    fn name(&self) -> Option<String> {
        let mut info = MONITORINFOEXW::new();
        let ret = unsafe{ GetMonitorInfoW(self.hmonitor, &mut info) };
        if ret == 0 {
            // TODO: Return error
            unimplemented!();
        }
        // Decode name
        let null_pos = info.dev_name.iter().position(|c| *c == 0).unwrap_or(32);
        let name = String::from_utf16_lossy(&info.dev_name[0..null_pos]);
        Some(name)
    }

    fn is_primary(&self) -> bool {
        let mut info = MONITORINFO::new();
        let ret = unsafe{ GetMonitorInfoW(self.hmonitor, (&mut info as *mut MONITORINFO).cast()) };
        if ret == 0 {
            // TODO: Return error
            unimplemented!();
        }
        (info.flags & MONITORINFOF_PRIMARY) != 0
    }

    fn position(&self) -> PhysicalPosition {
        let mut info = MONITORINFO::new();
        let ret = unsafe{ GetMonitorInfoW(self.hmonitor, (&mut info as *mut MONITORINFO).cast()) };
        if ret == 0 {
            // TODO: Return error
            unimplemented!();
        }
        let rect = info.monitor_rect;
        PhysicalPosition::new(rect.left, rect.top)
    }

    fn size(&self) -> PhysicalSize {
        let mut info = MONITORINFO::new();
        let ret = unsafe{ GetMonitorInfoW(self.hmonitor, (&mut info as *mut MONITORINFO).cast()) };
        if ret == 0 {
            // TODO: Return error
            unimplemented!();
        }
        let rect = info.monitor_rect;
        PhysicalSize::new(rect.width() as u32, rect.height() as u32)
    }

    fn dpi(&self) -> Dpi {
        let mut dpix = 0u32;
        let mut dpiy = 0u32;
        let ret = unsafe{ GetDpiForMonitor(self.hmonitor, MDT_RAW_DPI, &mut dpix, &mut dpiy) };
        if ret != 0 {
            // TODO: Return error
            unimplemented!();
        }
        Dpi::new(dpix as f64, dpiy as f64)
    }

    fn scale(&self) -> f64 {
        let mut sfactor = 0u32;
        let ret = unsafe{ GetScaleFactorForMonitor(self.hmonitor, &mut sfactor) };
        if ret != 0 {
            // TODO: Return error
            unimplemented!();
        }
        if sfactor == DEVICE_SCALE_FACTOR_INVALID {
            // TODO: Return error
            unimplemented!();
        }
        (sfactor as f64) / 100.0
    }
}

#[derive(Debug)]
pub struct Win32EventLoop {
    window_handles: Vec<*mut c_void>,
}

impl EventLoopTrait for Win32EventLoop {
    fn new() -> Self {
        Self{
            window_handles: Vec::new(),
        }
    }

    fn add_window(&mut self, wnd: &Win32Window) {
        self.window_handles.push(wnd.handle_ptr());
    }

    fn run<F>(&mut self, mut f: F)
        where F: FnMut(&mut ControlFlow, Event) + 'static {
        // Set the user function
        let mut control_flow = ControlFlow::Poll;
        for handle in &self.window_handles {
            if let Some(data) = Win32Window::user_data(*handle) {
                data.handler = Some(&mut f);
                data.control_flow = &mut control_flow;
            }
        }
        // Start looping
        let mut msg = MSG::new();
        loop {
            // Decide if poll or to wait
            let poll = control_flow == ControlFlow::Poll;
            let have_events = unsafe{ if poll {
                    PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE)
                }
                else {
                    GetMessageW(&mut msg, ptr::null_mut(), 0, 0)
                } };
            if have_events != 0 {
                unsafe{
                    TranslateMessage(&mut msg);
                    DispatchMessageW(&mut msg);
                }
            }
            // Check exit condition
            if control_flow == ControlFlow::Exit {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HwndState {
    maximized: bool,
    style: u32,
    exstyle: u32,
    rect: RECT,
}

struct HwndUser {
    events: Vec<Event>,
    control_flow: *mut ControlFlow,
    handler: Option<*mut dyn FnMut(&mut ControlFlow, Event)>,
}

impl HwndUser {
    fn new() -> Self {
        Self{
            events: Vec::new(),
            control_flow: ptr::null_mut(),
            handler: None,
        }
    }
}

#[derive(Debug)]
pub struct Win32Window {
    hwnd: *mut c_void,
    windowed: Option<HwndState>,
}

impl Win32Window {
    fn user_data(hwnd: *mut c_void) -> Option<&'static mut HwndUser> {
        let user_data = unsafe{ GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut HwndUser;
        if user_data.is_null() {
            return None;
        }
        let data = unsafe{ &mut *user_data };
        Some(data)
    }

    extern "system" fn wnd_proc(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize {
        let window_id = WindowId(hwnd);
        let window_event = |event: WindowEvent| {
            Event::WindowEvent{ window_id, event }
        };
        let push_event = |e: Event| {
            if let Some(data) = Self::user_data(hwnd) {
                data.events.push(e);
            }
        };
        // Handle events
        let ret = match msg {
            // Existential messages
            WM_CREATE => {
                // We have to set the user-pointer
                let crea = unsafe{ &*(lparam as *const CREATESTRUCTW) };
                unsafe{ SetWindowLongPtrW(hwnd, GWLP_USERDATA, crea.param as isize); }
                // Send event
                push_event(window_event(WindowEvent::Created));
                0
            },
            WM_CLOSE => {
                push_event(window_event(WindowEvent::CloseRequested));
                0
            },
            WM_DESTROY => {
                push_event(window_event(WindowEvent::Closed));
                unsafe{ DefWindowProcW(hwnd, msg, wparam, lparam) }
            },
            // Focus
            WM_SETFOCUS => {
                push_event(window_event(WindowEvent::FocusChanged(true)));
                0
            },
            WM_KILLFOCUS => {
                push_event(window_event(WindowEvent::FocusChanged(false)));
                0
            },
            // Size
            WM_SIZING => {
                let rect = unsafe{ &*(lparam as *const RECT) };
                let width = rect.width() as u32;
                let height = rect.height() as u32;
                let size = PhysicalSize::new(width, height);
                push_event(window_event(WindowEvent::Resized(size)));
                unsafe{ DefWindowProcW(hwnd, msg, wparam, lparam) }
            },
            WM_SIZE => {
                let width = (lparam & 0xffff) as u32;
                let height = (lparam >> 16) as u32;
                let size = PhysicalSize::new(width, height);
                push_event(window_event(WindowEvent::Resized(size)));
                unsafe{ DefWindowProcW(hwnd, msg, wparam, lparam) }
            },
            // Others
            _ => unsafe{ DefWindowProcW(hwnd, msg, wparam, lparam) },
        };
        // Try emptying the queue
        if let Some(data) = Self::user_data(hwnd) {
            if let Some(handler) = data.handler {
                let handler = unsafe{ &mut *handler };
                let control_flow = unsafe{ &mut *data.control_flow };
                for e in data.events.drain(..) {
                    handler(control_flow, e);
                }
            }
        }
        ret
    }
}

impl WindowTrait for Win32Window {
    fn new() -> Self {
        let hinstance = unsafe{ GetModuleHandleW(ptr::null_mut()) };
        if hinstance.is_null() {
            // TODO: Error handling
            unimplemented!();
        }

        let class_name = to_wstring(OsStr::new("Cacti Window Class"));
        let window_name = to_wstring(OsStr::new("Window Name"));

        // Window class
        let mut wndclass = WNDCLASSW::new();
        wndclass.style = CS_OWNDC;
        wndclass.wnd_proc = Some(Self::wnd_proc);
        wndclass.hinstance = hinstance;
        wndclass.class_name = class_name.as_ptr();

        let ret = unsafe{ RegisterClassW(&wndclass) };
        if ret == 0 {
            // TODO: Return error
            unimplemented!();
        }

        // User data
        let user_data = Box::leak(Box::new(HwndUser::new()));

        // Window
        let hwnd = unsafe{ CreateWindowExW(
            WS_EX_LAYERED,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT,
            CW_USEDEFAULT, CW_USEDEFAULT,
            ptr::null_mut(),
            ptr::null_mut(),
            hinstance,
            (user_data as *mut HwndUser).cast()) };
        if hwnd.is_null() {
            // TODO: Return error
            unimplemented!();
        }

        unsafe{ SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) };

        Self{
            hwnd,
            windowed: None,
        }
    }

    fn close(&mut self) {
        unsafe{ ShowWindow(self.hwnd, SW_HIDE) };
    }

    fn handle_ptr(&self) -> *mut c_void { self.hwnd }

    fn monitor(&self) -> Win32Monitor {
        let monitor = unsafe{ MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST) };
        Win32Monitor{ hmonitor: monitor }
    }

    fn inner_size(&self) -> PhysicalSize {
        let mut rect = RECT::new();
        unsafe{ GetClientRect(self.hwnd, &mut rect) };
        PhysicalSize::new(rect.width() as u32, rect.height() as u32)
    }

    fn outer_size(&self) -> PhysicalSize {
        let mut rect = RECT::new();
        unsafe{ GetWindowRect(self.hwnd, &mut rect) };
        PhysicalSize::new(rect.width() as u32, rect.height() as u32)
    }

    fn set_visible(&mut self, vis: bool) {
        let cmd = if vis { SW_SHOW } else { SW_HIDE };
        unsafe{ ShowWindow(self.hwnd, cmd) };
    }

    fn set_resizable(&mut self, res: bool) -> bool {
        const FLAGS: u32 = WS_MAXIMIZEBOX | WS_THICKFRAME;
        let style = unsafe{ GetWindowLongW(self.hwnd, GWL_STYLE) } as u32;
        let newstyle = if res { style | FLAGS } else { style & !FLAGS };
        unsafe{ SetWindowLongW(self.hwnd, GWL_STYLE, newstyle as i32) };
        true
    }

    fn set_title(&mut self, title: &str) -> bool {
        let wtitle = to_wstring(OsStr::new(title));
        unsafe{ SetWindowTextW(self.hwnd, wtitle.as_ptr()) != 0 }
    }

    fn set_position(&mut self, pos: PhysicalPosition) -> bool {
        unsafe{ SetWindowPos(self.hwnd, HWND_TOP, pos.x, pos.y, 0, 0, SWP_NOSIZE | SWP_NOZORDER) != 0 }
    }

    fn set_inner_size(&mut self, siz: PhysicalSize) -> bool {
        let style = unsafe{ GetWindowLongW(self.hwnd, GWL_STYLE) };
        let exstyle = unsafe{ GetWindowLongW(self.hwnd, GWL_EXSTYLE) };
        let mut rect = RECT{
            left: 0,
            top: 0,
            right: siz.width as i32,
            bottom: siz.height as i32,
        };
        let ret = unsafe{ AdjustWindowRectEx(&mut rect, style as u32, 0, exstyle as u32) };
        if ret == 0 {
            return false;
        }
        unsafe{ SetWindowPos(
            self.hwnd, HWND_TOP, 0, 0, rect.width(), rect.height(), SWP_NOMOVE | SWP_NOZORDER) != 0 }
    }

    fn set_outer_size(&mut self, siz: PhysicalSize) -> bool {
        unsafe{ SetWindowPos(
            self.hwnd, HWND_TOP, 0, 0, siz.width as i32, siz.height as i32, SWP_NOMOVE | SWP_NOZORDER) != 0 }
    }

    fn set_pinned(&mut self, p: bool) -> bool {
        let tm = if p { HWND_TOPMOST } else { HWND_NOTOPMOST };
        unsafe{ SetWindowPos(self.hwnd, tm, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE) != 0 }
    }

    fn set_transparency(&mut self, t: f64) -> bool {
        let b = (t * 255.0) as u8;
        unsafe{ SetLayeredWindowAttributes(self.hwnd, 0, b, LWA_ALPHA) != 0 }
    }

    fn set_fullscreen(&mut self, fs: bool) -> bool {
        const FLAGS: u32 = WS_CAPTION | WS_THICKFRAME;
        const EXFLAGS: u32 = WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE
            | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE;

        if fs == self.windowed.is_some() {
            // Already in desired state
            return true;
        }

        if fs {
            // Save windowed state
            let mut placement = WINDOWPLACEMENT::new();
            let ret = unsafe{ GetWindowPlacement(self.hwnd, &mut placement) };
            if ret == 0 {
                return false;
            }
            let maximized = placement.show == SW_MAXIMIZE as u32;
            let style = unsafe{ GetWindowLongW(self.hwnd, GWL_STYLE) } as u32;
            let exstyle = unsafe{ GetWindowLongW(self.hwnd, GWL_EXSTYLE) } as u32;
            let rect = placement.normal_pos;
            self.windowed = Some(HwndState{ maximized, style, exstyle, rect });
            // Remove windowed styles
            unsafe{
                SetWindowLongW(self.hwnd, GWL_STYLE, (style & !FLAGS) as i32);
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, (exstyle & !EXFLAGS) as i32);
            }
            // Stretch on current monitor
            let monitor = unsafe{ MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST) };
            let mut minfo = MONITORINFO::new();
            let ret = unsafe{ GetMonitorInfoW(monitor, (&mut minfo as *mut MONITORINFO).cast()) };
            if ret == 0 {
                return false;
            }
            let mrect = minfo.monitor_rect;
            unsafe{ SetWindowPos(
                self.hwnd,
                HWND_TOP,
                mrect.left, mrect.top, mrect.width(), mrect.height(),
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED) != 0 }
        }
        else {
            // Restore state
            let state = self.windowed.take().unwrap();
            unsafe{
                SetWindowLongW(self.hwnd, GWL_STYLE, state.style as i32);
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, state.exstyle as i32);
            }
            let rect = state.rect;
            let res = unsafe{ SetWindowPos(
                self.hwnd,
                HWND_TOP,
                rect.left, rect.top, rect.width(), rect.height(),
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED) };
            if res == 0 {
                return false;
            }
            if state.maximized {
                unsafe{ ShowWindow(self.hwnd, SW_MAXIMIZE) };
            }
            true
        }
    }
}

impl Drop for Win32Window {
    fn drop(&mut self) {
        let user_data = unsafe{ GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) } as *mut HwndUser;
        unsafe{ Box::from_raw(user_data); }
        unsafe{ DestroyWindow(self.hwnd); }
    }
}
