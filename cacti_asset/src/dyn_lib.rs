//! Loading and unloading of dynamic/shared libraries and symbols at runtime.
//!
//! # Usage
//!
//! First you need to load a dynamic [Library](struct.Library.html):
//!
//! ```no_run
//! # fn main() -> std::io::Result<()> {
//! use cacti_asset::dyn_lib::*;
//!
//! let mut lib = Library::load("foo.dll")?;
//! # Ok(())
//! # }
//! ```
//!
//! After that you can load [Symbol](struct.Symbol.html)s from it:
//!
//! ```no_run
//! # fn main() -> std::io::Result<()> {
//! # use cacti_asset::dyn_lib::*;
//! # let mut lib = Library::load("foo.dll")?;
//! let sym: Symbol<extern "system" fn(u32) -> u32> = lib.load_symbol("times_two")?;
//! // Calling it
//! let raw_fn = *sym;
//! let four = raw_fn(2);
//! # Ok(())
//! # }
//! ```
//!
//! The `Symbol` type's lifetime is tied to the `Library`'s, but de-referencing
//! the symbol gets rid of the wrapper.
//!
//! # Porting the library to other platforms
//!
//! To port this library to other platforms, the `trait DynLib` has to be
//! implemented for a type and have it aliased as `DynLibImpl` in global scope
//! for the platform:
//!
//! ```no_run
//! # use std::io::Result;
//! # use std::path::Path;
//! # trait DynLib: Sized {
//! #     type Symbol: std::fmt::Debug;
//! #     fn load(path: &Path) -> Result<Self>;
//! #     fn unload(&mut self);
//! #     fn load_symbol(&mut self, name: &str) -> Result<Self::Symbol>;
//! # }
//! #[cfg(target_os = "new_platform")]
//! mod my_platform {
//!     /// Library type must be debug.
//!     #[derive(Debug)]
//!     struct MyPlatformDynLib { /* ... */ }
//!
//!     /// Symbol type must be debug and copy.
//!     #[derive(Debug, Clone, Copy)]
//!     struct MyPlatformSymbol { /* ... */ }
//!
//!     impl DynLib for MyPlatformDynLib {
//!         /// The symbol type we defined for this platform.
//!         type Symbol = MyPlatformSymbol;
//!
//!         /// Here you must load the library from the given path. Sometimes
//!         /// there can be platform-specific behavior - like loading system
//!         /// libraries without full path on Windows - that should be
//!         /// documented.
//!         fn load(path: &Path) -> Result<Self> {
//!             // ...
//! # unimplemented!()
//!         }
//!
//!         /// Here you should unload the library, freeing up the resource.
//!         fn unload(&mut self) {
//!             // ...
//!         }
//!
//!         /// Here you should load the symbol with the given name.
//!         fn load_symbol(&mut self, name: &str) -> Result<Self::Symbol> {
//!             // ...
//! # unimplemented!()
//!         }
//!     }
//! }
//!
//! #[cfg(target_os = "new_platform")] type DynLibImpl = my_platform::MyPlatformDynLib;
//! ```

// TODO: Doc platform-specific usage

use std::io::Result;
use std::path::Path;
use std::marker::PhantomData;
use std::ops::Deref;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// Represents a loaded dynamic/shared library that gets unloaded when dropped.
#[derive(Debug)]
pub struct Library(DynLibImpl);

