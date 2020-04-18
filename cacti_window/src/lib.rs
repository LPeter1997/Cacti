//! TODO: doc

use std::io;
use std::ffi::c_void;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

// TODO: Solve coordinates problem
// Strong types for coords?

// TODO: Right now this model doesn't make sense. There can be only one window!
// We need to allow multiple windows! But the current loop just sucks!
// Also it would be nicer to be able to modify the window itself in the loop and
// not have a separate thingy for that...

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

    pub fn handle_ptr(&self) -> *const c_void { self.monitor.handle_ptr() }
    pub fn handle_mut_ptr(&mut self) -> *mut c_void { self.monitor.handle_ptr() }

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
pub struct EventLoop(EventLoopImpl);

impl EventLoop {
    pub fn new() -> Self { Self(EventLoopImpl::new()) }

    pub fn add_window(&mut self, wnd: &Window) {
        self.0.add_window(&wnd.0);
    }

    pub fn quit(&mut self, code: u32) {
        self.0.quit(code);
    }

    pub fn run<F>(&mut self, f: F) where F: FnMut(Event) + 'static {
        self.0.run(f);
    }
}

#[derive(Debug)]
pub struct Window(WindowImpl);

impl Window {
    pub fn new() -> Self { Self(WindowImpl::new()) }

    pub fn id(&self) -> WindowId { WindowId(self.handle_ptr()) }
    pub fn handle_ptr(&self) -> *const c_void { self.0.handle_ptr() }
    pub fn handle_mut_ptr(&mut self) -> *mut c_void { self.0.handle_ptr() }

    pub fn inner_size(&self) -> (u32, u32) {
        self.0.inner_size()
    }

    pub fn outer_size(&self) -> (u32, u32) {
        self.0.outer_size()
    }

    pub fn set_visible(&mut self, vis: bool) {
        self.0.set_visible(vis)
    }

    pub fn set_resizable(&mut self, res: bool) -> bool {
        self.0.set_resizable(res)
    }

    pub fn set_title(&mut self, title: &str) -> bool {
        self.0.set_title(title)
    }

    pub fn set_position(&mut self, x: i32, y: i32) -> bool {
        self.0.set_position(x, y)
    }

    pub fn set_inner_size(&mut self, w: u32, h: u32) -> bool {
        self.0.set_inner_size(w, h)
    }

    pub fn set_pinned(&mut self, p: bool) -> bool {
        self.0.set_pinned(p)
    }

    pub fn set_transparency(&mut self, t: f64) -> bool {
        self.0.set_transparency(t)
    }

    pub fn set_fullscreen(&mut self, fs: bool) -> bool {
        self.0.set_fullscreen(fs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(*const c_void);

#[derive(Debug)]
pub enum Event {
    WindowEvent{
        window_id: WindowId,
        event: WindowEvent,
    },
}

#[derive(Debug)]
pub enum WindowEvent {
    Created,
    CloseRequested,
    Closed,
    FocusChanged(bool),
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

trait MonitorTrait: Sized {
    fn all_monitors() -> Vec<Self>;

    fn handle_ptr(&self) -> *mut c_void;

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

trait EventLoopTrait {
    fn new() -> Self;

    fn add_window(&mut self, wnd: &WindowImpl);
    fn quit(&mut self, code: u32);

    fn run<F>(&mut self, f: F) where F: FnMut(Event) + 'static;
}

trait WindowTrait: Sized {
    fn new() -> Self;

    fn handle_ptr(&self) -> *mut c_void;

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
}

mod win32;

#[cfg(target_os = "windows")] use win32 as impls;

type MonitorImpl = impls::Win32Monitor;
type EventLoopImpl = impls::Win32EventLoop;
type WindowImpl = impls::Win32Window;
