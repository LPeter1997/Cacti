//! A minimalistic, dependency-free, cross-platform filesystem watch library.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::fs;
use std::fmt;
use std::error;
use std::result;
use std::io;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// The `Result` type of this library.
pub type Result<T> = result::Result<T, Error>;

/// A filesystem watch that can listen to changes in files and folder
/// structures.
pub trait Watch: Sized {
    /// Creates a new `Watch` with no files watched.
    fn new() -> Result<Self>;

    /// Starts watching a given `Path` with the given recursion settings.
    fn watch(&mut self, p: impl AsRef<Path>, rec: Recursion) -> Result<()>;

    /// Stops watching a given `Path`.
    fn unwatch(&mut self, p: impl AsRef<Path>);

    /// Reads the next `Event`. Returns `None` if there's no `Event` to consume.
    fn poll_event(&mut self) -> Option<Event>;

    /// In case of a `Watch` that relies on some polling technique, this sets
    /// the interval that polling should be performed between. If a `Watch`
    /// relies on filesystem notifications, this `Duration` is unused.
    fn with_interval(self, _interval: Duration) -> Self {
        self
    }
}

/// Describes recursion strategies while watching a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recursion {
    /// Watch all the files and subfolders inside.
    Recursive,
    /// Only watch this path.
    NotRecursive,
}

/// The filesystem events the `Watch` can detect and produce.
#[derive(Debug, Clone)]
pub enum Event {
    /// A previously non-existing path now contains a file or folder.
    Created(PathBuf),
    /// A file or folder has been modified.
    Modified(PathBuf),
    /// A file or folder has been deleted.
    Deleted(PathBuf),
}

