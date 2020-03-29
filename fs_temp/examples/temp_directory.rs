//! Showcasing temporary directory operations.

use std::fs;

fn main() -> std::io::Result<()> {
    // Creating a temporary directory in the default temporary directory
    let mut path = None;
    {
        let dir = fs_temp::directory()?;
        let dir_path = dir.path();
        path = Some(dir_path.to_path_buf());
        println!("Temporary created at {:?}", dir_path);
        assert!(dir_path.exists());
    }
    // It must have been deleted by now
    assert!(!path.unwrap().exists());

    // Creating a temporary directory in the current working directory
    let mut path = None;
    {
        let dir = fs_temp::directory_in(".")?;
        let dir_path = fs::canonicalize(dir.path())?;
        path = Some(dir_path.to_path_buf());
        println!("Temporary created at {:?}", dir_path);
        assert!(dir_path.exists());
    }
    // It must have been deleted by now
    assert!(!path.unwrap().exists());

    // Creating the temporary directory "./hello"
    let mut path = None;
    {
        let dir = fs_temp::directory_at("./hello")?;
        let dir_path = fs::canonicalize(dir.path())?;
        assert!(dir_path.ends_with("hello"));
        path = Some(dir_path.to_path_buf());
        println!("Temporary created at {:?}", dir_path);
        assert!(dir_path.exists());
    }
    // It must have been deleted by now
    assert!(!path.unwrap().exists());

    Ok(())
}
