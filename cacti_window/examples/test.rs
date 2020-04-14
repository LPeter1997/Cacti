
use cacti_window::*;
use std::io;

fn main() -> io::Result<()> {
    for monitor in Monitor::all_monitors() {
        println!("position: {:?}, resolution: {:?}, DPI: {:?}, scale: {:?}, is primary: {:?}",
            monitor.position(), monitor.resolution(), monitor.dpi(), monitor.scale(), monitor.is_primary());
    }
    Ok(())
}
