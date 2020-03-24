//! A minimalistic, dependency-free, cross-platform filesystem watch library.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// The `Result` type of this library.
pub type Result<T> = io::Result<T>;

/// A filesystem watch that can listen to changes in files and folder
/// structures.
///
/// # Examples
///
/// Watching `"C:/foo/bar.txt"` for changes, using the default `Watch` for this
/// platform:
///
/// ```no_run
/// use std::time::Duration;
///
/// // Ask for the default on this platform.
/// let mut watch = fs_watch::default_watch()?;
///
/// // We can set the interval, in case this is a platform with no better
/// // strategy to support other than polling. For a single file this isn't a
/// // huge deal, but for larger folder structures it's important to keep poll
/// // intervals as long as tolerable.
/// // Here we just allow polling twice every second.
/// watch.set_interval(Duration::from_millis(500));
///
/// // Register the logged path, we don't need recursion for a file
/// watch.watch("C:/foo/bar.txt", Recursion::NotRecursive)?;
///
/// // For simplicity we just loop and log
/// loop {
///     while let Some(ev_result) = watch.poll_event() {
///         match ev_result {
///             Ok(ev) => println!("Event happened: {:?}", ev),
///             Err(err) => println!("Error happened: {}", err),
///         }
///     }
/// }
/// ```
pub trait Watch: Sized {
    /// Creates a new `Watch` with no files watched.
    ///
    /// # Errors
    ///
    /// In case of an IO or system error, an error variant is returned.
    fn new() -> Result<Self>;

    /// Starts watching a given `Path` with the given recursion setting.
    ///
    /// In case the given `Path` is already watched, the new settings will
    /// override it.
    ///
    /// # Errors
    ///
    /// In case of an IO or system error, an error variant is returned. In an
    /// error is returned, the passed in path is not added to the watchlist.
    fn watch(&mut self, p: impl AsRef<Path>, rec: Recursion) -> Result<()>;

    /// Stops watching a given `Path`.
    fn unwatch(&mut self, p: impl AsRef<Path>);

    /// Reads the next `Event`. Returns `None` if there's no `Event` to consume.
    ///
    /// # Errors
    ///
    /// In case of an IO or system error, an error variant is returned. This
    /// could be caused by access policy changes for example, so it's a normal
    /// part of the event queue.
    fn poll_event(&mut self) -> Option<Result<Event>>;

    /// In case of a `Watch` that relies on some polling technique, this sets
    /// the interval that polling should be performed. If a `Watch` relies on
    /// filesystem notifications, this interval is unused.
    fn set_interval(&mut self, _interval: Duration) { }
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
pub struct Event {
    /// When the `Event` happened.
    pub time: SystemTime,
    /// The path the `Event` is relevant for.
    pub path: PathBuf,
    /// The kind of operation the `Event` represents.
    pub kind: EventKind,
}

impl Event {
    /// Creates a new `Event` with the provided data.
    fn new(time: SystemTime, path: impl AsRef<Path>, kind: EventKind) -> Self {
        Self{ time, path: path.as_ref().to_path_buf(), kind }
    }

    /// Creates a new `Event` with kind `EventKind::Create`.
    fn create(time: SystemTime, path: impl AsRef<Path>) -> Self {
        Self::new(time, path, EventKind::Create)
    }

    /// Creates a new `Event` with kind `EventKind::Modify`.
    fn modify(time: SystemTime, path: impl AsRef<Path>) -> Self {
        Self::new(time, path, EventKind::Modify)
    }

