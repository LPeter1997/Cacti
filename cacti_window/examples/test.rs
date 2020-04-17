
use cacti_window::*;
use std::io;

fn main() -> io::Result<()> {
    for monitor in Monitor::all_monitors() {
        println!("name: {:?}, position: {:?}, size: {:?}, DPI: {:?}, scale: {:?}, is primary: {:?}",
            monitor.name(), monitor.position(), monitor.size(), monitor.dpi(), monitor.scale(), monitor.is_primary());
    }

    let mut wnd = Window::new();
    wnd.set_title("Hello, Window!");
    wnd.set_position(100, 100);
    wnd.set_inner_size(960, 540);
    wnd.set_visible(true);
    //wnd.set_resizable(false);
    //wnd.set_pinned(true);
    //wnd.set_transparency(1.0);
    //wnd.set_fullscreen(true);

    wnd.run_event_loop(|| {
        println!("Runn");
    });

    Ok(())
}