impl Library {
    /// Loads the dynamic/shared library from the given path.
    ///
    /// # Examples
    ///
    /// Let's load `Kernel32` on Windows:
    ///
    /// ```no_run
    /// # fn main() -> std::io::Result<()> {
    /// use cacti_asset::dyn_lib::Library;
    ///
    /// let lib = Library::load("kernel32")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Loading a `dll` from the current working directory is not different:
    ///
    /// ```no_run
    /// # fn main() -> std::io::Result<()> {
    /// use cacti_asset::dyn_lib::Library;
    ///
    /// let lib = Library::load("some_lib.dll")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// In case of an IO or system error, an error variant is returned.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self(DynLibImpl::load(path.as_ref())?))
    }

    /// Loads a symbol from this library with the given symbolic name.
    ///
    /// # Examples
    ///
    /// Loading and running `GetProcessVersion` from `Kernel32` on Windows:
    ///
    /// ```no_run
    /// # fn main() -> std::io::Result<()> {
    /// use cacti_asset::dyn_lib::*;
    ///
    /// let mut lib = Library::load("kernel32")?;
    /// let sym: Symbol<extern "system" fn(u32) -> u32> = lib.load_symbol("GetProcessVersion")?;
    /// // Deref to get the function pointer itself
    /// let get_ver = *sym;
    /// // Call it
    /// let ver = get_ver(0);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// In case of an IO or system error, an error variant is returned.
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

/// Represents a loaded symbol. This symbol's lifetime is tied to the library
/// it's loaded from to pervent usage after unloading the library.
///
/// The symbol can be de-reference to get the actual symbol value.
///
/// # Examples
///
/// Loading the `i32` symbol called `MAGIC_NUM` from the library `foo.so`:
///
/// ```no_run
/// # fn main() -> std::io::Result<()> {
/// use cacti_asset::dyn_lib::*;
///
/// let mut lib = Library::load("foo.so")?;
/// let sym: Symbol<i32> = lib.load_symbol("MAGIC_NUM")?;
/// // Getting the actual value
/// let magic_num = *sym;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
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
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::io;
    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn FreeLibrary(hmodule: *mut c_void) -> i32;
        fn GetProcAddress(hmodule: *mut c_void, name: *const i8) -> *mut c_void;
    }

    /// Converts the Rust &OsStr into a WinAPI `WCHAR` string.
    fn to_wstring(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0).into_iter()).collect()
    }

    #[derive(Debug)]
    pub struct WinApiDynLib(*mut c_void);

    #[derive(Debug, Clone, Copy)]
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
    use std::os::raw::{c_char, c_int};
    use std::os::unix::ffi::OsStrExt;
    use std::io;
    use std::ptr;
    use super::*;

    const RTLD_NOW: i32 = 0x2;

    #[link(name = "c")]
    extern "C" {
        fn dlopen(fname: *const c_char, flag: c_int) -> *mut c_void;
        fn dlerror() -> *mut c_char;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }

    #[derive(Debug)]
    pub struct UnixDynLib(*mut c_void);

    #[derive(Debug, Clone, Copy)]
    pub struct UnixSymbol(*const c_void);

    fn get_dlerror() -> io::Error {
        let err = unsafe{ dlerror() };
        let err_str = unsafe{ CString::from_raw(err) }.into_string()
            .unwrap_or_else(|_| "Unknown dlerror".into());
        io::Error::new(io::ErrorKind::Other, err_str)
    }

    impl DynLib for UnixDynLib {
        type Symbol = UnixSymbol;

        fn load(path: &Path) -> Result<Self> {
            let name = unsafe{ CString::from_vec_unchecked(path.as_os_str().as_bytes().to_vec()) };
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
            let name = unsafe{ CString::from_vec_unchecked(name.as_bytes().to_vec()) };
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

    // TODO: Cross-platform tests
    // We would need to figure out a way to ad-hoc compile some dynamic library
    // so we could actually test loading it...

    #[test]
    #[cfg(target_os = "windows")]
    fn test_win32_nonexisting() {
        let l = Library::load("nonexisting");
        assert!(l.is_err());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_win32_kernel32() -> Result<()> {
        let mut l = Library::load("kernel32")?;
        let sym: Symbol<extern "system" fn(u32) -> u32> = l.load_symbol("GetProcessVersion")?;
        let v = sym(0);
        assert_ne!(0, v);
        Ok(())
    }
}
