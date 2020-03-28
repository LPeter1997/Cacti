//! A minimalistic, dependency-free, cross-platform library for generating
//! temporary paths, files and directories.
//!
//! The whole library consists of 6 functions and a single type:
//!  * [path](fn.path.html): Returns a unique path for temporaries.
//!  * [path_in](fn.path_in.html): Returns a unique path in a given root
//! directory for temporaries.
//!  * [file](fn.file.html): Creates a temporary file that gets deleted, when
//! it's handle is dropped.
//!  * [file_in](fn.file_in.html): Creates a temporary file in a given root
//! directory that gets deleted, when it's handle is dropped.
//!  * [directory](fn.directory.html): Creates a temporary directory that gets
//! deleted, when it's handle is dropped.
//!  * [directory_in](fn.directory_in.html): Creates a temporary directory in a
//! given root directory that gets deleted with all it's contents, when it's
//! handle is dropped.
//!  * [Directory](type.Directory.html): Represents a directory handle that
//! deletes it's associated directory and all of it's contents, when dropped.
//!
//! # Porting the library to other platforms
//!
//! To port the library to other platforms, take a look at `trait FsTemp`, which
//! only requires the implementation of 2 (and another optional) function:
//!  * `tmp_path`: To return a default temporary directory for the current
//! platform.
//!  * `file_handle`: To create a file handle at a given path, that gets cleaned
//! up by the OS, when the handle is dropped.
//!  * (optional) `unique_in`: To search for a unique dile or directory name
//! inside a given root directory. The default implementation just calls
//! `generate_unique_path`.

use std::io::Result;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::fs;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// Tries to find a path that's usable to create a temporary directory or file
/// at. An optional extension can be supplied - without the dot. The parent
/// directory of the returned path is guaranteed to exist.
///
/// Note, that the function doesn't actually create the directory or file, see
/// [directory](fn.directory.html) and [file](fn.file.html) for such
/// functionality, which also automatically clean up the created directories and
/// files.
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
    path_in(&FsTempImpl::tmp_path()?, extension)
}

/// Tries to find a path inside the given root directory that's usable to create
/// a temporary directory or file at. An optional extension can be supplied -
/// without the dot. The parent of the returned path is guaranteed to be the
/// given root directory.
///
/// Note, that the function doesn't actually create the directory or file, see
/// [directory](fn.directory.html) and [file](fn.file.html) for such
/// functionality, which also automatically clean up the created directories and
/// files.
///
/// # Examples
///
/// Creating a temporary directory inside `C:/TMP`, assuming it exists:
///
/// ```no_run
/// use std::fs;
///
/// # fn main() -> std::io::Result<()> {
/// let temp_dir_path = fs_temp::path_in("C:/TMP", None)?;
/// fs::create_dir(&temp_dir_path)?;
/// // Now we can work inside temp_dir_path!
/// # Ok(())
/// # }
/// ```
///
/// Creating a temporary TXT file inside `C:/TMP`, assuming it exists:
///
/// ```no_run
/// use std::fs;
///
/// # fn main() -> std::io::Result<()> {
/// let temp_file_path = fs_temp::path_in("C:/TMP", Some("txt"))?;
/// let temp_file = fs::File::create(&temp_file_path)?;
/// // Now we can write to temp_file!
/// # Ok(())
/// # }
/// ```
pub fn path_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
    FsTempImpl::unique_in(root.as_ref(), extension)
}

/// Tries to create a temporary file, returning it's handle. An optional
/// extension can be supplied - without the dot. When the returned handle gets
/// dropped, the file is deleted.
///
/// # Examples
///
/// Creating a temporary TXT file:
///
/// ```no_run
/// use std::io::Write;
///
/// # fn main() -> std::io::Result<()> {
/// let mut file = fs_temp::file(Some("txt"))?;
/// // Now we can write to the file!
/// file.write_all("Hello, World!".as_bytes())?;
/// # Ok(())
/// # }
/// ```
pub fn file(extension: Option<&str>) -> Result<File> {
    FsTempImpl::file_handle(&path(extension)?)
}

/// Tries to create a temporary file inside the given root directory, returning
/// it's handle. An optional extension can be supplied - without the dot. When
/// the returned handle gets dropped, the file is deleted. The created file is
/// guaranteed to be directly inside the given root directory.
///
/// # Examples
///
/// Creating a temporary TXT file inside `C:/TMP`, assuming it exists:
///
/// ```no_run
/// use std::io::Write;
///
/// # fn main() -> std::io::Result<()> {
/// let mut file = fs_temp::file_in("C:/TMP", Some("txt"))?;
/// // Now we can write to the file!
/// file.write_all("Hello, World!".as_bytes())?;
/// # Ok(())
/// # }
/// ```
pub fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<File> {
    FsTempImpl::file_handle(&path_in(root, extension)?)
}

/// Tries to create a temporary directory, returning it's handle. When the
/// returned handle gets dropped, the directory and all of it's contents are
/// deleted.
///
/// # Examples
///
/// Creating a temporary directory:
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// let dir = fs_temp::directory()?;
/// // We can work inside the directory now!
/// # Ok(())
/// # }
/// ```
pub fn directory() -> Result<Directory> {
    directory_in(&FsTempImpl::tmp_path()?)
}

