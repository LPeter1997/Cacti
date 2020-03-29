//! Showcasing temporary path operations.

fn main() -> std::io::Result<()> {
    // Generating a unique path in a default temporary place with TXT extension
    {
        let path = fs_temp::path(Some("txt"))?;
        println!("Temporary path is: {:?}", path);
        assert!(!path.exists());
    }

    // Creating a temporary path in the current working directory with TXT
    // extension
    {
        let path = fs_temp::path_in(".", Some("txt"))?;
        println!("Temporary path is: {:?}", path);
        assert!(!path.exists());
    }

    Ok(())
}
