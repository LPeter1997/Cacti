//! A minimalistic, dependency-free, cross-platform library for generating
//! temporary file and directory paths.
//!
//! The library consists of 2 functions:
//!  * [path](fn.path.html): For a unique path in a general, OS-specific
//! location.
//!  * [path_in](fn.path_in.html): For a unique path in a given root.

use std::io::Result;
use std::path::{Path, PathBuf};

/// Tries to find a path that's usable to create a temporary directory at. An
/// optional extension can be supplied - without the dot - to generate a file
/// path. The parent directory is guaranteed to exist.
///
/// Note, that the function doesn't actually create the directory or file.
///
/// # Examples
///
/// Creating a temporary directory:
///
/// ```no_run
/// use std::fs;
///
/// # fn main() -> std::io::Result<()> {
/// let temp_dir_path = fs_temp::path(None)?;
/// fs::create_dir(&temp_dir_path)?;
/// // Now we can work inside temp_dir_path!
/// # Ok(())
/// # }
/// ```
///
/// Creating a temporary TXT file:
///
/// ```no_run
/// use std::fs;
///
/// # fn main() -> std::io::Result<()> {
/// let temp_file_path = fs_temp::path(Some("txt"))?;
/// let temp_file = fs::File::create(&temp_file_path)?;
/// // Now we can write to temp_file!
/// # Ok(())
/// # }
/// ```
pub fn path(extension: Option<&str>) -> Result<PathBuf> {
    functions::path(extension)
}

/// Tries to find a path under the given root that's usable to create a
/// temporary directory. An optional extension can be supplied - without the dot -
/// to generate a file path. The parent directory is guaranteed to exist.
///
/// Note, that the function doesn't actually create the directory or file.
///
/// # Examples
///
/// Creating a temporary directory inside our working directory:
///
/// ```no_run
/// use std::fs;
///
/// # fn main() -> std::io::Result<()> {
/// let temp_dir_path = fs_temp::path_in(".", None)?;
/// fs::create_dir(&temp_dir_path)?;
/// // Now we can work inside temp_dir_path!
/// # Ok(())
/// # }
/// ```
///
/// Creating a temporary TXT file inside our working directory:
///
/// ```no_run
/// use std::fs;
///
/// # fn main() -> std::io::Result<()> {
/// let temp_file_path = fs_temp::path_in(".", Some("txt"))?;
/// let temp_file = fs::File::create(&temp_file_path)?;
/// // Now we can write to temp_file!
/// # Ok(())
/// # }
/// ```
pub fn path_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
    functions::path_in(root, extension)
}

// WinAPI implementation ///////////////////////////////////////////////////////

// NOTE: path_in is kind of platform-independent...

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
                let subdir = f(timestamp, i);
                // Combine
                root.push(subdir);
                if !root.exists() {
                    return root;
                }
                // Already exists, remove the added portion
                root.pop();
            }
        }
    }

    pub fn path_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
        let root = root.as_ref().to_path_buf();
        if let Some(extension) = extension {
            Ok(unique_path(root,
                |timestamp, i| format!("tmp_{}_{}.{}", timestamp, i, extension)))
        }
        else {
            Ok(unique_path(root,
                |timestamp, i| format!("tmp_{}_{}", timestamp, i)))
        }
    }

    pub fn path(extension: Option<&str>) -> Result<PathBuf> {
        let root = tmp_path()?;
        if let Some(extension) = extension {
            Ok(unique_path(root,
                |timestamp, i| format!("tmp_{}_{}.{}", timestamp, i, extension)))
        }
        else {
            Ok(unique_path(root,
                |timestamp, i| format!("tmp_{}_{}", timestamp, i)))
        }
    }
}

// Defaults for OSes inside `functions` module.

#[cfg(target_os = "windows")]
mod functions {
    use super::win32;
    pub use win32::path;
    pub use win32::path_in;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_no_ext() -> Result<()> {
        let fpath = path(None)?;
        assert!(!fpath.exists());
        let parent = fpath.parent();
        assert!(parent.is_some());
        assert!(parent.unwrap().exists());
        Ok(())
    }

    #[test]
    fn test_path_with_ext() -> Result<()> {
        let fpath = path(Some("txt"))?;
        assert!(!fpath.exists());
        assert_eq!(fpath.extension().and_then(|p| p.to_str()), Some("txt"));
        let parent = fpath.parent();
        assert!(parent.is_some());
        assert!(parent.unwrap().exists());
        Ok(())
    }

    #[test]
    fn test_path_in_root() -> Result<()> {
        let root_path = path(None)?;
        let fpath = path_in(&root_path, None)?;
        assert!(!fpath.exists());
        let parent = fpath.parent();
        assert!(parent.is_some());
        let parent = parent.unwrap();
        assert_eq!(parent, &root_path);
        Ok(())
    }
}