/// Tries to create a temporary directory inside the given root directory,
/// returning it's handle. When the returned handle gets dropped, the directory
/// and all of it's contents are deleted. The created directory is guaranteed to
/// be directly inside the given root directory.
///
/// # Examples
///
/// Creating a temporary directory inside `C:/TMP`, assuming it exists:
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// let dir = fs_temp::directory_in("C:/TMP")?;
/// // We can work inside the directory now!
/// # Ok(())
/// # }
/// ```
pub fn directory_in(root: impl AsRef<Path>) -> Result<Directory> {
    let path = path_in(root, None)?;
    fs::create_dir(&path)?;
    Ok(Directory{ path })
}

/// Represents a directory handle, that deletes it's associated directory and
/// all of it's contents, when the handle gets dropped.
#[derive(Debug)]
pub struct Directory {
    path: PathBuf,
}

impl Directory {
    /// Returns the path of this directory.
    pub fn path(&self) -> &Path { &self.path }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

/// The functionality every platform must implement.
trait FsTemp {
    /// Returns a default temporary path for the given platform.
    fn tmp_path() -> Result<PathBuf>;

    /// Creates a file handle that automatically gets deleted when closed.
    fn file_handle(path: &Path) -> Result<File>;

    /// The default unique file/directory name searching strategy for the
    /// platform.
    fn unique_in(root: &Path, extension: Option<&str>) -> Result<PathBuf> {
        generate_unique_path(root, extension)
    }
}

/// A general strategy to find a unique directory or file name in the given
/// root directory.
fn generate_unique_path(root: &Path, extension: Option<&str>) -> Result<PathBuf> {
    use std::io::{Error, ErrorKind};
    use std::time::SystemTime;

    const TRY_COUNT: usize = 256;
    const ITER_COUNT: usize = 4096;

    let postfix = extension.map(|e| format!(".{}", e)).unwrap_or_else(String::new);
    let mut path = root.to_path_buf();

    for _ in 0..TRY_COUNT {
        // Get a timestamp
        // NOTE: Can this unwrap fail?
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        // Construct prefix
        let prefix = format!("tmp_{}_", timestamp);

        for i in 0..ITER_COUNT {
            // Construct the full last part
            let last_part = format!("{}{}{}", prefix, i, postfix);
            // Try to append to the path, if it's unique, we are done
            path.push(last_part);
            if !path.exists() {
                return Ok(path);
            }
            // Not unique
            path.pop();
        }
    }

    Err(Error::new(ErrorKind::TimedOut,
        format!("Could not find unique path in '{:?}'!", root)))
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Path, PathBuf};
    use std::mem::MaybeUninit;
    use std::slice;
    use super::*;

    // Access constants
    const GENERIC_READ : u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    // Share constants
    const FILE_SHARE_DELETE: u32 = 0x00000004;
    // Flags and attributes
    const FILE_ATTRIBUTE_TEMPORARY : u32 = 0x00000100;
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x04000000;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetTempPathW(
            buffer_len: u32     ,
            buffer    : *mut u16,
        ) -> u32;
    }

    /// The Win32 implementation of the library.
    pub struct WinApiTemp;

    impl FsTemp for WinApiTemp {
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

        fn file_handle(path: &Path) -> Result<File> {
            OpenOptions::new()
                .create_new(true)
                .read(true).write(true)
                .access_mode(GENERIC_READ | GENERIC_WRITE)
                .share_mode(FILE_SHARE_DELETE)
                .attributes(FILE_ATTRIBUTE_TEMPORARY)
                .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
                .open(path)
        }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type FsTempImpl = win32::WinApiTemp;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_path() -> Result<()> {
        let p = path(None)?;
        assert!(!p.exists());
        assert!(p.extension() == None);
        let parent = p.parent();
        assert!(parent.is_some());
        assert!(parent.unwrap().exists());
        Ok(())
    }

    #[test]
    fn test_path_with_extension() -> Result<()> {
        let p = path(Some("txt"))?;
        assert!(!p.exists());
        assert!(p.extension() == Some(&OsString::from("txt")));
        let parent = p.parent();
        assert!(parent.is_some());
        assert!(parent.unwrap().exists());
        Ok(())
    }

    #[test]
    fn test_path_in() -> Result<()> {
        // We kinda depend on `path` for this
        let root = path(None)?;
        let p = path_in(&root, None)?;
        assert!(!p.exists());
        assert!(p.extension() == None);
        let parent = p.parent();
        assert!(parent.is_some());
        assert_eq!(parent.unwrap(), root);
        Ok(())
    }

    #[test]
    fn test_file() -> Result<()> {
        // NOTE: We can't get file path
        {
            let _f = file(Some("txt"))?;
        }
        // NOTE: We can't detect if it was deleted
        Ok(())
    }

    // NOTE: Test file_in?

    #[test]
    fn test_directory() -> Result<()> {
        let mut dpath = None;
        {
            let d = directory()?;
            dpath = Some(d.path().to_path_buf());
            assert!(dpath.as_ref().unwrap().exists());
        }
        assert!(!dpath.unwrap().exists());
        Ok(())
    }

    // NOTE: Test directory_in?
}
