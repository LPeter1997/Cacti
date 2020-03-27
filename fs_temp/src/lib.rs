//! A minimalistic, dependency-free, cross-platform library for creating
//! temporary files and folders.

use std::io;

/// The `Result` type of this library.
pub type Result<T> = io::Result<T>;

/*
API:
/// Returns a temporary file name without path.
fn file_name() -> PathBuf;
/// Returns a temporary absolute path for a directory.
fn directory_path() -> PathBuf;
/// Returns a temporary absolute file-path.
fn file_path() -> PathBuf;
*/

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{c_void, OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;
    use std::mem;
    use std::ptr;
    use std::slice;
    use super::*;

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

    pub fn directory_path() -> Result<PathBuf> {
        const BUFFER_SIZE: usize = 8192;

        let mut buffer: mem::MaybeUninit<[u16; BUFFER_SIZE]> = mem::MaybeUninit::uninit();
        let len = unsafe { GetTempPathW(
            BUFFER_SIZE as u32,
            buffer.as_mut_ptr().cast()) };
        if len == 0 {
            Err(last_error())
        }
        else {
            let slice = unsafe { slice::from_raw_parts(buffer.as_mut_ptr().cast::<u16>(), len as usize) };
            Ok(PathBuf::from(OsString::from_wide(slice)))
        }
    }
}

pub use win32::directory_path;
