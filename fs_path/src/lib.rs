//! Provides a path getter for file handles, when possible.
//!
//! Rust doesn't provide this by default, because the file could have been
//! removed. There are still use-cases when it makes sense to ask for the path,
//! when it's still valid.
//!
//! The whole public API consists of the [FilePath](trait.FilePath.html)
//! `trait`.
//!
//! # Usage
//!
//! Just make sure to have [FilePath](trait.FilePath.html) in scope, and keep in
//! mind that the operation can fail:
//!
//! ```no_run
//! use std::fs::File;
//! use std::path::PathBuf;
//! use fs_path::FilePath;
//!
//! # fn main() -> std::io::Result<()> {
//! let file = File::create("C:/TMP/foo.txt")?;
//! let path = file.path()?;
//! assert_eq!(path, PathBuf::from("C:/TMP/foo.txt"));
//! # Ok(())
//! # }
//! ```
//!
//! # Porting the library to other platforms
//!
//! A single function named `path_for` has to be in global scope for the
//! platform:
//!
//! ```no_run
//! use std::io::Result;
//! use std::path::PathBuf;
//! use std::fs::File;
//!
//! fn path_for(handle: &File) -> Result<PathBuf> {
//!     // ...
//! # Ok(PathBuf::new())
//! }
//! ```
//!
//! Which should return the path for the handle, if possible.

use std::fs::File;
use std::path::PathBuf;
use std::io::Result;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// The `trait` that's being implemented for `File`s.
pub trait FilePath {
    /// Returns the path of this `File` handle, if it's valid.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::path::PathBuf;
    /// use fs_path::FilePath;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let file = File::create("C:/TMP/foo.txt")?;
    /// let path = file.path()?;
    /// assert_eq!(path, PathBuf::from("C:/TMP/foo.txt"));
    /// # Ok(())
    /// # }
    /// ```
    fn path(&self) -> Result<PathBuf>;
}

impl FilePath for File {
    fn path(&self) -> Result<PathBuf> {
        path_for(self)
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// Unsupported implementation //////////////////////////////////////////////////

mod unsupported {
    #![allow(dead_code)]

    use std::io::{Error, ErrorKind};
    use super::*;

    pub fn path_for(_file: &File) -> Result<PathBuf> {
        Err(Error::new(ErrorKind::Other,
            "Asking the path of a file handle is unsupported on this platform!"))
    }
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use std::ptr;
    use super::*;

    const FILE_NAME_NORMALIZED: u32 = 0;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetFinalPathNameByHandleW(
            handle     : *mut c_void,
            buffer     : *mut u16   ,
            buffer_size: u32        ,
            flags      : u32        ,
        ) -> u32;
    }

    pub fn path_for(file: &File) -> Result<PathBuf> {
        let handle = file.as_raw_handle();
        let required_size = unsafe { GetFinalPathNameByHandleW(
            handle,
            ptr::null_mut(),
            0,
            FILE_NAME_NORMALIZED) };
        if required_size == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut buffer = vec![0u16; required_size as usize];
        let written_size = unsafe { GetFinalPathNameByHandleW(
            handle,
            buffer.as_mut_ptr(),
            required_size,
            FILE_NAME_NORMALIZED) };
        if written_size == 0 || written_size > required_size {
            return Err(std::io::Error::last_os_error());
        }
        // Remove 0-terminator
        let buffer = &buffer[..(written_size as usize)];
        Ok(OsString::from_wide(buffer).into())
    }
}

// Linux implementation ////////////////////////////////////////////////////////

#[cfg(target_os = "linux")]
mod linux {
    use std::os::unix::io::AsRawFd;
    use std::fs;
    use super::*;

    pub fn path_for(file: &File) -> Result<PathBuf> {
        let id = file.as_raw_fd();
        let symlink = format!("/proc/self/fd/{}", id);
        fs::read_link(&symlink)
    }
}

// OSX implementation //////////////////////////////////////////////////////////

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    #[link(name = "c")]
    extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }

    pub fn path_for(file: &File) -> Result<PathBuf> {
        unimplemented!()
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] use win32::path_for;
#[cfg(target_os = "linux")] use linux::path_for;
#[cfg(target_os = "macos")] use macos::path_for;
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
)))] use unsupported::path_for;

#[cfg(test)]
mod tests {
    use super::*;

    /// Just a helper type to delete the file even when the test fails.
    struct DelFile(PathBuf);

    impl Drop for DelFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn test_path() -> Result<()> {
        // TODO: This is kinda bad, locally creates a file
        let name = PathBuf::from("fs_path_testing.txt");
        let file = File::create(&name)?;
        let _del = DelFile(name.clone());
        assert_eq!(file.path()?.file_name(), Some(name.as_os_str()));
        Ok(())
    }
}
