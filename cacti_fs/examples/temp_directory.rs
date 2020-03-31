//! Showcasing temporary directory operations.

use std::fs;
use cacti_fs::temp;

fn main() -> std::io::Result<()> {
    // Creating a temporary directory in the default temporary directory
    let path;
    {
        let dir = temp::directory()?;
        path = dir.path().to_path_buf();
        println!("Temporary created at {:?}", path);
    }
    // It must have been deleted by now
    assert!(!path.exists());

    // Creating a temporary directory in the current working directory
    let path;
    {
        let dir = temp::directory_in(".")?;
        path = fs::canonicalize(dir.path())?;
        println!("Temporary created at {:?}", path);
    }
    // It must have been deleted by now
    assert!(!path.exists());

    // Creating the temporary directory "./hello"
    let path;
    {
        let dir = temp::directory_at("./hello")?;
        path = fs::canonicalize(dir.path())?;
        assert!(path.ends_with("hello"));
        println!("Temporary created at {:?}", path);
    }
    // It must have been deleted by now
    assert!(!path.exists());

    Ok(())
}
