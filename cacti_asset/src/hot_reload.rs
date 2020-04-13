//! Run-time hot-code-reloading.
// TODO: doc n stuff

use std::path::{Path, PathBuf};
use std::io;
use std::fs;
use std::ptr;
use std::time::SystemTime;
use crate::dyn_lib::*;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// The type responsible for managing the dinamically loaded part of the
/// application. This type should be held in the part that won't be
/// live-reloaded.
pub struct Host<API> {
    /// Path to the client library.
    path    : PathBuf,
    /// The loaded client library.
    library: Library,
    /// The last modification time read from the client library's path.
    mtime  : SystemTime,
    /// The loaded symbol from the client library.
    client : ClientApi,
    /// The state of the client.
    state  : Option<Vec<u8>>,
    /// The API type the host provides from it's side.
    api    : API,
    /// The number of library reloads.
    counter: usize,
    /// The path we copy out the library file to to allow recompilation.
    tmppath: PathBuf,
}

impl <API> Drop for Host<API> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.tmppath);
    }
}

impl <API> Host<API> {
    /// Creates a new host from the provided API and path for the client
    /// library.
    pub fn new(api: API, path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let mtime = mtime(path)?;

        // Copy the library
        let tmppath = lib_path(path, 0);
        fs::copy(path, &tmppath)?;
        let mut library = Library::load(&tmppath)?;
        let client = load_symbol(&mut library)?;

        Ok(Self{
            path: path.to_path_buf(),
            library,
            mtime,
            client,
            state: None,
            api,
            counter: 0,
            tmppath,
        })
    }

    /// Updates the client and returns the status returned by the client's
    /// update method.
    pub fn update(&mut self) -> io::Result<Loop> {
        let api = (&mut self.api as *mut API).cast();
        if self.state.is_none() {
            // We need to create the state
            let mut state = vec![0u8; self.client.state_size];
            (self.client.new_state)(state.as_mut_ptr(), api);
            self.state = Some(state);
            let cur_state = self.state.as_mut().unwrap();
            // Call initialize
            (self.client.initialize)(cur_state.as_mut_ptr(), api);
            // Call reload
            (self.client.reload)(cur_state.as_mut_ptr(), api);
        }
        // Here we must have a valid state
        let cur_state = self.state.as_mut().unwrap();
        // Check if we need to reload the library
        let mtime = mtime(&self.path)?;
        if mtime != self.mtime {
            // We need to reload the library
            // Call unload
            (self.client.unload)(cur_state.as_mut_ptr(), api);
            // Copy a new version of the library
            self.counter += 1;
            let tmppath = lib_path(&self.path, self.counter);
            fs::copy(&self.path, &tmppath)?;
            // Reload library and symbol
            let mut library = Library::load(&tmppath)?;
            let client = load_symbol(&mut library)?;
            self.library = library;
            self.client = client;
            self.mtime = mtime;
            // Delete old
            let _ = fs::remove_file(&self.tmppath);
            self.tmppath = tmppath;
            // Migrate the state
            let mut new_state = vec![0u8; self.client.state_size];
            (self.client.migrate_state)(
                cur_state.as_mut_ptr(),
                cur_state.len(),
                new_state.as_mut_ptr(),
                api);
            self.state = Some(new_state);
            let cur_state = self.state.as_mut().unwrap();
            // Call reload
            (self.client.reload)(cur_state.as_mut_ptr(), api);
        }
        let cur_state = self.state.as_mut().unwrap();
        // Now perform the update
        let loop_res = (self.client.update)(cur_state.as_mut_ptr(), api);
        let loop_res = Loop::from_u32(loop_res);
        if loop_res == Loop::Stop {
            // Call unload
            (self.client.unload)(cur_state.as_mut_ptr(), api);
            // Call terminate
            (self.client.terminate)(cur_state.as_mut_ptr(), api);
            // Destroy the state
            (self.client.drop_state)(cur_state.as_mut_ptr());
            self.state = None;
        }
        Ok(loop_res)
    }
}

/// This is the trait that the live-reloaded state type should implement. These
/// are the functions called by the `Host` to update the application.
pub trait Client {
    /// The type of host API this client expects. This is received from the host
    /// as a way to use services provided by it.
    type HostApi;

    /// Creates a new, initial client state.
    fn new(api: &mut Self::HostApi) -> Self;
    /// Migrates the old state - represented as a slice of bytes - to the new,
    /// current version.
    fn migrate(old: &mut [u8], api: &mut Self::HostApi) -> Self;

    /// Called before the first `reload`, to initialize things in the state.
    fn initialize(&mut self, api: &mut Self::HostApi);
    /// Called every time after the new code is loaded.
    fn reload(&mut self, api: &mut Self::HostApi);
    /// Called every time before the old code is unloaded.
    fn unload(&mut self, api: &mut Self::HostApi);
    /// Called after the last `unload` to clean up.
    fn terminate(&mut self, api: &mut Self::HostApi);
    /// Called to perform update on the application. The returned `Loop` variant
    /// will determine if the client application should continue running or
    /// terminate.
    fn update(&mut self, api: &mut Self::HostApi) -> Loop;
}

