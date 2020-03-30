//! Cross-platform utility for creating unique temporary files and directories.
//!
//! The whole library consists of a handful of functions and a single type:
//!  * [file](fn.file.html): Creates a temporary file that gets deleted when
//! it's handle is dropped.
//!  * [file_in](fn.file_in.html): Creates a temporary file in a given root
//! directory that gets deleted when it's handle is dropped.
//!  * [directory](fn.directory.html): Creates a temporary directory that gets
//! deleted with all it's contents when it's handle is dropped.
//!  * [directory_in](fn.directory_in.html): Creates a temporary directory in a
//! given root directory that gets deleted with all it's contents when it's
//! handle is dropped.
//!  * [Directory](struct.Directory.html): Represents a directory handle that
//! deletes it's associated directory and all of it's contents, when dropped.
//!
//! # Usage
//!
//! For detailed usage, read the documentation of the individual functions.
//!
//! # Porting the library to other platforms
//!
//! To port this library to other platforms, the `trait FsTemp` has to be
//! implemented for a type and have it aliased as `FsTempImpl` in global scope
//! for the platform:
//!
//! ```no_run
//! # use std::io::Result;
//! # use std::path::{Path, PathBuf};
//! # use std::fs;
//! # trait FsTemp {
//! #    type Directory: std::fmt::Debug;
//! #    fn temp_path() -> Result<PathBuf>;
//! #    fn temp_file_in(root: &Path, extension: Option<&str>) -> Result<fs::File>;
//! #    fn temp_dir_in(root: &Path) -> Result<Self::Directory>;
//! # }
//! #[cfg(target_os = "new_platform")]
//! mod my_platform {
//!     struct MyPlatformTemp;
//!
//!     impl FsTemp for MyPlatformTemp {
//!         /// The internal directory handle for this platform.
//!         type Directory = MyDirectory;
//!
//!         /// Here you need to return some safe temporary directory path for
//!         /// the platform.
//!         fn temp_path() -> Result<PathBuf> {
//!             // ...
//! # unimplemented!()
//!         }
//!
//!         /// Here you should create a file in the given root directory and
//!         /// return a handle that deletes it when dropped. The optional
//!         /// extension is supplied without the dot.
//!         fn temp_file_in(root: &Path, extension: Option<&str>) -> Result<fs::File> {
//!             // ...
//! # unimplemented!()
//!         }
//!
//!         /// Here you should create a directory in the given root directory
//!         /// and return the defined handle deletes it when dropped.
//!         fn temp_dir_in(root: &Path) -> Result<Self::Directory> {
//!             // ...
//! # unimplemented!()
//!         }
//!     }
//!
//!     /// Must implement `std::fmt::Debug`, also must delete the associated
//!     /// directory, when dropped.
//!     #[derive(Debug)]
//!     struct MyDirectory { /* .. */ }
//!
//!     impl MyDirectory {
//!         /// Your directory handle must define a `path` method that returns
//!         /// the `&Path` of the represented directory.
//!         fn path(&self) -> &Path {
//!             // ...
//! # unimplemented!()
//!         }
//!     }
//! }
//!
//! #[cfg(target_os = "new_platform")] type FsTempImpl = my_platform::MyPlatformTemp;
//! ```
//!
//! If you need a thread-safe unique path-searching algorithm, take a look at
//! the internal function `unique_path_with_timestamp`. You can supply the
//! thread ID as the `extra` parameter.

use std::io::Result;
use std::path::{Path, PathBuf};
use std::fs;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// Tries to create a temporary file at some default place, returning it's
/// handle. An optional extension can be supplied - without the dot. When the
/// returned handle gets dropped, the file is deleted.
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
pub fn file(extension: Option<&str>) -> Result<fs::File> {
    file_in(&FsTempImpl::temp_path()?, extension)
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
pub fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<fs::File> {
    FsTempImpl::temp_file_in(root.as_ref(), extension)
}

/// Tries to create a temporary directory at some default place, returning it's
/// handle. When the returned handle gets dropped, the directory and all of it's
/// contents are deleted.
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
    directory_in(&FsTempImpl::temp_path()?)
}

/// Tries to create a temporary directory inside the given root directory,
/// returning it's handle. When the returned handle gets dropped, the directory
/// and all of it's contents are deleted.
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
    Ok(Directory(FsTempImpl::temp_dir_in(root.as_ref())?))
}

/// Represents a handle for a directory created by one of the directory
/// functions.
#[derive(Debug)]
pub struct Directory(<FsTempImpl as FsTemp>::Directory);

