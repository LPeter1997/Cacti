//! Retrieves the path of a file from an `std::fs::File` handle.

use std::fs;
use std::ffi::OsStr;
use cacti_fs::path::FilePath;

fn main() -> std::io::Result<()> {
    let f = fs::File::create("test.txt")?;
    let path = f.path()?;
    assert!(path.exists());
    assert_eq!(path.file_name(), Some(OsStr::new("test.txt")));
    println!("Full path: {:?}", path);
    fs::remove_file("test.txt")?;
    Ok(())
}
