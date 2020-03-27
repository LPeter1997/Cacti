//! A minimalistic, dependency-free, cross-platform library for creating
//! temporary files and folders.

use std::io;

/// The `Result` type of this library.
pub type Result<T> = io::Result<T>;

/*
API:
fn directory_in(root: impl AsRef<Path>) -> Result<PathBuf>;
fn directory() -> Result<PathBuf>;
fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf>;
fn file(extension: Option<&str>) -> Result<PathBuf>;
*/

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{c_void, OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;
    use std::mem;
    use std::ptr;
    use std::slice;
    use super::*;

    const PATH_BUFFER_SIZE: usize = 8192;
    type PathBuffer = [u16; PATH_BUFFER_SIZE];

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLastError() -> u32;

        fn GetTempPathW(
            buffer_len: u32     ,
            buffer    : *mut u16,
        ) -> u32;

        fn GetTempFileNameW(
            path_name: *const u16,
            prefix   : *const u16,
            unique   : u32       ,
            res_name : *mut u16  ,
        ) -> u32;
    }

    /// Returns the last OS error represented as an `io::Error`.
    fn last_error() -> io::Error {
        let error = unsafe { GetLastError() };
        io::Error::from_raw_os_error(error as i32)
    }

    /// Converts the Rust &OsStr into a WinAPI WCHAR string.
    fn to_wstring(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0).into_iter()).collect()
    }

    /// Creates an uninitialized WSTRING buffer for paths.
    fn path_buffer() -> mem::MaybeUninit<PathBuffer> {
        mem::MaybeUninit::uninit()
    }

    /// Creates an `OsString` from a WSTRING buffer with the given length.
    fn buffer_to_os_string(buffer: &mem::MaybeUninit<PathBuffer>, len: u32) -> OsString {
        let slice = unsafe {
            slice::from_raw_parts(buffer.as_ptr().cast::<u16>(), len as usize)
        };
        OsString::from_wide(slice)
    }

    /// Returns the OS temporary path.
    fn tmp_path() -> Result<PathBuf> {
        let mut buffer = path_buffer();
        let len = unsafe { GetTempPathW(
            PATH_BUFFER_SIZE as u32,
            buffer.as_mut_ptr().cast()) };
        if len == 0 {
            Err(last_error())
        }
        else {
            Ok(PathBuf::from(buffer_to_os_string(&buffer, len)))
        }
    }

    /// Tries until it creates a unique path with the given formatter function.
    fn unique_path<F>(mut path: PathBuf, mut f: F) -> PathBuf
        where F: FnMut(u128, usize) -> String {
        const TRY_COUNT: usize = 16384;
        loop {
            // Generate a timestamp
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
            for i in 0..TRY_COUNT {
                // Combine the try-index and the timestamp
                let subfolder = f(timestamp, i);
                path.push(subfolder);
                if !path.exists() {
                    return path;
                }
                path.pop();
            }
        }
    }

    pub fn directory_in(root: impl AsRef<Path>) -> Result<PathBuf> {
        let root = root.as_ref().to_path_buf();
        Ok(unique_path(root, |timestamp, i| format!("tmp_{}_{}", timestamp, i)))
    }

    pub fn directory() -> Result<PathBuf> {
        Ok(unique_path(tmp_path()?, |timestamp, i| format!("tmp_{}_{}", timestamp, i)))
    }

    pub fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
        let root = root.as_ref().to_path_buf();
        let extension = extension.unwrap_or("TMP");
        Ok(unique_path(root, |timestamp, i| format!("tmp_{}_{}.{}", timestamp, i, extension)))
    }

    pub fn file(extension: Option<&str>) -> Result<PathBuf> {
        let extension = extension.unwrap_or("TMP");
        Ok(unique_path(tmp_path()?, |timestamp, i| format!("tmp_{}_{}.{}", timestamp, i, extension)))
    }
}

pub use win32::directory_in;
pub use win32::file_in;
pub use win32::directory;
pub use win32::file;