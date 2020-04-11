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
use std::marker::PhantomData;
use std::ops::Deref;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

// TODO: doc

pub struct Library(DynLibImpl);

impl Library {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self(DynLibImpl::load(path.as_ref())?))
    }

    pub fn load_symbol<T>(&mut self, name: &str) -> Result<Symbol<T>> {
        Ok(Symbol{
            sym: self.0.load_symbol(name)?,
            phantom: PhantomData,
        })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        self.0.unload();
    }
}

pub struct Symbol<'a, T: 'a> {
    sym: <DynLibImpl as DynLib>::Symbol,
    phantom: PhantomData<&'a T>,
}

impl <'a, T: 'a> Deref for Symbol<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe{ std::mem::transmute(self.sym.ptr_ref()) }
    }
}

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

// Unsupported implementation //////////////////////////////////////////////////

mod unsupported {
    #![allow(dead_code)]

    use std::io::{Error, ErrorKind};
    use super::*;

    #[derive(Debug)]
    pub struct UnsupportedDynLib;

    #[derive(Debug)]
    pub struct UnsupportedSymbol;

    impl DynLib for UnsupportedDynLib {
        type Symbol = UnsupportedSymbol;

        fn load(_path: &Path) -> Result<Self> {
            Err(Error::new(ErrorKind::Other, "Library loading is not supported on this platform!"))
        }

        fn unload(&mut self) { unreachable!() }
        fn load_symbol(&mut self, _name: &str) -> Result<Self::Symbol> { unreachable!() }
    }
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
        ) -> *mut c_void;
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
        pub fn ptr_ref(&self) -> &*const c_void { &self.0 }
    }
}

// Unix implementation /////////////////////////////////////////////////////////

#[cfg(target_family = "unix")]
mod unix {
    use std::ffi::{CString, c_void};
    use std::os::raw::c_char;
    use std::os::unix::ffi::OsStrExt;
    use std::io;
    use super::*;

    const RTLD_NOW: i32 = 0x2;

    #[link(name = "c")]
    extern "C" {
        fn dlopen(fname: *const c_char, flag: i32) -> *mut c_void;
        fn dlerror() -> *mut c_char;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> i32;
    }

    #[derive(Debug)]
    pub struct UnixDynLib(*mut c_void);

    #[derive(Debug)]
    pub struct UnixSymbol(*const c_void);

    fn get_dlerror() -> io::Error {
        let err = dlerror();
        let err_str = unsafe{ CString::from_raw(err) };
        io::Error::new(io::ErrorKind::Other, err_str)
    }

    impl DynLib for UnixDynLib {
        type Symbol = UnixSymbol;

        fn load(path: &Path) -> Result<Self> {
            let name = CString::new(path.as_os_str().as_bytes());
            let handle = unsafe{ dlopen(name.as_ptr(), RTLD_NOW) };
            if handle.is_null() {
                return Err(get_dlerror());
            }
            Ok(Self(handle))
        }

        fn unload(&mut self) {
            if self.0.is_null() {
                return;
            }
            unsafe{ dlclose(self.0) };
            self.0 = ptr::null_mut();
        }

        fn load_symbol(&mut self, name: &str) -> Result<Self::Symbol> {
            let name = unsafe{ CString::from_vec_unchecked(name) };
            let sym = unsafe{ dlsym(self.0, name.as_ptr()) };
            if sym.is_null() {
                return Err(get_dlerror());
            }
            Ok(UnixSymbol(sym))
        }
    }

    impl UnixSymbol {
        pub fn ptr_ref(&self) -> &*const c_void { &self.0 }
    }
}

// Choosing the right implementation based on platform.

#[cfg(target_os = "windows")] type DynLibImpl = win32::WinApiDynLib;
#[cfg(target_family = "unix")] type DynLibImpl = unix::UnixDynLib;
#[cfg(not(any(
    target_os = "windows",
    target_family = "unix",
)))] type DynLibImpl = unsupported::UnsupportedDynLib;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win32_nonexisting() {
        let l = Library::load("nonexisting");
        assert!(l.is_err());
    }

    #[test]
    fn test_win32_kernel32() -> Result<()> {
        let mut l = Library::load("kernel32")?;
        let sym: Symbol<extern "system" fn(u32) -> u32> = l.load_symbol("GetProcessVersion")?;
        let v = sym(0);
        assert_ne!(0, v);
        Ok(())
    }
}
