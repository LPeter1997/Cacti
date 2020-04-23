//! TODO: doc

// NOTE: to allow toi build in CI
#![cfg(not(CI))]

use std::io;
use std::ffi::c_void;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

#[derive(Debug)]
pub struct Monitor(MonitorImpl);

impl Monitor {
    pub fn all_monitors() -> Vec<Self> {
        MonitorImpl::all_monitors().into_iter().map(|m| Self(m)).collect()
    }

    pub fn handle_ptr(&self) -> *const c_void { self.0.handle_ptr() }
    pub fn handle_mut_ptr(&mut self) -> *mut c_void { self.0.handle_ptr() }

    pub fn name(&self) -> Option<String> {
        self.0.name()
    }

    pub fn is_primary(&self) -> bool {
        self.0.is_primary()
    }

    pub fn position(&self) -> PhysicalPosition {
        self.0.position()
    }

    pub fn size(&self) -> PhysicalSize {
        self.0.size()
    }

    pub fn dpi(&self) -> Dpi {
        self.0.dpi()
    }

    pub fn scale(&self) -> f64 {
        self.0.scale()
    }
}

#[derive(Debug)]
pub struct EventLoop(EventLoopImpl);

impl EventLoop {
    pub fn new() -> Self { Self(EventLoopImpl::new()) }

    pub fn add_window(&mut self, wnd: &Window) {
        self.0.add_window(&wnd.0);
    }

    pub fn run<F>(&mut self, f: F)
        where F: FnMut(&mut ControlFlow, Event) + 'static {
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

    pub fn monitor(&self) -> Monitor { Monitor(self.0.monitor()) }

    pub fn dpi(&self) -> Dpi { self.monitor().dpi() }
    pub fn scale(&self) -> f64 { self.monitor().scale() }

    pub fn inner_size(&self) -> PhysicalSize {
        self.0.inner_size()
    }

    pub fn outer_size(&self) -> PhysicalSize {
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

    pub fn set_position(&mut self, pos: PhysicalPosition) -> bool {
        self.0.set_position(pos)
    }

    pub fn set_inner_size(&mut self, siz: PhysicalSize) -> bool {
        self.0.set_inner_size(siz)
    }

    pub fn set_outer_size(&mut self, siz: PhysicalSize) -> bool {
        self.0.set_outer_size(siz)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    Poll,
    Wait,
    Exit,
}

#[derive(Debug, PartialEq)]
pub enum Event {
    WindowEvent{
        window_id: WindowId,
        event: WindowEvent,
    },
    LogicUpdate,
    Redraw(WindowId),
    AfterRedraw,
    LoopExited,
}

// TODO: Event for DPI/scale changes
#[derive(Debug, PartialEq)]
pub enum WindowEvent {
    Created,
    CloseRequested,
    Closed,
    FocusChanged(bool),
    Resized(PhysicalSize),
}

// ////////////////////////////////////////////////////////////////////////// //
//                            Size representations                            //
// ////////////////////////////////////////////////////////////////////////// //

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dpi {
    pub horizontal: f64,
    pub vertical: f64,
}

impl Dpi {
    pub fn new(horizontal: f64, vertical: f64) -> Self {
        Self{ horizontal, vertical }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPosition {
    pub x: i32,
    pub y: i32,
}

impl PhysicalPosition {
    pub fn new(x: i32, y: i32) -> Self {
        Self{ x, y }
    }

    pub fn to_logical(&self, scale: f64) -> LogicalPosition {
        LogicalPosition{
            x: self.x as f64 / scale,
            y: self.y as f64 / scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

impl PhysicalSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self{ width, height }
    }

    pub fn to_logical(&self, scale: f64) -> LogicalSize {
        LogicalSize{
            width: self.width as f64 / scale,
            height: self.height as f64 / scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPosition {
    pub x: f64,
    pub y: f64,
}

impl LogicalPosition {
    pub fn new(x: f64, y: f64) -> Self {
        Self{ x, y }
    }

    pub fn to_physical(&self, scale: f64) -> PhysicalPosition {
        PhysicalPosition{
            x: (self.x * scale) as i32,
            y: (self.y * scale) as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    pub fn new(width: f64, height: f64) -> Self {
        Self{ width, height }
    }

    pub fn to_physical(&self, scale: f64) -> PhysicalSize {
        PhysicalSize{
            width: (self.width * scale) as u32,
            height: (self.height * scale) as u32,
        }
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

trait MonitorTrait: Sized {
    fn all_monitors() -> Vec<Self>;

    fn handle_ptr(&self) -> *mut c_void;

    fn name(&self) -> Option<String>;
    fn is_primary(&self) -> bool;
    fn position(&self) -> PhysicalPosition;
    fn size(&self) -> PhysicalSize;
    fn dpi(&self) -> Dpi;
    fn scale(&self) -> f64;
}

trait EventLoopTrait {
    fn new() -> Self;

    fn add_window(&mut self, wnd: &WindowImpl);

    fn run<F>(&mut self, f: F)
        where F: FnMut(&mut ControlFlow, Event) + 'static;
}

trait WindowTrait: Sized {
    fn new() -> Self;

    fn handle_ptr(&self) -> *mut c_void;

    fn monitor(&self) -> MonitorImpl;

    fn inner_size(&self) -> PhysicalSize;
    fn outer_size(&self) -> PhysicalSize;

    fn set_visible(&mut self, vis: bool);
    fn set_resizable(&mut self, res: bool) -> bool;
    fn set_title(&mut self, title: &str) -> bool;
    fn set_position(&mut self, pos: PhysicalPosition) -> bool;
    fn set_inner_size(&mut self, siz: PhysicalSize) -> bool;
    fn set_outer_size(&mut self, siz: PhysicalSize) -> bool;
    fn set_pinned(&mut self, p: bool) -> bool;
    fn set_transparency(&mut self, t: f64) -> bool;
    fn set_fullscreen(&mut self, fs: bool) -> bool;
}

mod win32;
mod x11;

#[cfg(target_os = "windows")]
mod impls {
    use super::win32::*;

    pub type MonitorImpl = Win32Monitor;
    pub type EventLoopImpl = Win32EventLoop;
    pub type WindowImpl = Win32Window;
}

#[cfg(target_os = "linux")]
mod impls {
    use super::x11::*;

    pub type MonitorImpl = X11Monitor;
    pub type EventLoopImpl = X11EventLoop;
    pub type WindowImpl = X11Window;
}

type MonitorImpl = impls::MonitorImpl;
type EventLoopImpl = impls::EventLoopImpl;
type WindowImpl = impls::WindowImpl;

// ////////////////////////////////////////////////////////////////////////// //
//                                   Tests                                    //
// ////////////////////////////////////////////////////////////////////////// //

#[cfg(all(test, not(CI)))]
mod tests {
    use std::rc::Rc;
    use std::cell::RefCell;
    use super::*;

    fn new_event_vec() -> (Rc<RefCell<Vec<Event>>>, Rc<RefCell<Vec<Event>>>) {
        let events = Rc::new(RefCell::new(Vec::new()));
        (events.clone(), events)
    }

    #[test]
    fn test_empty_loop() {
        let (events, events_in) = new_event_vec();
        let mut event_loop = EventLoop::new();
        event_loop.run(move |control_flow, event| {
            *control_flow = ControlFlow::Exit;

            events_in.borrow_mut().push(event);
        });
        assert_eq!(events.borrow().as_slice(), &[]);
    }
}
