
use cacti_window::*;
use std::io;

fn main() -> io::Result<()> {
    for monitor in Monitor::all_monitors() {
        println!("name: {:?}, position: {:?}, size: {:?}, DPI: {:?}, scale: {:?}, is primary: {:?}",
            monitor.name(), monitor.position(), monitor.size(), monitor.dpi(), monitor.scale(), monitor.is_primary());
    }
    Ok(())
}
