//! Cross-platform utility for generating unique temporary paths, files and
//! directories.
//!
//! The whole library consists of a handful of functions and a single type:
//!  * [path](fn.path.html): Returns a unique path for temporaries.
//!  * [path_in](fn.path_in.html): Returns a unique path in a given root
//! directory for temporaries.
//!  * [file](fn.file.html): Creates a temporary file that gets deleted, when
//! it's handle is dropped.
//!  * [file_in](fn.file_in.html): Creates a temporary file in a given root
//! directory that gets deleted, when it's handle is dropped.
//!  * [file_at](fn.file_at.html): Creates a temporary file at the given path
//! that gets deleted, when it's handle is dropped.
//!  * [directory](fn.directory.html): Creates a temporary directory that gets
//! deleted, when it's handle is dropped.
//!  * [directory_in](fn.directory_in.html): Creates a temporary directory in a
//! given root directory that gets deleted with all it's contents, when it's
//! handle is dropped.
//!  * [directory_at](fn.directory_at.html): Creates a temporary directory at
//! the given path that gets deleted with all it's contents, when it's handle is
//! dropped.
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
//! #    fn temp_file(path: &Path) -> Result<fs::File>;
//! #    fn temp_dir(path: &Path) -> Result<Self::Directory>;
//! #    fn unique_path_in(root: &Path, extension: Option<&str>) -> Result<PathBuf>;
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
//!         /// Here you should create a file at the given path and return a
//!         /// handle that deletes it when dropped.
//!         fn temp_file(path: &Path) -> Result<fs::File> {
//!             // ...
//! # unimplemented!()
//!         }
//!
//!         /// Here you should create a directory at the given path and return
//!         /// the defined handle deletes it when dropped.
//!         fn temp_dir(path: &Path) -> Result<Self::Directory> {
//!             // ...
//! # unimplemented!()
//!         }
//!
//!         /// Here you should provide a strategy to search for a unique path
//!         /// inside the given root and with the given optional extension.
//!         ///
//!         /// You can use the general `unique_path_with_timestamp` strategy,
//!         /// that uses a timestamp. If you use that, make sure to pass in
//!         /// some unique thread identifier as the `extra` parameter, to keep
//!         /// things thread-safe.
//!         fn unique_path_in(root: &Path, extension: Option<&str>) -> Result<PathBuf> {
//!             // ...
//! # unimplemented!()
//!         }
//!     }
//!
//!     /// Must implement std::fmt::Debug, also must delete the associated
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

use std::io::Result;
use std::path::{Path, PathBuf};
use std::fs;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// Tries to find a path that's usable to create a temporary directory or file
/// at. An optional extension can be supplied - without the dot. The parent
/// directory of the returned path is guaranteed to exist.
///
/// **Note:** The function doesn't actually create the directory or file, see
/// [directory](fn.directory.html) and [file](fn.file.html) for such
/// functionality. There are more differences than just calling
/// `fs::create_dir(path(...))`.
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
///
/// Please note, that neither of the examples clean up the created files and
/// directories.
pub fn path(extension: Option<&str>) -> Result<PathBuf> {
    path_in(&FsTempImpl::temp_path()?, extension)
}

/// Tries to find a path inside the given root directory that's usable to create
/// a temporary directory or file at. An optional extension can be supplied -
/// without the dot. The parent of the returned path is guaranteed to be the
/// given root directory.
///
/// **Note:** The function doesn't actually create the directory or file, see
/// [directory_in](fn.directory_in.html) and [file_in](fn.file_in.html) for such
/// functionality. There are more differences than just calling
/// `fs::create_dir(path_in(...))`.
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
///
/// Please note, that neither of the examples clean up the created files and
/// directories.
pub fn path_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
    FsTempImpl::unique_path_in(root.as_ref(), extension)
}

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
    file_at(&path_in(root, extension)?)
}