impl Directory {
    /// Returns the path of this directory handle.
    pub fn path(&self) -> &Path { self.0.path() }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

/// The functionality every platform must implement.
trait FsTemp {
    /// The type of the directory handle this platform provides.
    type Directory: std::fmt::Debug;

    /// Returns a default temporary path for the given platform.
    fn temp_path() -> Result<PathBuf>;

    /// Creates a file handle in the given root directory that automatically
    /// gets deleted, when closed. An optional extension can be supplied without
    /// the dot.
    fn temp_file_in(root: &Path, extension: Option<&str>) -> Result<fs::File>;

    /// Creates a directory handle in the given root directory that
    /// automatically gets deleted, when closed.
    fn temp_dir_in(root: &Path) -> Result<Self::Directory>;
}

// A general, timestamp-based unique path-finder.
fn unique_path_with_timestamp(root: &Path, extra: u64, extension: Option<&str>) -> Result<PathBuf> {
    use std::io::{Error, ErrorKind};
    use std::time::SystemTime;

    const TRY_COUNT: usize = 256;
    const ITER_COUNT: usize = 4096;

    let extension = extension.map(|e| format!(".{}", e)).unwrap_or_else(String::new);
    let postfix = format!("_{}{}", extra, extension);
    let mut path = root.to_path_buf();

    for _ in 0..TRY_COUNT {
        // Get a timestamp
        // NOTE: Can this unwrap fail?
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        // Construct prefix
        let prefix = format!("tmp_{}", timestamp);

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

// Unsupported implementation //////////////////////////////////////////////////

mod unsupported {
    #![allow(dead_code)]

    use std::io::{Error, ErrorKind};
    use super::*;

    pub struct UnsupportedTemp;

    impl FsTemp for UnsupportedTemp {
        type Directory = UnsupportedDirectory;

        fn temp_path() -> Result<PathBuf> {
            Err(Error::new(ErrorKind::Other,
                "Temporary file paths are not supported on this platform!"))
        }

        fn temp_file_in(_root: &Path, _extension: Option<&str>) -> Result<fs::File> {
            Err(Error::new(ErrorKind::Other,
                "Temporary files are not supported on this platform!"))
        }

        fn temp_dir_in(_root: &Path) -> Result<Self::Directory> {
            Err(Error::new(ErrorKind::Other,
                "Temporary directories are not supported on this platform!"))
        }
    }

    #[derive(Debug)]
    pub struct UnsupportedDirectory;

    impl UnsupportedDirectory {
        pub fn path(&self) -> &Path { unimplemented!() }
    }
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{OsStr, OsString, c_void};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::io::FromRawHandle;
    use std::path::{Path, PathBuf};
    use std::ptr;
    use std::io;
    use super::*;

    // Access constants
    const GENERIC_READ : u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    // Share constants
    const FILE_SHARE_DELETE: u32 = 0x00000004;
    // Creation and disposition
    const CREATE_NEW   : u32 = 1;
    const OPEN_EXISTING: u32 = 3;
    // Flags and attributes
    const FILE_ATTRIBUTE_TEMPORARY  : u32 = 0x00000100;
    const FILE_FLAG_DELETE_ON_CLOSE : u32 = 0x04000000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    // Returned by handle-returning functions on failure
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetTempPathW(
            buffer_len: u32     ,
            buffer    : *mut u16,
        ) -> u32;

        fn CreateFileW(
            name     : *const u16 ,
            access   : u32        ,
            share    : u32        ,
            security : *mut c_void,
            crea_disp: u32        ,
            attribs  : u32        ,
            template : *mut c_void,
        ) -> *mut c_void;

        fn CreateDirectoryW(
            name    : *const u16 ,
            security: *mut c_void,
        ) -> i32;

        fn CloseHandle(handle: *mut c_void) -> i32;

        fn GetCurrentThreadId() -> u32;
    }

    /// Converts the Rust &OsStr into a WinAPI `WCHAR` string.
    fn to_wstring(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0).into_iter()).collect()
    }

    /// The Win32 implementation of the `FsTemp` trait.
    pub struct WinApiTemp;

    impl FsTemp for WinApiTemp {
        type Directory = WinApiDirectory;

