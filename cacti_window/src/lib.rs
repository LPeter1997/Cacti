
// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

pub struct Monitor(MonitorImpl);

impl Monitor {
    pub fn all_monitors() -> Vec<Self> {
        MonitorImpl::all_monitors().into_iter().map(|m| Self(m)).collect()
    }

    pub fn position(&self) -> (isize, isize) {
        self.0.position()
    }

    pub fn resolution(&self) -> (usize, usize) {
        self.0.resolution()
    }

    pub fn dpi(&self) -> (f64, f64) {
        self.0.dpi()
    }

    pub fn scale(&self) -> f64 {
        self.0.scale()
    }

    pub fn is_primary(&self) -> bool {
        self.0.is_primary()
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

trait MonitorTrait: Sized {
    fn all_monitors() -> Vec<Self>;
    fn position(&self) -> (isize, isize);
    fn resolution(&self) -> (usize, usize);
    fn dpi(&self) -> (f64, f64);
    fn scale(&self) -> f64;
    fn is_primary(&self) -> bool;
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
            info: *mut MONITORINFO,
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
    struct MONITORINFO {
        cbSize      : u32 ,
        monitor_rect: RECT,
        work_rect   : RECT,
        flags       : u32 ,
    }

    impl MONITORINFO {
        fn new() -> Self {
            let mut res: MONITORINFO = unsafe{ mem::zeroed() };
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

        fn monitor_info(&self) -> MONITORINFO {
            let mut info = MONITORINFO::new();
            // TODO: Error handling
            unsafe{ GetMonitorInfoW(self.hmonitor, &mut info) };
            info
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

        fn position(&self) -> (isize, isize) {
            let info = self.monitor_info();
            let rect = info.monitor_rect;
            (rect.left as isize, rect.top as isize)
        }

        fn resolution(&self) -> (usize, usize) {
            let info = self.monitor_info();
            let rect = info.monitor_rect;
            let width = (rect.right - rect.left) as usize;
            let height = (rect.bottom - rect.top) as usize;
            (width, height)
        }

        fn dpi(&self) -> (f64, f64) {
            let mut dpix = 0u32;
            let mut dpiy = 0u32;
            // TODO: Error handling
            unsafe{ GetDpiForMonitor(
                self.hmonitor,
                MDT_RAW_DPI,
                &mut dpix,
                &mut dpiy) };
            (dpix as f64, dpiy as f64)
        }

        fn scale(&self) -> f64 {
            // TODO: Error handling
            let mut factor = 0u32;
            unsafe{ GetScaleFactorForMonitor(self.hmonitor, &mut factor) };

            match factor {
                // TODO: Error handling
                DEVICE_SCALE_FACTOR_INVALID => unimplemented!(),
                x => (x as f64) /100.0,
            }
        }

        fn is_primary(&self) -> bool {
            let info = self.monitor_info();
            (info.flags & MONITORINFOF_PRIMARY) != 0
        }
    }
}

#[cfg(target_os = "windows")] type MonitorImpl = win32::Win32Monitor;
