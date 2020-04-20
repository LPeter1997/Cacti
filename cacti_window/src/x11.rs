
#![cfg(target_os = "linux")]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint, c_long, c_ulong};
use std::cell::RefCell;
use std::ptr;
use std::mem;
use super::*;

// ////////////////////////////////////////////////////////////////////////// //
//                                X11 bindings                                //
// ////////////////////////////////////////////////////////////////////////// //

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut c_void;
    fn XCloseDisplay(display: *mut c_void) -> c_int;
    fn XScreenCount(display: *mut c_void) -> c_int;
    fn XScreenOfDisplay(display: *mut c_void, index: c_int) -> *mut c_void;
    fn XWidthOfScreen(screen: *mut c_void) -> c_int;
    fn XHeightOfScreen(screen: *mut c_void) -> c_int;
    fn XDefaultScreenOfDisplay(display: *mut c_void) -> *mut c_void;
    fn XRootWindowOfScreen(screen: *mut c_void) -> c_ulong;
    fn XGetGeometry(
        display : *mut c_void ,
        drawable: c_ulong     ,
        root    : *mut c_ulong,
        xpos    : *mut c_int  ,
        ypos    : *mut c_int  ,
        width   : *mut c_uint ,
        height  : *mut c_uint ,
        border  : *mut c_uint ,
        depth   : *mut c_uint ,
    ) -> c_int;
    fn XWidthMMOfScreen(screen: *mut c_void) -> c_int;
    fn XHeightMMOfScreen(screen: *mut c_void) -> c_int;
    fn XCreateSimpleWindow(
        display     : *mut c_void,
        parent      : c_ulong    ,
        x           : c_int      ,
        y           : c_int      ,
        width       : c_uint     ,
        height      : c_uint     ,
        border_width: c_uint     ,
        border      : c_ulong    ,
        background  : c_ulong    ,
    ) -> c_ulong;
    fn XBlackPixel(display: *mut c_void, screen_idx: c_int) -> c_ulong;
    fn XWhitePixel(display: *mut c_void, screen_idx: c_int) -> c_ulong;
    fn XMapWindow(display: *mut c_void, window: c_ulong) -> c_int;
    fn XUnmapWindow(display: *mut c_void, window: c_ulong) -> c_int;
    fn XNextEvent(display: *mut c_void, event: *mut XEvent) -> c_int;
    fn XFillRectangle(
        display : *mut c_void,
        drawable: c_ulong    ,
        gc      : *mut c_void,
        x       : c_int      ,
        y       : c_int      ,
        width   : c_uint     ,
        height  : c_uint     ,
    ) -> c_int;
    fn XDefaultGC(display: *mut c_void, screen_idx: c_int) -> *mut c_void;
    fn XSelectInput(
        display: *mut c_void,
        window : c_ulong    ,
        mask   : c_long     ,
    ) -> c_int;
    fn XDestroyWindow(display: *mut c_void, window: c_ulong) -> c_int;
}

const ExposureMask: c_long = 0x8000;

#[repr(C)]
struct XEvent {
    pad: [c_long; 24],
}

