//! TODO: doc

use std::io;
use std::ffi::c_void;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

#[derive(Debug)]
pub struct Monitor {
    monitor: MonitorImpl,
    info: MonitorInfo,
}

impl Monitor {
    pub fn all_monitors() -> Vec<Self> {
        MonitorImpl::all_monitors().into_iter().filter_map(|monitor|
            monitor.info().ok().map(|info| Self{ monitor, info })
        ).collect()
    }

    pub fn handle_ptr(&self) -> *const c_void {
        self.monitor.handle_ptr()
    }

    pub fn handle_mut_ptr(&mut self) -> *mut c_void {
        self.monitor.handle_mut_ptr()
    }

    pub fn name(&self) -> Option<&str> {
        self.info.name.as_ref().map(|n| n.as_str())
    }

    pub fn position(&self) -> (i32, i32) {
        self.info.position
    }

    pub fn size(&self) -> (u32, u32) {
        self.info.size
    }

    pub fn dpi(&self) -> (f64, f64) {
        self.info.dpi
    }

    pub fn scale(&self) -> f64 {
        self.info.scale
    }

    pub fn is_primary(&self) -> bool {
        self.info.primary
    }
}

#[derive(Debug)]
pub struct Window {
    window: WindowImpl,
}

impl Window {
    pub fn new() -> Self {
        Self{ window: WindowImpl::new() }
    }

    pub fn handle_ptr(&self) -> *const c_void { self.window.handle_ptr() }
    pub fn handle_mut_ptr(&mut self) -> *mut c_void { self.window.handle_mut_ptr() }

    pub fn inner_size(&self) -> (u32, u32) {
        self.window.inner_size()
    }

    pub fn outer_size(&self) -> (u32, u32) {
        self.window.outer_size()
    }

    pub fn set_visible(&mut self, vis: bool) {
        self.window.set_visible(vis)
    }

    pub fn set_resizable(&mut self, res: bool) -> bool {
        self.window.set_resizable(res)
    }

    pub fn set_title(&mut self, title: &str) -> bool {
        self.window.set_title(title)
    }

    pub fn set_position(&mut self, x: i32, y: i32) -> bool {
        self.window.set_position(x, y)
    }

    pub fn set_inner_size(&mut self, w: u32, h: u32) -> bool {
        self.window.set_inner_size(w, h)
    }

    pub fn set_pinned(&mut self, p: bool) -> bool {
        self.window.set_pinned(p)
    }

    pub fn set_transparency(&mut self, t: f64) -> bool {
        self.window.set_transparency(t)
    }

    pub fn set_fullscreen(&mut self, fs: bool) -> bool {
        self.window.set_fullscreen(fs)
    }

