
use std::fs::File;
use std::path::PathBuf;
use std::io::Result;

pub trait FilePath {
    fn path(&self) -> Result<PathBuf>;
}

impl FilePath for File {
    fn path(&self) -> Result<PathBuf> {
        FsPathImpl::path_for(self)
    }
}

trait FsPath {
    fn path_for(file: &File) -> Result<PathBuf>;
}

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::ptr;
    use std::slice;
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

    pub struct WinApiFsPath;

    impl FsPath for WinApiFsPath {
        fn path_for(file: &File) -> Result<PathBuf> {
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
            let buffer: &[u16] = unsafe { slice::from_raw_parts(
                buffer.as_ptr().cast(),
                written_size as usize) };
            Ok(OsString::from_wide(buffer).into())
        }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type FsPathImpl = win32::WinApiFsPath;