impl XEvent {
    fn new() -> Self {
        unsafe{ mem::zeroed() }
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// We need a mechanism to safely store a single display pointer and release it
// when no longer needed.
// NOTE: Do we need all this crud?

thread_local! {
    static CONNECTION: RefCell<(*mut c_void, usize)> = RefCell::new((ptr::null_mut(), 0));
}

fn ref_connection() -> *mut c_void {
    CONNECTION.with(|c| {
        let mut c = c.borrow_mut();
        if c.1 == 0 {
            c.0 = unsafe{ XOpenDisplay(ptr::null()) };
        }
        c.1 += 1;
        c.0
    })
}

fn forget_connection() {
    CONNECTION.with(|c| {
        let mut c = c.borrow_mut();
        c.1 -= 1;
        if c.1 == 0 {
            unsafe{ XCloseDisplay(c.0) };
            c.0 = ptr::null_mut();
        }
    });
}

#[derive(Debug)]
struct Connection(*mut c_void);

impl Connection {
    fn new() -> Self {
        Self(ref_connection())
    }
}

impl Clone for Connection {
    fn clone(&self) -> Self {
        ref_connection();
        Self(self.0)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        forget_connection();
    }
}

////////////////////////////////////////////////////////////////////////////////

// TODO: Check return values

#[derive(Debug)]
pub struct X11Monitor {
    srvr: Connection,
    handle: *mut c_void,
}

impl MonitorTrait for X11Monitor {
    fn all_monitors() -> Vec<Self> {
        let mut result = Vec::new();
        let srvr = Connection::new();
        let cnt = unsafe{ XScreenCount(srvr.0) };
        for i in 0..cnt {
            let handle = unsafe{ XScreenOfDisplay(srvr.0, i) };
            result.push(Self{ srvr: srvr.clone(), handle });
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
        let def = unsafe{ XDefaultScreenOfDisplay(self.srvr.0) };
        def == self.handle
    }

    fn position(&self) -> PhysicalPosition {
        let root = unsafe{ XRootWindowOfScreen(self.handle) };
        let (mut ret_root, mut xp, mut yp, mut width, mut height, mut border, mut depth) =
            (0, 0, 0, 0, 0, 0, 0);
        unsafe{ XGetGeometry(
            self.srvr.0, root,
            &mut ret_root,
            &mut xp, &mut yp,
            &mut width, &mut height,
            &mut border, &mut depth) };
        PhysicalPosition::new(xp, yp)
    }

    fn size(&self) -> PhysicalSize {
        let width = unsafe{ XWidthOfScreen(self.handle) };
        let height = unsafe{ XHeightOfScreen(self.handle) };
        PhysicalSize::new(width as u32, height as u32)
    }

    fn dpi(&self) -> Dpi {
        let root = unsafe{ XRootWindowOfScreen(self.handle) };
        let (mut ret_root, mut xp, mut yp, mut width, mut height, mut border, mut depth) =
            (0, 0, 0, 0, 0, 0, 0);
        unsafe{ XGetGeometry(
            self.srvr.0, root,
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
pub struct X11EventLoop {}

impl EventLoopTrait for X11EventLoop {
    fn new() -> Self {
        Self{}
    }

    fn add_window(&mut self, wnd: &WindowImpl) {
    }

    fn run<F>(&mut self, f: F)
        where F: FnMut(&mut ControlFlow, Event) + 'static {
        let srvr = Connection::new();
        let mut e = XEvent::new();
        loop {
            unsafe{ XNextEvent(srvr.0, &mut e) };
        }
    }
}

#[derive(Debug)]
pub struct X11Window {
    srvr: Connection,
    handle: c_ulong,
}

impl WindowTrait for X11Window {
    fn new() -> Self {
        let srvr = Connection::new();
        let screen = unsafe{ XDefaultScreenOfDisplay(srvr.0) };
        let root = unsafe{ XRootWindowOfScreen(screen) };
        let black = unsafe{ XBlackPixel(srvr.0, 0) };
        let white = unsafe{ XWhitePixel(srvr.0, 0) };
        let handle = unsafe{ XCreateSimpleWindow(
            srvr.0,
            root,
            100, 100,
            200, 200,
            1,
            black, white) };
        unsafe{ XSelectInput(srvr.0, handle, ExposureMask) };
        Self{ srvr, handle }
    }

    fn close(&mut self) {
        unsafe{ XUnmapWindow(self.srvr.0, self.handle) };
    }

    fn handle_ptr(&self) -> *mut c_void {
        self.handle as *mut c_void
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
        if vis {
            unsafe{ XMapWindow(self.srvr.0, self.handle) };
        }
        else {
            unsafe{ XUnmapWindow(self.srvr.0, self.handle) };
        }
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