/// A descriptive type to control the main application loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loop {
    /// The client application should continue running.
    Continue,
    /// The client application should be terminated.
    Stop,
}

#[macro_export]
macro_rules! hot_reload {
    ( $state:ident ) => {
        #[no_mangle]
        pub static HOT_RELOAD_CLIENT_API: $crate::hot_reload::ClientApi =
        $crate::hot_reload::ClientApi{
            state_size   : ::std::mem::size_of::<$state>(),
            new_state    : hot_reload_api::new_state      ,
            migrate_state: hot_reload_api::migrate_state  ,
            drop_state   : hot_reload_api::drop_state     ,

            initialize   : hot_reload_api::initialize     ,
            reload       : hot_reload_api::reload         ,
            unload       : hot_reload_api::unload         ,
            terminate    : hot_reload_api::terminate      ,
            update       : hot_reload_api::update         ,
        };

        mod hot_reload_api {
            type State = super::$state;

            $crate::hot_reload_funcs!();
        }
    };
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

#[macro_export(local_inner_macros)]
macro_rules! hot_reload_funcs {
    () => {
        use ::std::mem;
        use ::std::ptr;
        use ::std::slice;
        use $crate::hot_reload::*;

        // Helpers

        fn cast_state(buffer: *mut u8) -> &'static mut State {
            unsafe{ &mut *buffer.cast() }
        }

        fn cast_api(buffer: *mut u8) -> &'static mut <State as Client>::HostApi {
            unsafe{ &mut *buffer.cast() }
        }

        fn state_to_buffer(state: State, buffer: *mut u8) {
            let state_ptr = &state as *const State as *const u8;
            unsafe{ ptr::copy(state_ptr, buffer, mem::size_of::<State>()) };
            mem::forget(state);
        }

        // Api implementation

        pub fn new_state(buffer: *mut u8, api: *mut u8) {
            let state = State::new(cast_api(api));
            state_to_buffer(state, buffer);
        }

        pub fn migrate_state(old: *mut u8, old_size: usize, new: *mut u8, api: *mut u8) {
            let old = unsafe{ slice::from_raw_parts_mut(old, old_size) };
            let new_state = State::migrate(old, cast_api(api));
            state_to_buffer(new_state, new);
        }

        pub fn drop_state(buffer: *mut u8) {
            let s: State = unsafe{ ptr::read(buffer.cast()) };
            mem::drop(s);
        }

        pub fn initialize(state: *mut u8, api: *mut u8) {
            State::initialize(cast_state(state), cast_api(api));
        }

        pub fn reload(state: *mut u8, api: *mut u8) {
            State::reload(cast_state(state), cast_api(api));
        }

        pub fn unload(state: *mut u8, api: *mut u8) {
            State::unload(cast_state(state), cast_api(api));
        }

        pub fn terminate(state: *mut u8, api: *mut u8) {
            State::terminate(cast_state(state), cast_api(api));
        }

        pub fn update(state: *mut u8, api: *mut u8) -> u32 {
            let res = State::update(cast_state(state), cast_api(api));
            Loop::to_u32(res)
        }
    };
}

impl Loop {
    fn from_u32(n: u32) -> Self {
        if n == 0 { Self::Continue } else { Self::Stop }
    }

    pub fn to_u32(self) -> u32 {
        if self == Self::Continue { 0 } else { 1 }
    }
}

fn mtime(path: &Path) -> io::Result<SystemTime> {
    fs::metadata(path)
        .and_then(|m| m.modified().or_else(|_| m.created()))
}

fn load_symbol(library: &mut Library) -> io::Result<ClientApi> {
    let sym: Symbol<*const ClientApi> = library.load_symbol("HOT_RELOAD_CLIENT_API")?;
    Ok(unsafe{ ptr::read(*sym) })
}

fn lib_path(path: &Path, cnt: usize, ) -> PathBuf {
    let mut new_path = path.to_path_buf();
    new_path.set_extension(format!("tmp_{}", cnt));
    new_path
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ClientApi {
    pub state_size   : usize,
    pub new_state    : fn(*mut u8, *mut u8),
    pub migrate_state: fn(*mut u8, usize, *mut u8, *mut u8),
    pub drop_state   : fn(*mut u8),

    pub initialize   : fn(*mut u8, *mut u8),
    pub reload       : fn(*mut u8, *mut u8),
    pub unload       : fn(*mut u8, *mut u8),
    pub terminate    : fn(*mut u8, *mut u8),
    pub update       : fn(*mut u8, *mut u8) -> u32,
}
