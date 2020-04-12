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
    path   : PathBuf,
    /// The loaded client library.
    library: Library,
    /// The last modification time read from the client library's path.
    mtime  : SystemTime,
    /// The loaded symbol from the client library.
    client : ClientApi,
    /// The state of the client.
    state  : Vec<u8>,
    /// The API type the host provides from it's side.
    api    : API,
}

impl <API> Host<API> {
    pub fn new(mut api: API, path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let mut library = Library::load(path)?;
        let mtime = mtime(path)?;
        let client = load_symbol(&mut library)?;
        let mut state = vec![0u8; client.state_size as usize];
        (client.new_state)(state.as_mut_ptr(), (&mut api as *mut API).cast());
        Ok(Self{
            path: path.to_path_buf(),
            library,
            mtime,
            client,
            state,
            api,
        })
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

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

fn mtime(path: &Path) -> io::Result<SystemTime> {
    fs::metadata(path)
        .and_then(|m| m.modified().or_else(|_| m.created()))
}

fn load_symbol(library: &mut Library) -> io::Result<ClientApi> {
    let sym: Symbol<*const ClientApi> = library.load_symbol("CLIENT_API")?;
    Ok(unsafe{ ptr::read(*sym) })
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ClientApi {
    state_size   : u64,
    new_state    : fn(*mut u8, *mut u8),
    migrate_state: fn(*mut u8, usize, *mut u8, *mut u8),
    drop_state   : fn(*mut u8),

    initialize   : fn(*mut u8, *mut u8),
    reload       : fn(*mut u8, *mut u8),
    unload       : fn(*mut u8, *mut u8),
    terminate    : fn(*mut u8, *mut u8),
    update       : fn(*mut u8, *mut u8) -> u32,
}