/// The possible errors this library can produce.
#[derive(Debug)]
pub enum Error {
    // An `std::io::Error`.
    IoError(io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::IoError(e)
    }
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// Polling /////////////////////////////////////////////////////////////////////

/// File states for the `PollWatch`.
#[derive(Debug)]
enum FileState {
    NotExisting{
        rec: Recursion,
    },
    Existing{
        is_file: bool,
        rec: Recursion,
        mod_time: Option<SystemTime>,
        substates: HashMap<PathBuf, FileState>,
    }
}

impl FileState {
    /// Reads the `FileState` of a `Path`.
    fn from_path(path: impl AsRef<Path>, rec: Recursion) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::NotExisting{ rec });
        }
        // Exists
        let info = fs::metadata(path)?;
        let mod_time = info.modified().ok();
        let is_file = info.is_file();
        let mut substates = HashMap::new();

        if rec == Recursion::Recursive && !is_file {
            // We care about subfolders
            Self::read_substates(path, &mut substates)?;
        }

        Ok(Self::Existing{ is_file, rec, mod_time, substates })
    }

    /// Reads the substates into the given buffer recursively.
    fn read_substates(path: impl AsRef<Path>, buffer: &mut HashMap<PathBuf, FileState>) -> Result<()> {
        for sub in fs::read_dir(path)? {
            // NOTE: Or just ignore if errors?
            let subpath = sub?.path();
            let substate = FileState::from_path(&subpath, Recursion::Recursive)?;
            buffer.insert(subpath, substate);
        }
        Ok(())
    }

    /// Updates the `FileState` corresponding at the given `Path`. Writes the
    /// occurred `Event`s into the given buffer.
    fn update(&mut self, path: impl AsRef<Path>, buffer: &mut VecDeque<Event>) {
        let path = path.as_ref();
        match self {
            Self::NotExisting{ rec } => {
                if path.exists() {
                    // The path exists now, update
                    // NOTE: Shouldn't something like this be logged?
                    if let Ok(next_state) = Self::from_path(path, *rec) {
                        *self = next_state;
                        // NOTE: We only add the event when we can update the
                        // state
                        buffer.push_back(Event::Created(path.to_path_buf()));
                    }
                }
            },
            Self::Existing{ is_file, rec, mod_time, substates } => {
                if !path.exists() {
                    // Not existing anymore
                    if !*is_file && *rec == Recursion::Recursive {
                        // We need to notify subentries too
                        // We just do an update
                        for (subpath, substate) in substates.iter_mut() {
                            substate.update(subpath, buffer);
                        }
                    }
                    *self = Self::NotExisting{ rec: *rec };
                    buffer.push_back(Event::Deleted(path.to_path_buf()));
                    return;
                }
                // Still existing, is it still the same?
                // NOTE: What if this is not Ok?
                if let Ok(info) = fs::metadata(path) {
                    if info.is_file() != *is_file {
                        // folder -> file or file -> folder
                        *is_file = info.is_file();
                        if *is_file {
                            // We clear out substates
                            substates.clear();
                        }
                        else if *rec == Recursion::Recursive {
                            // We need to read substates for the folder
                            // NOTE: We ignore potential error
                            let _ = Self::read_substates(path, substates);
                            // Also notify about creations
                            for subpath in substates.keys() {
                                buffer.push_back(Event::Created(subpath.to_path_buf()));
                            }
                        }
                        // Emulate by a delete + create
                        buffer.push_back(Event::Deleted(path.to_path_buf()));
                        buffer.push_back(Event::Created(path.to_path_buf()));
                        return;
                    }
                    // Still a file or folder, check modification date
                    // NOTE: What if this is not Ok?
                    if let Ok(mtime) = info.modified() {
                        let modified = mod_time.map(|m| m != mtime).unwrap_or(true);
                        if modified {
                            buffer.push_back(Event::Modified(path.to_path_buf()));
                            *mod_time = Some(mtime);
                        }
                    }
                    if !*is_file {
                        // Update substates
                        for (subpath, substate) in substates.iter_mut() {
                            substate.update(subpath, buffer);
                        }
                        // Check for new entries
                        // NOTE: What if this is not Ok?
                        if let Ok(walk) = fs::read_dir(path) {
                            for entry in walk {
                                // NOTE: What if this is not Ok?
                                if let Ok(entry) = entry {
                                    let path = entry.path();
                                    if !substates.contains_key(&path) {
                                        // Add it
                                        // NOTE: What if this is not Ok?
                                        if let Ok(substate) = FileState::from_path(&path, *rec) {
                                            buffer.push_back(Event::Created(path.to_path_buf()));
                                            substates.insert(path, substate);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A very trivial `Watch` implementation that polls file status.
#[derive(Debug)]
pub struct PollWatch {
    last_time: SystemTime,
    interval: Duration,
    events: VecDeque<Event>,
    // Map from path to `FileState`.
    watched: HashMap<PathBuf, FileState>,
}

impl PollWatch {
    /// Returns `true`, if a scan should happen, because enough time has
    /// elapsed.
    fn should_update(&mut self) -> bool {
        let now = SystemTime::now();
        if let Ok(elapsed) = now.duration_since(self.last_time) {
            if elapsed < self.interval {
                // Not enough time has passed
                false
            }
            else {
                // Enough time has passed
                self.last_time = now;
                true
            }
        }
        else {
            // Time went backwards, we simply update `self.last_time`, but don't
            // force an update
            self.last_time = now;
            false
        }
    }

    /// Updates the `FileState`s, if enough time has elapsed.
    fn update(&mut self) {
        if !self.should_update() {
            return;
        }

        for (p, state) in &mut self.watched {
            state.update(p, &mut self.events);
        }
    }
}

impl Watch for PollWatch {
    fn new() -> Result<Self> {
        Ok(Self{
            last_time: SystemTime::UNIX_EPOCH,
            interval: Duration::from_secs(1),
            events: VecDeque::new(),
            watched: HashMap::new(),
        })
    }

    fn watch(&mut self, p: impl AsRef<Path>, rec: Recursion) -> Result<()> {
        let p = p.as_ref();
        let state = FileState::from_path(p, rec)?;
        self.watched.insert(p.to_path_buf(), state);
        Ok(())
    }

    fn unwatch(&mut self, p: impl AsRef<Path>) {
        self.watched.remove(p.as_ref());
    }

    fn poll_event(&mut self) -> Option<Event> {
        self.update();
        self.events.pop_front()
    }

    fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}