    pub fn run_event_loop<F>(&mut self, mut f: F)
        where F: FnMut(&mut EventLoop) {
        self.window.run_event_loop(f);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventLoop {
    Wait,
    Poll,
    Stop,
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

trait MonitorTrait: Sized {
    fn all_monitors() -> Vec<Self>;

    fn handle_ptr(&self) -> *const c_void;
    fn handle_mut_ptr(&mut self) -> *mut c_void;

    fn info(&self) -> io::Result<MonitorInfo>;
}

#[derive(Debug)]
struct MonitorInfo {
    name    : Option<String>,
    position: (i32, i32),
    size    : (u32, u32),
    dpi     : (f64, f64),
    scale   : f64,
    primary : bool,
}

trait WindowTrait: Sized {
    fn new() -> Self;

    fn handle_ptr(&self) -> *const c_void;
    fn handle_mut_ptr(&mut self) -> *mut c_void;

    fn inner_size(&self) -> (u32, u32);
    fn outer_size(&self) -> (u32, u32);

    fn set_visible(&mut self, vis: bool);
    fn set_resizable(&mut self, res: bool) -> bool;
    fn set_title(&mut self, title: &str) -> bool;
    fn set_position(&mut self, x: i32, y: i32) -> bool;
    fn set_inner_size(&mut self, w: u32, h: u32) -> bool;
    fn set_pinned(&mut self, p: bool) -> bool;
    fn set_transparency(&mut self, t: f64) -> bool;
    fn set_fullscreen(&mut self, fs: bool) -> bool;

    fn run_event_loop<F>(&mut self, f: F) where F: FnMut(&mut EventLoop);
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{OsStr, c_void};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::mem;
    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: *mut c_void,
            clip_rect: *mut c_void,
            proc: extern "system" fn(*mut c_void, *mut c_void, *mut c_void, isize) -> i32,
            data: isize,
        ) -> i32;

        fn GetMonitorInfoW(
            hmonitor: *mut c_void,
            info: *mut MONITORINFOEXW,
        ) -> i32;

        fn RegisterClassW(class: *const WNDCLASSW) -> u16;

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

        fn ShowWindow(
            hwnd: *mut c_void,
            cmd: i32,
        ) -> i32;

        fn DefWindowProcW(
            hwnd: *mut c_void,
            msg: u32,
            wparam: u32,
            lparam: i32,
        ) -> i32;

        fn SetWindowTextW(
            hwnd: *mut c_void,
            title: *const u16,
        ) -> i32;

        fn SetWindowPos(
            hwnd: *mut c_void,
            hwnd_after: *mut c_void,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;

        fn AdjustWindowRectEx(
            rect: *mut RECT,
            style: u32,
            menu: i32,
            ex_style: u32,
        ) -> i32;

        fn GetWindowLongA(
            hwnd: *mut c_void,
            index: i32,
        ) -> i32;

        fn SetWindowLongA(
            hwnd: *mut c_void,
            index: i32,
            new: i32,
        ) -> i32;

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

        fn TranslateMessage(msg: *const MSG) -> i32;

        fn DispatchMessageW(msg: *const MSG) -> i32;

        fn SetLayeredWindowAttributes(
            hwnd: *mut c_void,
            color: u32,
            alpha: u8,
            flags: u32,
        ) -> i32;

        fn GetWindowRect(
            hwnd: *mut c_void,
            rect: *mut RECT,
        ) -> i32;

        fn SendMessageW(
            hwnd: *mut c_void,
            msg: u32,
            wparam: u32,
            lparam: i32,
        ) -> i32;

        fn MonitorFromWindow(
            hwnd: *mut c_void,
            flags: u32,
        ) -> *mut c_void;

        fn GetWindowPlacement(
            hwnd: *mut c_void,
            placement: *mut WINDOWPLACEMENT,
        ) -> i32;

        fn PostQuitMessage(code: i32);
    }

    #[link(name = "shcore")]
    extern "system" {
        fn GetDpiForMonitor(
            hmonitor: *mut c_void,
            dpity: u32,
            dpix: *mut u32,
            dpiy: *mut u32,
        ) -> i32;

        fn GetScaleFactorForMonitor(
            hmonitor: *mut c_void,
            factor: *mut u32,
        ) -> i32;
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

    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;

    const PM_REMOVE: u32 = 0x0001;

    const LWA_ALPHA: u32 = 0x00000002;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct RECT {
        left  : i32,
        top   : i32,
        right : i32,
        bottom: i32,
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
        cbSize      : u32 ,
        monitor_rect: RECT,
        work_rect   : RECT,
        flags       : u32 ,
        dev_name    : [u16; 32],
    }

    impl MONITORINFOEXW {
        fn new() -> Self {
            let mut res: Self = unsafe{ mem::zeroed() };
            res.cbSize = mem::size_of::<Self>() as u32;
            res
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct WNDCLASSW {
        style      : u32,
        wnd_proc   : Option<extern "system" fn(*mut c_void, u32, u32, i32) -> i32>,
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
        fn new() -> Self {
            unsafe{ mem::zeroed() }
        }
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
        wparam: u32,
        lparam: i32,
        time: u32,
        point: POINT,
        private: u32,
    }

    impl MSG {
        fn new() -> Self {
            unsafe{ mem::zeroed() }
        }
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

    /// Converts the Rust &OsStr into a WinAPI `WCHAR` string.
    fn to_wstring(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0).into_iter()).collect()
    }

    #[derive(Debug)]
    pub struct Win32Monitor {
        hmonitor: *mut c_void,
    }

    impl Win32Monitor {
        extern "system" fn monitor_enum_proc(
            hmonitor: *mut c_void,
            _hdc: *mut c_void,
            _lprect: *mut c_void,
            lparam: isize
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
                Self::monitor_enum_proc,
                &mut monitors as *mut Vec<Self> as isize) };
            monitors
        }

        fn handle_ptr(&self) -> *const c_void { self.hmonitor }
        fn handle_mut_ptr(&mut self) -> *mut c_void { self.hmonitor }

        fn info(&self) -> io::Result<MonitorInfo> {
            // Get MONITORINFOEXW
            let mut info = MONITORINFOEXW::new();
            let ret = unsafe{ GetMonitorInfoW(self.hmonitor, &mut info) };
            if ret == 0 {
                // TODO: Return error
                unimplemented!();
            }
            let rect = info.monitor_rect;
            let primary = (info.flags & MONITORINFOF_PRIMARY) != 0;
            // Decode name
            let null_pos = info.dev_name.iter().position(|c| *c == 0).unwrap_or(32);
            let name = String::from_utf16_lossy(&info.dev_name[0..null_pos]);

            // Get DPI
            let mut dpix = 0u32;
            let mut dpiy = 0u32;
            let ret = unsafe{ GetDpiForMonitor(
                self.hmonitor,
                MDT_RAW_DPI,
                &mut dpix,
                &mut dpiy) };
            if ret != 0 {
                // TODO: Return error
                unimplemented!();
            }

            // Get scale factor
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
            let scale = (sfactor as f64) / 100.0;

            Ok(MonitorInfo{
                name: Some(name),
                position: (rect.left, rect.top),
                size: ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32),
                dpi: (dpix as f64, dpiy as f64),
                scale,
                primary,
            })
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct HwndState {
        maximized: bool,
        style: u32,
        exstyle: u32,
        rect: RECT,
    }

    #[derive(Debug)]
    pub struct Win32Window {
        hwnd: *mut c_void,
        windowed: Option<HwndState>,
    }

    impl Win32Window {
        extern "system" fn wnd_proc(hwnd: *mut c_void, msg: u32, wparam: u32, lparam: i32) -> i32 {
            unsafe{ DefWindowProcW(hwnd, msg, wparam, lparam) }
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
            wndclass.wnd_proc = Some(Self::wnd_proc);
            wndclass.hinstance = hinstance;
            wndclass.class_name = class_name.as_ptr();

            let ret = unsafe{ RegisterClassW(&wndclass) };
            if ret == 0 {
                // TODO: Return error
                unimplemented!();
            }

            // Window
            let hwnd = unsafe{ CreateWindowExW(
                WS_EX_LAYERED,
                class_name.as_ptr(),
                window_name.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                ptr::null_mut()) };
            if hwnd.is_null() {
                // TODO: Return error
                println!("aaa: {:?}", std::io::Error::last_os_error());
                unimplemented!();
            }

            Self{
                hwnd,
                windowed: None,
            }
        }

        fn handle_ptr(&self) -> *const c_void { self.hwnd }
        fn handle_mut_ptr(&mut self) -> *mut c_void { self.hwnd }

        fn inner_size(&self) -> (u32, u32) {
            unimplemented!()
        }

        fn outer_size(&self) -> (u32, u32) {
            unimplemented!()
        }

        fn set_visible(&mut self, vis: bool) {
            let cmd = if vis { SW_SHOW } else { SW_HIDE };
            unsafe{ ShowWindow(self.hwnd, cmd) };
        }

        fn set_resizable(&mut self, res: bool) -> bool {
            const FLAGS: u32 = WS_MAXIMIZEBOX | WS_THICKFRAME;
            let style = unsafe{ GetWindowLongA(self.hwnd, GWL_STYLE) } as u32;
            let newstyle = if res { style | FLAGS } else { style & !FLAGS };
            unsafe{ SetWindowLongA(self.hwnd, GWL_STYLE, newstyle as i32) };
            true
        }

        fn set_title(&mut self, title: &str) -> bool {
            let wtitle = to_wstring(OsStr::new(title));
            unsafe{ SetWindowTextW(self.hwnd, wtitle.as_ptr()) != 0 }
        }

        fn set_position(&mut self, x: i32, y: i32) -> bool {
            unsafe{ SetWindowPos(
                self.hwnd,
                HWND_TOP,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER) != 0 }
        }

        fn set_inner_size(&mut self, w: u32, h: u32) -> bool {
            let style = unsafe{ GetWindowLongA(self.hwnd, GWL_STYLE) };
            let exstyle = unsafe{ GetWindowLongA(self.hwnd, GWL_EXSTYLE) };
            let mut rect = RECT{
                left: 0,
                top: 0,
                right: w as i32,
                bottom: h as i32,
            };
            let ret = unsafe{ AdjustWindowRectEx(&mut rect, style as u32, 0, exstyle as u32) };
            if ret == 0 {
                return false;
            }
            unsafe{ SetWindowPos(
                self.hwnd,
                HWND_TOP,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOMOVE | SWP_NOZORDER) != 0 }
        }

        fn set_pinned(&mut self, p: bool) -> bool {
            let tm = if p { HWND_TOPMOST } else { HWND_NOTOPMOST };
            unsafe{ SetWindowPos(
                self.hwnd,
                tm,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE) != 0 }
        }

        fn set_transparency(&mut self, t: f64) -> bool {
            let b = (t * 255.0) as u8;
            unsafe{ SetLayeredWindowAttributes(
                self.hwnd,
                0,
                b,
                LWA_ALPHA) != 0 }
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
                let style = unsafe{ GetWindowLongA(self.hwnd, GWL_STYLE) } as u32;
                let exstyle = unsafe{ GetWindowLongA(self.hwnd, GWL_EXSTYLE) } as u32;
                let rect = placement.normal_pos;
                self.windowed = Some(HwndState{ maximized, style, exstyle, rect });
                // Remove windowed styles
                unsafe{
                    SetWindowLongA(self.hwnd, GWL_STYLE, (style & !FLAGS) as i32);
                    SetWindowLongA(self.hwnd, GWL_EXSTYLE, (exstyle & !EXFLAGS) as i32);
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
                    mrect.left,
                    mrect.top,
                    mrect.right - mrect.left,
                    mrect.bottom - mrect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED) != 0 }
            }
            else {
                // Restore state
                let state = self.windowed.take().unwrap();
                unsafe{
                    SetWindowLongA(self.hwnd, GWL_STYLE, state.style as i32);
                    SetWindowLongA(self.hwnd, GWL_EXSTYLE, state.exstyle as i32);
                }
                let rect = state.rect;
                let res = unsafe{ SetWindowPos(
                    self.hwnd,
                    HWND_TOP,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
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

        fn run_event_loop<F>(&mut self, mut f: F)
            where F: FnMut(&mut EventLoop) {
            // TODO: Not call f here, but inside the wnd_proc to avoid the modal loop crap
            let mut msg = MSG::new();
            let mut ev_loop = EventLoop::Poll;
            loop {
                f(&mut ev_loop);
                if ev_loop == EventLoop::Stop {
                    unsafe{ PostQuitMessage(0) };
                }

                if ev_loop == EventLoop::Poll {
                    let res = unsafe{ PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE) };
                    if res != 0 {
                        unsafe{
                            TranslateMessage(&mut msg);
                            DispatchMessageW(&mut msg);
                        }
                    }
                }
                else {
                    let res = unsafe{ GetMessageW(&mut msg, self.hwnd, 0, 0) };
                    if res == 0 {
                        break;
                    }
                    unsafe{
                        TranslateMessage(&mut msg);
                        DispatchMessageW(&mut msg);
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")] type MonitorImpl = win32::Win32Monitor;
#[cfg(target_os = "windows")] type WindowImpl = win32::Win32Window;
