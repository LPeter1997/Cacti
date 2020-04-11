//! Run-time hot-code-reloading.
// TODO: doc n stuff

use std::path::{Path, PathBuf};
use std::io;
use crate::dyn_lib::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loop {
    Continue,
    Stop,
}

/// The trait that every live-reloadable code must implement.
pub trait Client {
    /// The type of state the client requires to operate on.
    type State;

    /// Called once, before the first loaded event.
    fn initialize(state: &mut Self::State);

    /// Called when the client's code is loaded into memory.
    fn reload(state: &mut Self::State);

    /// Called when the client's code is about to be unloaded from memory.
    fn unload(state: &mut Self::State);

    /// Called when the client is about to be destroyed.
    fn terminate(state: &mut Self::State);

    /// Called in regular intervals to update the client.
    fn update(state: &mut Self::State) -> Loop;
}

pub struct Host<C: Client> {
    lib_path: PathBuf,
    // TODO: We box for now because of lifetime issues
    lib: Box<Library>,
    state: C::State,
}

impl <C: Client> Host<C> {
    pub fn new(lib_path: impl AsRef<Path>) -> io::Result<Self> where C::State: Default {
        Self::with_state(lib_path, C::State::default())
    }

    pub fn with_state(lib_path: impl AsRef<Path>, state: C::State) -> io::Result<Self> {
        let lib_path = lib_path.as_ref();
        let mut lib = Box::new(Library::load(lib_path)?);
        // TODO: Symbol name?
        // TODO: Symbol type?
        let _sym: Symbol<*mut ()> = lib.load_symbol("")?;
        // TODO: Load symbol stuff?
        Ok(Self{
            lib_path: lib_path.to_path_buf(),
            lib,
            state,
        })
    }

    pub fn run(&mut self) {
        // TODO: Init
        loop {
            // TODO: Check if needed to reload
                // TODO: If so, unload, reload
            // TODO: Call update
            // TODO: If terminated, break loop
        }
        // TODO: Terminate
    }
}
