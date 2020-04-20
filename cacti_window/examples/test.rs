
use cacti_window::*;
use std::io;

#[cfg(not(CI))]
fn main() -> io::Result<()> {
    for monitor in Monitor::all_monitors() {
        println!("name: {:?}, position: {:?}, size: {:?}, DPI: {:?}, scale: {:?}, is primary: {:?}",
            monitor.name(), monitor.position(), monitor.size(), monitor.dpi(), monitor.scale(), monitor.is_primary());
    }

    let mut event_loop = EventLoop::new();

    let mut wnd = Window::new();
    event_loop.add_window(&wnd);
    wnd.set_title("Hello, Window!");
    /*wnd.set_inner_size(PhysicalSize::new(960, 540));*/
    wnd.set_position(PhysicalPosition::new(400, 600));
    wnd.set_visible(true);

    event_loop.run(move |control_flow, event| {
        /*println!("{:?}", event);

        match event {
            Event::WindowEvent{ window_id: _, event: WindowEvent::CloseRequested } => {
                wnd.close();
                // I really dislike this design tho...
                *control_flow = ControlFlow::Exit;
            },
            _ => {},
        }*/
    });

    Ok(())
}

#[cfg(CI)]
fn main() {
}
