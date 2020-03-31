//! Showcasing temporary path operations.

use cacti_fs::temp;

fn main() -> std::io::Result<()> {
    // Generating a unique path in a default temporary place with TXT extension
    {
        let path = temp::path(Some("txt"))?;
        println!("Temporary path is: {:?}", path);
        assert!(!path.exists());
    }

    // Creating a temporary path in the current working directory with TXT
    // extension
    {
        let path = temp::path_in(".", Some("txt"))?;
        println!("Temporary path is: {:?}", path);
        assert!(!path.exists());
    }

    Ok(())
}
