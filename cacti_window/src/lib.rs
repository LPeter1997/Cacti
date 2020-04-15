//! TODO: doc

use std::io;
use std::ffi::c_void;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

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

    pub fn set_visible(&self, vis: bool) {
        self.window.set_visible(vis)
    }

    pub fn set_title(&self, title: &str) -> bool {
        self.window.set_title(title)
    }

    pub fn set_position(&self, x: i32, y: i32) -> bool {
        self.window.set_position(x, y)
    }

    pub fn set_inner_size(&self, w: u32, h: u32) -> bool {
        self.window.set_inner_size(w, h)
    }

    pub fn set_pinned(&self, p: bool) -> bool {
        self.window.set_pinned(p)
    }

    pub fn set_transparency(&self, t: f64) -> bool {
        self.window.set_transparency(t)
    }

    pub fn run_event_loop<F>(&mut self, f: F) where F: FnMut() {
        unimplemented!()
    }
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

    fn set_visible(&self, vis: bool);
    fn set_title(&self, title: &str) -> bool;
    fn set_position(&self, x: i32, y: i32) -> bool;
    fn set_inner_size(&self, w: u32, h: u32) -> bool;
    fn set_pinned(&self, p: bool) -> bool;
    fn set_transparency(&self, t: f64) -> bool;
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::c_void;
    use std::ptr;
    use std::mem;
    use super::*;

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
    }

    const MONITORINFOF_PRIMARY: u32 = 1;

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

    const MDT_EFFECTIVE_DPI: u32 = 0;
    const MDT_ANGULAR_DPI: u32 = 1;
    const MDT_RAW_DPI: u32 = 2;

    const DEVICE_SCALE_FACTOR_INVALID: u32 = 0;

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
    struct MONITORINFOEXW {
        cbSize      : u32 ,
        monitor_rect: RECT,
        work_rect   : RECT,
        flags       : u32 ,
        dev_name    : [u16; 32],
    }

    impl MONITORINFOEXW {
        fn new() -> Self {
            let mut res: MONITORINFOEXW = unsafe{ mem::zeroed() };
            res.cbSize = mem::size_of::<Self>() as u32;
            res
        }
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

    #[derive(Debug)]
    pub struct Win32Window {
        hwnd: *mut c_void,
    }

    impl WindowTrait for Win32Window {
        fn new() -> Self {
            unimplemented!()
        }

        fn handle_ptr(&self) -> *const c_void { self.hwnd }
        fn handle_mut_ptr(&mut self) -> *mut c_void { self.hwnd }

        fn inner_size(&self) -> (u32, u32) {
            unimplemented!()
        }

        fn set_visible(&self, vis: bool) {
            unimplemented!()
        }

        fn set_title(&self, title: &str) -> bool {
            unimplemented!()
        }

        fn set_position(&self, x: i32, y: i32) -> bool {
            unimplemented!()
        }

        fn set_inner_size(&self, w: u32, h: u32) -> bool {
            unimplemented!()
        }

        fn set_pinned(&self, p: bool) -> bool {
            unimplemented!()
        }

        fn set_transparency(&self, t: f64) -> bool {
            unimplemented!()
        }
    }
}

#[cfg(target_os = "windows")] type MonitorImpl = win32::Win32Monitor;
#[cfg(target_os = "windows")] type WindowImpl = win32::Win32Window;