    /// Creates a new `Event` with kind `EventKind::Delete`.
    fn delete(time: SystemTime, path: impl AsRef<Path>) -> Self {
        Self::new(time, path, EventKind::Delete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The watched path has something created.
    Create,
    /// The watched path has something modified.
    Modify,
    /// The watched path has something deleted.
    Delete,
}

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// Null ////////////////////////////////////////////////////////////////////////

/// The simplest `Watch` implementation, that basically doesn't watch anything.
/// This is to not to break the library when a platform doesn't support even the
/// simplest method of polling file info.
pub struct NullWatch;

impl Watch for NullWatch {
    fn new() -> Result<Self> {
        Ok(Self)
    }

    fn watch(&mut self, _p: impl AsRef<Path>, _rec: Recursion) -> Result<()> {
        Ok(())
    }

    fn unwatch(&mut self, _p: impl AsRef<Path>) { }

    fn poll_event(&mut self) -> Option<Result<Event>> {
        None
    }
}

// Polling /////////////////////////////////////////////////////////////////////

/// A very trivial `Watch` implementation that polls file status between time
/// intervals.
///
/// **Important**: Polling only tries to poll file statuses, when the `Event`s
/// are polled from the `PollWatch` and enough time is elapsed based on the
/// given interval.
#[derive(Debug)]
pub struct PollWatch {
    last_time: SystemTime,
    interval: Duration,
    events: VecDeque<Result<Event>>,
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
    /// Creates a new `PollWatch` with one second intervals between polls.
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
        let state = FileState::new(p, rec, &mut self.events);
        self.watched.insert(p.to_path_buf(), state);
        Ok(())
    }

    fn unwatch(&mut self, p: impl AsRef<Path>) {
        self.watched.remove(p.as_ref());
    }

    fn poll_event(&mut self) -> Option<Result<Event>> {
        self.update();
        self.events.pop_front()
    }

    /// Sets the time interval for polling.
    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }
}

/// File states for the `PollWatch`.
#[derive(Debug)]
enum FileState {
    NotExisting{
        rec: Recursion,
    },
    ExistingFile{
        rec: Recursion,
        mod_time: SystemTime,
    },
    ExistingDirectory{
        rec: Recursion,
        mod_time: SystemTime,
        substates: HashMap<PathBuf, FileState>,
    }
}

impl FileState {
    /// Returns the creation-time of a path, jumping through the `Result` chain.
    fn ctime(path: impl AsRef<Path>) -> Result<SystemTime> {
        let i = fs::metadata(path)?;
        i.modified().or_else(|_| i.created())
    }

    /// Returns the modification-time of a path, jumping through the `Result`
    /// chain.
    fn mtime(path: impl AsRef<Path>) -> Result<SystemTime> {
        fs::metadata(path).and_then(|i| i.modified())
    }

    /// Creates a `FileState`, only logging errors.
    fn new(
        path: impl AsRef<Path>,
        rec: Recursion,
        events: &mut VecDeque<Result<Event>>,
    ) -> Self {
        Self::new_internal(path, rec, false, events)
    }

    /// Creates a `FileState` assuming that the given path has been watched
    /// before and it needs to log changes.
    fn new_created(
        path: impl AsRef<Path>,
        rec: Recursion,
        events: &mut VecDeque<Result<Event>>,
    ) -> Self {
        Self::new_internal(path, rec, true, events)
    }

    /// Creates a `FileState` for the given path, while logging errors. If
    /// `_create` is `true`, creation `Event`s are also logged.
    fn new_internal(
        path: impl AsRef<Path>,
        rec: Recursion,
        log_create: bool,
        events: &mut VecDeque<Result<Event>>,
    ) -> Self {
        let path = path.as_ref();
        if !path.exists() {
            return Self::NotExisting{ rec };
        }
        // Exists
        let mod_time = (if log_create { Self::ctime(path) } else { Self::mtime(path) })
            .unwrap_or_else(|_| SystemTime::now());
        if log_create {
            // Log that it got created
            events.push_back(Ok(Event::create(mod_time, path)));
        }
        if path.is_file() {
            return Self::ExistingFile{ rec, mod_time };
        }
        // Directory
        let mut substates = HashMap::new();
        if rec == Recursion::Recursive {
            // Create recursively
            let subdirs = fs::read_dir(path);
            if subdirs.is_err() {
                // Log error
                events.push_back(Err(subdirs.unwrap_err()));
            }
            else {
                for subdir in subdirs.unwrap() {
                    if subdir.is_err() {
                        // Log error
                        events.push_back(Err(subdir.unwrap_err()));
                    }
                    else {
                        let subpath = subdir.unwrap().path();
                        let substate = Self::new_internal(&subpath, rec, log_create, events);
                        substates.insert(subpath, substate);
                    }
                }
            }
        }
        return Self::ExistingDirectory{ rec, mod_time, substates };
    }

