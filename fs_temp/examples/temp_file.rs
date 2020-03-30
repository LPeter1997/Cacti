//! Showcasing temporary file operations.

fn main() -> std::io::Result<()> {
    // NOTE: We can't check path

    // Creating a temporary file in the default temporary directory
    {
        let _file = fs_temp::file()?;
    }
    // It must have been deleted by now

    // Creating a temporary file in the current working directory
    {
        let _file = fs_temp::file_in(".")?;
    }
    // It must have been deleted by now

    Ok(())
}
