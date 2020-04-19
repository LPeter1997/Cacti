
use cacti_window::*;
use std::io;

fn main() -> io::Result<()> {
    for monitor in Monitor::all_monitors() {
        println!("name: {:?}, position: {:?}, size: {:?}, DPI: {:?}, scale: {:?}, is primary: {:?}",
            monitor.name(), monitor.position(), monitor.size(), monitor.dpi(), monitor.scale(), monitor.is_primary());
    }

    let mut event_loop = EventLoop::new();

    let mut wnd = Window::new();
    event_loop.add_window(&wnd);
    wnd.set_title("Hello, Window!");
    wnd.set_position(100, 100);
    wnd.set_inner_size(960, 540);
    wnd.set_visible(true);

    event_loop.run(move |control_flow, event| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent{ window_id: _, event: WindowEvent::CloseRequested } => {
                wnd.close();
                *control_flow = ControlFlow::Exit;
            },
            _ => {},
        }
    });

    Ok(())
}