        fn temp_path() -> Result<PathBuf> {
            // Ask for the buffer size
            let required_size = unsafe { GetTempPathW(
                0,
                ptr::null_mut()) };
            if required_size == 0 {
                return Err(io::Error::last_os_error());
            }
            // Allocate
            let mut buffer = vec![0u16; required_size as usize];
            // Fill
            let written_size = unsafe { GetTempPathW(
                required_size,
                buffer.as_mut_ptr()) };
            if written_size == 0 || written_size > required_size {
                return Err(io::Error::last_os_error());
            }
            // Remove 0-terminator
            let buffer = &buffer[..(written_size as usize)];
            Ok(OsString::from_wide(buffer).into())
        }

        fn temp_file_in(root: &Path, extension: Option<&str>) -> Result<fs::File> {
            // Generate path
            let extra = unsafe { GetCurrentThreadId() };
            let path = unique_path_with_timestamp(root, extra as u64, extension)?;
            let path = to_wstring(path.as_os_str());
            // Actually create
            let handle = unsafe { CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_DELETE,
                ptr::null_mut(),
                CREATE_NEW,
                FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE,
                ptr::null_mut()) };
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe{ fs::File::from_raw_handle(handle) })
        }

        fn temp_dir_in(root: &Path) -> Result<Self::Directory> {
            // Generate path
            let extra = unsafe { GetCurrentThreadId() };
            let path = unique_path_with_timestamp(root, extra as u64, None)?;
            let wpath = to_wstring(path.as_os_str());
            // First create the directory
            let result = unsafe { CreateDirectoryW(
                wpath.as_ptr(),
                ptr::null_mut()) };
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
            // Now the trickery, open with `CreateFileW` so the OS can delete it
            // even when the program gets interrupted
            let handle = unsafe { CreateFileW(
                wpath.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_DELETE,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE | FILE_FLAG_BACKUP_SEMANTICS,
                ptr::null_mut()) };
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            Ok(WinApiDirectory{ handle, path: path.to_path_buf() })
        }
    }

    /// Win32 directory handle type.
    #[derive(Debug)]
    pub struct WinApiDirectory {
        handle: *mut c_void,
        path: PathBuf,
    }

    impl Drop for WinApiDirectory {
        fn drop(&mut self) {
            if self.handle == INVALID_HANDLE_VALUE {
                return;
            }
            unsafe { CloseHandle(self.handle) };
            self.handle = INVALID_HANDLE_VALUE;
        }
    }

    impl WinApiDirectory {
        pub fn path(&self) -> &Path { &self.path }
    }
}

// Linux implementation ////////////////////////////////////////////////////////

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    /// The Linux implementation of the `FsTemp` trait.
    pub struct LinuxTemp;

    impl FsTemp for LinuxTemp {
        type Directory = LinuxDirectory;

        fn temp_path() -> Result<PathBuf> {
            Ok(PathBuf::from("/tmp"))
        }

        fn temp_file_in(_root: &Path, _extension: Option<&str>) -> Result<fs::File> {
            unimplemented!()
        }

        fn temp_dir_in(_root: &Path) -> Result<Self::Directory> {
            unimplemented!()
        }
    }

    /// Linux directory handle type.
    #[derive(Debug)]
    pub struct LinuxDirectory {
        path: PathBuf,
    }

    impl LinuxDirectory {
        pub fn path(&self) -> &Path { &self.path }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type FsTempImpl = win32::WinApiTemp;
#[cfg(target_os = "linux")] type FsTempImpl = linux::LinuxTemp;
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
)))] type FsTempImpl = unsupported::UnsupportedTemp;

#[cfg(test)]
mod tests {
    use super::*;
    use fs_path::FilePath;
    use std::ffi::OsString;

    #[test]
    fn test_file() -> Result<()> {
        let path;
        {
            let file = file(Some("txt"))?;
            path = file.path()?;
            assert!(path.exists());
            assert!(path.extension() == Some(&OsString::from("txt")));
        }
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn test_file_in() -> Result<()> {
        let path;
        {
            let file = file_in(".", Some("txt"))?;
            path = file.path()?;
            assert_eq!(
                fs::canonicalize(path.parent().unwrap())?,
                fs::canonicalize(".")?
            );
            assert!(path.exists());
            assert!(path.extension() == Some(&OsString::from("txt")));
        }
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn test_directory() -> Result<()> {
        let path;
        {
            let dir = directory()?;
            path = dir.path().to_path_buf();
            assert!(path.exists());
        }
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn test_directory_in() -> Result<()> {
        let path;
        {
            let dir = directory_in(".")?;
            path = dir.path().to_path_buf();
            assert_eq!(
                fs::canonicalize(path.parent().unwrap())?,
                fs::canonicalize(".")?
            );
            assert!(path.exists());
        }
        assert!(!path.exists());
        Ok(())
    }
}
