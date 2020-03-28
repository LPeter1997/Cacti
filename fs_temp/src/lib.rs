//! A minimalistic, dependency-free, cross-platform library for generating
//! temporary files and directories.

use std::io::Result;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::fs;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

pub fn path(extension: Option<&str>) -> Result<PathBuf> {
    path_in(&FsTempImpl::tmp_path()?, extension)
}

pub fn path_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<PathBuf> {
    FsTempImpl::unique_in(root.as_ref(), extension)
}

pub fn file(extension: Option<&str>) -> Result<File> {
    FsTempImpl::file_handle(&path(extension)?)
}

pub fn file_in(root: impl AsRef<Path>, extension: Option<&str>) -> Result<File> {
    FsTempImpl::file_handle(&path_in(root, extension)?)
}

pub fn directory() -> Result<Directory> {
    directory_in(&FsTempImpl::tmp_path()?)
}

pub fn directory_in(root: impl AsRef<Path>) -> Result<Directory> {
    let path = path_in(root, None)?;
    fs::create_dir(&path)?;
    Ok(Directory{ path })
}

#[derive(Debug)]
pub struct Directory {
    path: PathBuf,
}

impl Directory {
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
    unimplemented!()
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
                .access_mode(GENERIC_READ | GENERIC_WRITE)
                .share_mode(FILE_SHARE_DELETE)
                .custom_flags(FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_DELETE_ON_CLOSE)
                .open(path)
        }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type FsTempImpl = win32::WinApiTemp;

#[cfg(test)]
mod tests {
    use super::*;
}