/// Tries to create a temporary file at the exact path. When the returned handle
/// gets dropped, the file is deleted.
///
/// # Examples
///
/// Creating a temporary TXT file named `test.txt`:
///
/// ```no_run
/// use std::io::Write;
///
/// # fn main() -> std::io::Result<()> {
/// {
///     let mut file = fs_temp::file_at("test.txt")?;
///     // Now we can write to the file!
///     file.write_all("Hello, World!".as_bytes())?;
/// }
/// // Here the file is deleted!
/// # Ok(())
/// # }
/// ```
pub fn file_at(full_path: impl AsRef<Path>) -> Result<fs::File> {
    FsTempImpl::temp_file(full_path.as_ref())
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
    directory_at(&path_in(root, None)?)
}

/// Tries to create a temporary directory at the exact path. When the returned
/// handle gets dropped, the directory and all of it's contents are deleted.
///
/// # Examples
///
/// Creating a temporary directory named `cache`:
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// {
///     let dir = fs_temp::directory_at("cache")?;
///     // We can work inside the directory now!
/// }
/// // Here the directory is deleted!
/// # Ok(())
/// # }
/// ```
pub fn directory_at(full_path: impl AsRef<Path>) -> Result<Directory> {
    Ok(Directory(FsTempImpl::temp_dir(full_path.as_ref())?))
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

    /// Creates a file handle at the given path that automatically gets deleted,
    /// when closed.
    fn temp_file(path: &Path) -> Result<fs::File>;

    /// Creates a directory handle at the given path that automatically gets
    /// deleted, when closed.
    fn temp_dir(path: &Path) -> Result<Self::Directory>;

    /// The default unique file/directory name searching strategy for the
    /// platform. Tries to search a unique file or directory name in the given
    /// root directory, with a given an optional extension.
    fn unique_path_in(root: &Path, extension: Option<&str>) -> Result<PathBuf>;
}

