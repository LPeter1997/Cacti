//! Loading and unloading of libraries and symbols at runtime.
//!
//! # Usage
//!
//! TODO
//!
//! # Porting the library to other platforms
//!
//! TODO

use std::io::Result;
use std::path::Path;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

// TODO

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

/// The library-level functionality every platform must implement.
trait DynLib: Sized {
    /// The type of symbol this platform provides.
    type Symbol: std::fmt::Debug;

    /// Loads the library at the given path.
    fn load(path: &Path) -> Result<Self>;

    /// Unloads this library.
    fn unload(&mut self);

    /// Loads the symbol with the given name.
    fn load_symbol(&mut self, name: &str) -> Result<Self::Symbol>;
}

// WinAPI implementation ///////////////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{OsStr, CString, c_void};
    use std::os::raw::c_char;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::io;
    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;

        fn FreeLibrary(hmodule: *mut c_void) -> i32;

        fn GetProcAddress(
            hmodule: *mut c_void  ,
            name   : *const c_char,
        ) -> *const c_void;
    }

    /// Converts the Rust &OsStr into a WinAPI `WCHAR` string.
    fn to_wstring(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0).into_iter()).collect()
    }

    #[derive(Debug)]
    pub struct WinApiDynLib(*mut c_void);

    #[derive(Debug)]
    pub struct WinApiSymbol(*const c_void);

    impl DynLib for WinApiDynLib {
        type Symbol = WinApiSymbol;

        fn load(path: &Path) -> Result<Self> {
            let wpath = to_wstring(path.as_os_str());
            let hmodule = unsafe{ LoadLibraryW(wpath.as_ptr()) };
            if hmodule.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(hmodule))
        }

        fn unload(&mut self) {
            if self.0.is_null() {
                return;
            }
            unsafe{ FreeLibrary(self.0) };
            self.0 = ptr::null_mut();
        }

        fn load_symbol(&mut self, name: &str) -> Result<Self::Symbol> {
            let name = unsafe{ CString::from_vec_unchecked(name.into()) };
            let sym = unsafe{ GetProcAddress(self.0, name.as_ptr()) };
            if sym.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(WinApiSymbol(sym))
        }
    }

    impl WinApiSymbol {
        pub fn as_ptr(&self) -> *const c_void { self.0 }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type DynLibImpl = win32::WinApiDynLib;

#[cfg(test)]
mod tests {
    use super::*;

    // TODO
}
