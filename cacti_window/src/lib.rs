
// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

pub struct Monitor(MonitorImpl);

impl Monitor {
    pub fn all_monitors() -> Vec<Self> {
        MonitorImpl::all_monitors().into_iter().map(|m| Self(m)).collect()
    }

    pub fn resolution(&self) -> (usize, usize) {
        self.0.resolution()
    }

    pub fn dpi(&self) -> (f64, f64) {
        self.0.dpi()
    }

    pub fn scale(&self) -> (f64, f64) {
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
    fn resolution(&self) -> (usize, usize);
    fn dpi(&self) -> (f64, f64);
    fn scale(&self) -> (f64, f64);
    fn is_primary(&self) -> bool;
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::c_void;
    use std::ptr;
    use super::*;

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: *mut c_void,
            clip_rect: *mut c_void,
            proc: extern "system" fn(*mut c_void, *mut c_void, *mut c_void, isize),
            data: isize,
        ) -> i32;
    }

    #[derive(Debug)]
    pub struct Win32Monitor {}

    extern "system" fn monitor_enum_proc(
        hmonitor: *mut c_void,
        hdc: *mut c_void,
        lprect: *mut c_void,
        lparam: isize
    ) {
        println!("Monitor!");
    }

    impl MonitorTrait for Win32Monitor {
        fn all_monitors() -> Vec<Self> {
            unsafe{ EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null_mut(),
                monitor_enum_proc,
                0) };
            unimplemented!()
        }

        fn resolution(&self) -> (usize, usize) {
            unimplemented!()
        }

        fn dpi(&self) -> (f64, f64) {
            unimplemented!()
        }

        fn scale(&self) -> (f64, f64) {
            unimplemented!()
        }

        fn is_primary(&self) -> bool {
            unimplemented!()
        }
    }
}

#[cfg(target_os = "windows")] type MonitorImpl = win32::Win32Monitor;
