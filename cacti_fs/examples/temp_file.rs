//! Showcasing temporary file operations.

use cacti_fs::temp;

fn main() -> std::io::Result<()> {
    // NOTE: We can't check path

    // Creating a temporary TXT file in the default temporary directory
    {
        let _file = temp::file(Some("txt"))?;
    }
    // It must have been deleted by now

    // Creating a temporary TXT file in the current working directory
    {
        let _file = temp::file_in(".", Some("txt"))?;
    }
    // It must have been deleted by now

    // Creating the temporary file "./foo.txt"
    {
        let _file = temp::file_at("./foo.txt")?;
    }
    // It must have been deleted by now

    Ok(())
}
