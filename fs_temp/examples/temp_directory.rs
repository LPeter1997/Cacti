//! Showcasing temporary directory operations.

use std::fs;

fn main() -> std::io::Result<()> {
    // Creating a temporary directory in the default temporary directory
    let path;
    {
        let dir = fs_temp::directory()?;
        path = dir.path().to_path_buf();
        println!("Temporary created at {:?}", path);
        assert!(path.exists());
    }
    // It must have been deleted by now
    assert!(!path.exists());

    // Creating a temporary directory in the current working directory
    let path;
    {
        let dir = fs_temp::directory_in(".")?;
        path = fs::canonicalize(dir.path())?;
        println!("Temporary created at {:?}", path);
        assert!(path.exists());
    }
    // It must have been deleted by now
    assert!(!path.exists());

    Ok(())
}
