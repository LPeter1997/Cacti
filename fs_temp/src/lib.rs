//! A minimalistic, dependency-free, cross-platform library for creating
//! temporary files and directories.

use std::path::{Path, PathBuf};

/// The `Result` type of this library.
pub type Result<T> = std::io::Result<T>;

/// Tries to find a path that's usable for creating a temporary directory at.
///
/// Note, that the function doesn't actually create the directory.
///
/// # Examples
///
/// Let's say we want to extract a ZIP archive, but don't want to keep the files
/// for long-term usage. We can use `directory` to find us a path that's great
/// for giving us a path where we can unzip it.
///
/// ```no_run
/// use std::fs;
/// # use std::path::Path;
///
/// # fn extract_stuff(from: impl AsRef<Path>, to: impl AsRef<Path>) {}
/// # fn main() -> std::io::Result<()> {
/// let unzip_path = fs_temp::directory()?;
/// // We have to create the path ourselves
/// fs::create_dir(&unzip_path)?;
/// extract_stuff("secret_memes.zip", &unzip_path);
/// # Ok(())
/// # }
/// ```
pub fn directory() -> Result<PathBuf> {
    functions::directory()
}

/// Tries to find a path in the given root that's usable for creating a
/// temporary directory at.
///
/// Note, that the function doesn't actually create the directory.
///
/// # Examples
///
/// Let's say that our tests need a temporary file-structure. This means a
/// unique directory for each test-case. To make cleanup easier, we'd like to
/// create our test directories in a common parent directory.
///
/// ```no_run
/// use std::fs;
/// # use std::path::Path;
///
/// # fn main() -> std::io::Result<()> {
/// let test_case_workdir = fs_temp::directory_in("test_root")?;
/// // We have to create the path ourselves
/// fs::create_dir(&test_case_workdir)?;
/// // Now we can copy our file-structure in for testing
/// # Ok(())
/// # }
/// ```
pub fn directory_in(root: impl AsRef<Path>) -> Result<PathBuf> {
    functions::directory_in(root)
}

/// Tries to find a path that's usable to create a temporary file. An optional
/// extension can be supplied - without the dot.
///
/// Note, that the function doesn't actually create the file.
///
/// # Examples
///
/// When we want to download a file for short-term usage.
///
/// ```no_run
/// # use std::path::Path;
/// # fn download(link: &str, p: impl AsRef<Path>) {}
/// # fn main() -> std::io::Result<()> {
/// // We want to download a cute puppy picture to mirror it
/// let temp_file = fs_temp::file(Some("jpg"))?;
/// // Now we can use temp_file as a save path
/// download("<some link>", temp_file);
/// // Don't forget to delete the file, it's not nice to litter!
/// # Ok(())
/// # }
/// ```
pub fn file(extension: Option<&str>) -> Result<PathBuf> {
    functions::file(extension)
}

/// Tries to find a path under the given root that's usable to create a
/// temporary file. An optional extension can be supplied - without the dot.
///
/// Note, that the function doesn't actually create the file.
///
/// # Examples
///
/// We want a file for short-term usage only in a specific location, probably in
/// our work-directory.
///
/// ```no_run
/// # use std::path::Path;
/// # fn download(link: &str, p: impl AsRef<Path>) {}
/// # fn main() -> std::io::Result<()> {
/// # const WORK_DIR: &str = "";
/// // We want to download a cute puppy picture to mirror it
/// let temp_file = fs_temp::file_in(WORK_DIR, None)?;
/// // Now we can use temp_file as a save path for example
/// download("<some link>", temp_file);
/// # Ok(())
/// # }
/// ```
pub fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
    functions::file_in(root, extension)
}

// WinAPI implementation ///////////////////////////////////////////////////////

// NOTE: file_in and directory_in are kind of platform-independent...

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;
    use std::mem::MaybeUninit;
    use std::slice;
    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetTempPathW(
            buffer_len: u32     ,
            buffer    : *mut u16,
        ) -> u32;
    }

    /// Returns what Windows thinks is a good root-directory for temporaries,
    /// based on `GetTempPathW`.
    fn tmp_path() -> Result<PathBuf> {
        const BUFFER_SIZE: usize = 261;

        let mut buffer: MaybeUninit<[u16; BUFFER_SIZE]> = MaybeUninit::uninit();
        let filled = unsafe { GetTempPathW(
            BUFFER_SIZE as u32,
            buffer.as_mut_ptr().cast()) };

        if filled == 0 {
            Err(std::io::Error::last_os_error())
        }
        else {
            let buffer: &[u16] = unsafe { slice::from_raw_parts(
                buffer.as_ptr().cast(),
                filled as usize) };
            Ok(OsString::from_wide(buffer).into())
        }
    }

    /// Creates a unique path in the given root, using the given formatter
    /// function. The formatter function creates the last postion of the path
    /// based on a timestamp and trial-index.
    fn unique_path<F>(mut root: PathBuf, mut f: F) -> PathBuf
        where F: FnMut(u128, usize) -> String {

        const TRY_COUNT: usize = 16384;

        loop {
            // NOTE: Can this `unwrap` ever fail?
            // Generate a timestamp
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
            for i in 0..TRY_COUNT {
                // Combine the the timestamp and trial-index
                let subfolder = f(timestamp, i);
                // Combine
                root.push(subfolder);
                if !root.exists() {
                    return root;
                }
                // Already exists, remove the added portion
                root.pop();
            }
        }
    }

    pub fn directory_in(root: impl AsRef<Path>) -> Result<PathBuf> {
        let root = root.as_ref().to_path_buf();
        Ok(unique_path(root, |timestamp, i| format!("tmp_{}_{}", timestamp, i)))
    }

    pub fn directory() -> Result<PathBuf> {
        let root = tmp_path()?;
        Ok(unique_path(root, |timestamp, i| format!("tmp_{}_{}", timestamp, i)))
    }

    pub fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
        let root = root.as_ref().to_path_buf();
        let extension = extension.unwrap_or("TMP");
        Ok(unique_path(root,
            |timestamp, i| format!("tmp_{}_{}.{}", timestamp, i, extension)))
    }

    pub fn file(extension: Option<&str>) -> Result<PathBuf> {
        let root = tmp_path()?;
        let extension = extension.unwrap_or("TMP");
        Ok(unique_path(root,
            |timestamp, i| format!("tmp_{}_{}.{}", timestamp, i, extension)))
    }
}

// Defaults for OSes inside `functions` module.

#[cfg(target_os = "windows")]
mod functions {
    use super::win32;
    pub use win32::directory_in;
    pub use win32::file_in;
    pub use win32::directory;
    pub use win32::file;
}