// A general, timestamp-based unique path-finder.
fn unique_path_with_timestamp<E>(root: &Path, extension: Option<&str>, extra: E) -> Result<PathBuf>
    where E: std::fmt::Display {

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
        let prefix = format!("tmp_{}_{}_", timestamp, extra);

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

        fn temp_file(_path: &Path) -> Result<fs::File> {
            Err(Error::new(ErrorKind::Other,
                "Temporary files are not supported on this platform!"))
        }

        /// Creates a directory handle at the given path that automatically gets
        /// deleted, when closed.
        fn temp_dir(_path: &Path) -> Result<Self::Directory> {
            Err(Error::new(ErrorKind::Other,
                "Temporary directories are not supported on this platform!"))
        }

        /// The default unique file/directory name searching strategy for the
        /// platform. Tries to search a unique file or directory name in the given
        /// root directory, with a given an optional extension.
        fn unique_path_in(_root: &Path, _extension: Option<&str>) -> Result<PathBuf> {
            Err(Error::new(ErrorKind::Other,
                "Unique paths are not supported on this platform!"))
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

    /// The Win32 implementation of `trait FsTemp`.
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

        fn temp_file(path: &Path) -> Result<fs::File> {
            let path = to_wstring(path.as_os_str());
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

        fn temp_dir(path: &Path) -> Result<Self::Directory> {
            // First create the path
            let wpath = to_wstring(path.as_os_str());
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

        fn unique_path_in(root: &Path, extension: Option<&str>) -> Result<PathBuf> {
            // For now we default to the generic one, appending thread-id
            let extra = unsafe{ GetCurrentThreadId() };
            unique_path_with_timestamp(root, extension, extra)
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

// Unix implementation /////////////////////////////////////////////////////////

#[cfg(target_family = "unix")]
mod unix {
    use std::os::unix::ffi::OsStrExt;
    use super::*;

    #[link(name = "c")]
    extern "C" {
        fn getpid() -> i32;
        fn unlink(pathname: *const u8);
    }

    /// Converts the Rust &OsStr into a C conar*.
    fn to_cstring(s: &OsStr) -> Vec<u8> {
        s.as_bytes().iter().chain(Some(0).into_iter()).collect()
    }

    /// `trait FsTemp` on Unix systems.
    pub struct UnixTemp;

    impl FsTemp for UnixTemp {
        type Directory = UnixDirectory;

        fn temp_path() -> Result<PathBuf> {
            Ok(PathBuf::from("/tmp"))
        }

        fn temp_file(path: &Path) -> Result<fs::File> {
            let f = fs::OpenOptions::new()
                .create_new(true)
                .read(true).write(true)
                .open(path)?;
            let cpath = to_cstring(path);
            unsafe { unlink(cpath.as_ptr()) };
        }

        fn temp_dir(path: &Path) -> Result<Self::Directory> {
            panic!("TODO UNIX")
        }

        fn unique_path_in(root: &Path, extension: Option<&str>) -> Result<PathBuf> {
            // For now we default to the generic one, appending thread-id
            let extra = unsafe{ getpid() };
            unique_path_with_timestamp(root, extension, extra)
        }
    }

    /// Unix directory handle type.
    #[derive(Debug)]
    pub struct UnixDirectory;

    impl UnixDirectory {
        pub fn path(&self) -> &Path { panic!("TODO UNIX") }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type FsTempImpl = win32::WinApiTemp;
#[cfg(target_family = "unix")] type FsTempImpl = unix::UnixTemp;
#[cfg(not(any(
    target_os = "windows",
    target_family = "unix",
)))] type FsTempImpl = unsupported::UnsupportedTemp;

#[cfg(test)]
mod tests {
    use super::*;
    use fs_path::FilePath;
    use std::ffi::OsString;

    #[test]
    fn test_path() -> Result<()> {
        let path = path(None)?;
        assert!(!path.exists());
        assert!(path.extension() == None);
        let parent = path.parent();
        assert!(parent.is_some());
        assert!(parent.unwrap().exists());
        Ok(())
    }

    #[test]
    fn test_path_with_extension() -> Result<()> {
        let path = path(Some("txt"))?;
        assert!(!path.exists());
        assert!(path.extension() == Some(&OsString::from("txt")));
        let parent = path.parent();
        assert!(parent.is_some());
        assert!(parent.unwrap().exists());
        Ok(())
    }

    #[test]
    fn test_path_in() -> Result<()> {
        let path = path_in(".", None)?;
        assert!(!path.exists());
        assert!(path.extension() == None);
        assert_eq!(
            fs::canonicalize(path.parent().unwrap())?,
            fs::canonicalize(".")?
        );
        Ok(())
    }

    #[test]
    fn test_file() -> Result<()> {
        let path;
        {
            let file = file(Some("txt"))?;
            path = file.path()?;
            assert!(path.exists()); // NOTE: Not true for unix probably
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
            assert!(path.exists()); // NOTE: Not true for unix probably
            assert!(path.extension() == Some(&OsString::from("txt")));
        }
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn test_file_at() -> Result<()> {
        let path;
        {
            let dir = file_at("./hello.txt")?;
            path = dir.path()?;
            assert_eq!(
                fs::canonicalize(path.parent().unwrap())?,
                fs::canonicalize(".")?
            );
            assert!(path.exists()); // NOTE: Not true for unix probably
            assert!(path.extension() == Some(&OsString::from("txt")));
            assert!(path.file_name() == Some(&OsString::from("hello.txt")));
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
            assert!(path.exists()); // NOTE: Not true for unix probably
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
            assert!(path.exists()); // NOTE: Not true for unix probably
        }
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn test_directory_at() -> Result<()> {
        let path;
        {
            let dir = directory_at("./foo")?;
            path = dir.path().to_path_buf();
            assert_eq!(
                fs::canonicalize(path.parent().unwrap())?,
                fs::canonicalize(".")?
            );
            assert!(path.exists()); // NOTE: Not true for unix probably
            assert!(path.ends_with("foo"));
        }
        assert!(!path.exists());
        Ok(())
    }
}
