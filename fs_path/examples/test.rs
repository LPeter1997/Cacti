
use std::fs::File;
use fs_path::*;

fn main() -> std::io::Result<()> {
    let f = File::open("C:/TMP/briefing.xml")?;
    println!("Path: {:?}", f.path()?);
    Ok(())
}
