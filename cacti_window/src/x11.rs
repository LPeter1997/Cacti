
#![cfg(target_os = "linux")]

use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr;
use std::mem;
use super::*;

use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr;
use std::mem;

// ////////////////////////////////////////////////////////////////////////// //
//                                X11 bindings                                //
// ////////////////////////////////////////////////////////////////////////// //

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(name: *mut c_char) -> *mut c_void;
    fn XScreenCount(display: *mut c_void) -> i32;
    fn XScreenOfDisplay(display: *mut c_void, index: i32) -> *mut c_void;
    fn XWidthOfScreen(screen: *mut c_void) -> i32;
    fn XHeightOfScreen(screen: *mut c_void) -> i32;
    fn XDefaultScreenOfDisplay(display: *mut c_void) -> *mut c_void;
    fn XRootWindowOfScreen(screen: *mut c_void) -> u32;
    fn XGetGeometry(
        display: *mut c_void,
        drawable: u32,
        root: *mut u32,
        xpos: *mut i32,
        ypos: *mut i32,
        width: *mut u32,
        height: *mut u32,
        border: *mut u32,
        depth: *mut u32,
    ) -> i32;
    fn XWidthMMOfScreen(screen: *mut c_void) -> i32;
    fn XHeightMMOfScreen(screen: *mut c_void) -> i32;
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// TODO: Check return values

#[derive(Debug)]
pub struct X11Monitor {
    srvr: *mut c_void,
    handle: *mut c_void,
}

impl MonitorTrait for X11Monitor {
    fn all_monitors() -> Vec<Self> {
        let mut result = Vec::new();
        let srvr = unsafe{ XOpenDisplay(ptr::null()) };
        let cnt = unsafe{ XScreenCount(srvr) };
        for i in 0..cnt {
            let handle = unsafe{ XScreenOfDisplay(srvr, i) };
            result.push(Self{ srvr, handle });
        }
        result
    }

    fn handle_ptr(&self) -> *mut c_void {
        self.handle
    }

    fn name(&self) -> Option<String> {
        // TODO
        None
    }

    fn is_primary(&self) -> bool {
        // NOTE: Not sure this is correct
        let def = unsafe{ XDefaultScreenOfDisplay(self.srvr) };
        def == self.handle
    }

    fn position(&self) -> PhysicalPosition {
        let root = unsafe{ XRootWindowOfScreen(screen0) };
        let (mut ret_root, mut xp, mut yp, mut width, mut height, mut border, mut depth) =
            (0, 0, 0, 0, 0, 0, 0);
        unsafe{ XGetGeometry(
            self.srvr, root,
            &mut ret_root,
            &mut xp, &mut yp,
            &mut width, &mut height,
            &mut border, &mut depth) };
        PhysicalPosition::new(xp, yp)
    }

    fn size(&self) -> PhysicalSize {
        let root = unsafe{ XRootWindowOfScreen(screen0) };
        let (mut ret_root, mut xp, mut yp, mut width, mut height, mut border, mut depth) =
            (0, 0, 0, 0, 0, 0, 0);
        unsafe{ XGetGeometry(
            self.srvr, root,
            &mut ret_root,
            &mut xp, &mut yp,
            &mut width, &mut height,
            &mut border, &mut depth) };
            PhysicalSize::new(width, height)
    }

    fn dpi(&self) -> Dpi {
        let root = unsafe{ XRootWindowOfScreen(screen0) };
        let (mut ret_root, mut xp, mut yp, mut width, mut height, mut border, mut depth) =
            (0, 0, 0, 0, 0, 0, 0);
        unsafe{ XGetGeometry(
            self.srvr, root,
            &mut ret_root,
            &mut xp, &mut yp,
            &mut width, &mut height,
            &mut border, &mut depth) };
        let width_mm = unsafe{ XWidthMMOfScreen(self.handle) };
        let height_mm = unsafe{ XHeightMMOfScreen(self.handle) };
        let dpix = (width as f64) / (width_mm as f64) * 25.4;
        let dpiy = (height as f64) / (height_mm as f64) * 25.4;
        Dpi::new(dpix, dpiy)
    }

    fn scale(&self) -> f64 {
        // TODO
        1.0
    }
}

#[derive(Debug)]
pub struct X11EventLoop {
}

impl EventLoopTrait for X11EventLoop {
    fn new() -> Self { unimplemented!() }

    fn add_window(&mut self, wnd: &WindowImpl) { unimplemented!() }

    fn run<F>(&mut self, f: F)
        where F: FnMut(&mut ControlFlow, Event) + 'static {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct X11Window {
}

impl WindowTrait for X11Window {
    fn new() -> Self {
        unimplemented!()
    }

    fn close(&mut self) {
        unimplemented!()
    }

    fn handle_ptr(&self) -> *mut c_void {
        unimplemented!()
    }

    fn monitor(&self) -> MonitorImpl {
        unimplemented!()
    }

    fn inner_size(&self) -> PhysicalSize {
        unimplemented!()
    }

    fn outer_size(&self) -> PhysicalSize {
        unimplemented!()
    }

    fn set_visible(&mut self, vis: bool) {
        unimplemented!()
    }

    fn set_resizable(&mut self, res: bool) -> bool {
        unimplemented!()
    }

    fn set_title(&mut self, title: &str) -> bool {
        unimplemented!()
    }

    fn set_position(&mut self, pos: PhysicalPosition) -> bool {
        unimplemented!()
    }

    fn set_inner_size(&mut self, siz: PhysicalSize) -> bool {
        unimplemented!()
    }

    fn set_outer_size(&mut self, siz: PhysicalSize) -> bool {
        unimplemented!()
    }

    fn set_pinned(&mut self, p: bool) -> bool {
        unimplemented!()
    }

    fn set_transparency(&mut self, t: f64) -> bool {
        unimplemented!()
    }

    fn set_fullscreen(&mut self, fs: bool) -> bool {
        unimplemented!()
    }
}