    /// Updates this `FileState` at the given path.
    fn update(&mut self, path: impl AsRef<Path>, events: &mut VecDeque<Result<Event>>) {
        let path = path.as_ref();
        match self {
            Self::NotExisting{ rec } => {
                if path.exists() {
                    // Update state while logging everything
                    *self = Self::new_created(path, *rec, events);
                    return;
                }
                // Nothing changed
            },

            Self::ExistingFile{ rec, mod_time } => {
                if !path.exists() {
                    // File no longer exists!
                    let rec = *rec;
                    self.delete_rec(path, SystemTime::now(), events);
                    *self = Self::NotExisting{ rec };
                    return;
                }
                if !path.is_file() {
                    // No longer a file, first delete then update state while
                    // logging everything
                    let rec = *rec;
                    self.delete_rec(path, SystemTime::now(), events);
                    *self = Self::new_created(path, rec, events);
                    return;
                }
                // Still file, check modification date
                if let Ok(mtime) = Self::mtime(path) {
                    if mtime > *mod_time {
                        events.push_back(Ok(Event::modify(mtime, path)));
                        *mod_time = mtime;
                    }
                }
            },

            Self::ExistingDirectory{ rec, mod_time, substates } => {
                if !path.exists() {
                    // Directory no longer exists!
                    let rec = *rec;
                    self.delete_rec(path, SystemTime::now(), events);
                    *self = Self::NotExisting{ rec };
                    return;
                }
                if !path.is_dir() {
                    // No longer a directory, first delete then update state
                    // while logging everything
                    let rec = *rec;
                    self.delete_rec(path, SystemTime::now(), events);
                    *self = Self::new_created(path, rec, events);
                    return;
                }
                // Still directory
                if *rec == Recursion::Recursive {
                    // Check for new entries
                    let subdirs = fs::read_dir(path);
                    if subdirs.is_err() {
                        // Log error
                        events.push_back(Err(subdirs.unwrap_err()));
                    }
                    else {
                        for subdir in subdirs.unwrap() {
                            if subdir.is_err() {
                                // Log error
                                events.push_back(Err(subdir.unwrap_err()));
                            }
                            else {
                                let subpath = subdir.unwrap().path();
                                if !substates.contains_key(&subpath) {
                                    // New thing
                                    let substate = Self::new_created(&subpath, *rec, events);
                                    substates.insert(subpath, substate);
                                }
                            }
                        }
                    }
                    // Update and prune existing entries
                    let mut to_remove = Vec::new();
                    for (subpath, subdir) in substates.iter_mut() {
                        subdir.update(subpath, events);
                        match subdir {
                            Self::NotExisting{ .. } => to_remove.push(subpath.clone()),
                            _ => {},
                        }
                    }
                    for path in to_remove {
                        substates.remove(&path);
                    }
                }
                // Check modification date
                if let Ok(mtime) = Self::mtime(path) {
                    if mtime > *mod_time {
                        events.push_back(Ok(Event::modify(mtime, path)));
                        *mod_time = mtime;
                    }
                }
            },
        }
    }

    /// Recursively writes delete operations for all substates.
    fn delete_rec(
        &self,
        path: impl AsRef<Path>,
        timestamp: SystemTime,
        events: &mut VecDeque<Result<Event>>,
    ) {
        match self {
            Self::NotExisting{ .. } => { /* no-op */ },

            Self::ExistingFile{ .. } => {
                // A file got deleted
                events.push_back(Ok(Event::delete(timestamp, path)));
            },

            Self::ExistingDirectory{ substates, .. } => {
                // Could have substates to delete, do that first
                for (subpath, substate) in substates.iter() {
                    substate.delete_rec(subpath, timestamp, events);
                }
                // Then delete folder
                events.push_back(Ok(Event::delete(timestamp, path)));
            }
        }
    }
}
