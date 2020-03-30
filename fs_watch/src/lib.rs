//! Cross-platform utility for monitoring filesystem changes.
//!
//! # Usage
//!
//! TODO
//!
//! # Porting the library to other platforms
//!
//! TODO

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use std::collections::{HashMap, VecDeque};
use std::io::Result;
use std::fs;
use std::io;

// ////////////////////////////////////////////////////////////////////////// //
//                                    API                                     //
// ////////////////////////////////////////////////////////////////////////// //

/// A filesystem watch that can listen to changes in files and folder
/// structures.
///
/// # Examples
///
/// Watching `"C:/foo/bar.txt"` for changes, using the default `Watch` for this
/// platform:
///
/// ```no_run
/// use fs_watch::*;
/// use std::time::Duration;
///
/// # fn main() -> std::io::Result<()> {
/// // Ask for the default on this platform.
/// let mut watch = DefaultWatch::new()?;
///
/// // We can set the interval, in case this is a platform with no better
/// // strategy to support other than polling, or due to some platform-specific
/// // behavior. For a single file this isn't a huge deal, but for larger folder
/// // structures it's important to keep poll intervals as long as tolerable.
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
/// # }
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
    /// the interval that polling should be performed.
    ///
    /// If a `Watch` relies on filesystem notifications, this interval could be
    /// unused, or set some platform-specific waiting behavior. For further
    /// information, read the platform-specific `Watch` implementations
    /// documentation.
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The operations that can be done on a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The watched path has something created.
    Create,
    /// The watched path has something modified.
    Modify,
    /// The watched path has something deleted.
    Delete,
}

/// The default, recommended `Watch` implementation for the platform.
pub type DefaultWatch = DefaultWatchImpl;

// ////////////////////////////////////////////////////////////////////////// //
//                               Implementation                               //
// ////////////////////////////////////////////////////////////////////////// //

// Null ////////////////////////////////////////////////////////////////////////

/// The simplest `Watch` implementation, that basically doesn't watch anything.
/// This is to not to break the library when a platform doesn't support even the
/// simplest method of polling file info.
#[derive(Debug)]
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
    /// `log_create` is `true`, creation `Event`s are also logged.
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
                    else {
                        println!("NO MTIME FOR YA");
                    }
                }
                else {
                    println!("FAILED TO READ MTIME NOOO");
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

// WinAPI, ReadDirectoryChangesW  //////////////////////////////////////////////

#[cfg(target_os = "windows")]
mod win32 {
    #![allow(non_snake_case)]

    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::mem;
    use std::ptr;
    use super::*;

    // Error type
    const ERROR_SUCCESS: u32 = 0;
    // File access
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    // File share
    const FILE_SHARE_READ  : u32 = 0x0001;
    const FILE_SHARE_WRITE : u32 = 0x0002;
    const FILE_SHARE_DELETE: u32 = 0x0004;
    // File creation disposition
    const OPEN_EXISTING: u32 = 3;
    // File flags and attributes
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OVERLAPPED      : u32 = 0x40000000;
    // File change notification filters
    const FILE_NOTIFY_CHANGE_FILE_NAME : u32 = 0x00000001;
    const FILE_NOTIFY_CHANGE_DIR_NAME  : u32 = 0x00000002;
    const FILE_NOTIFY_CHANGE_LAST_WRITE: u32 = 0x00000010;

    // Returned by handle-returning functions on failure
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

    type OverlappedCompletionRoutine =
        Option<unsafe extern "system" fn(u32, u32, *mut c_void)>;

    #[repr(C)]
    struct OVERLAPPED {
        Internal    : u64        ,
        InternalHigh: u64        ,
        Offset      : u32        ,
        OffsetHigh  : u32        ,
        hEvent      : *mut c_void,
    }

    impl OVERLAPPED {
        fn zeroed() -> Self {
            unsafe{ mem::zeroed() }
        }
    }

    #[repr(C)]
    struct FILE_NOTIFY_INFORMATION {
        NextEntryOffset: u32     ,
        Action         : u32     ,
        FileNameLength : u32     ,
        FileName       : *mut u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLastError() -> u32;

        fn CreateFileW(
            file_name : *const u16 ,
            access    : u32        ,
            share_mode: u32        ,
            security  : *mut c_void,
            crea_disp : u32        ,
            attr_flags: u32        ,
            template  : *mut c_void,
        ) -> *mut c_void;

        fn CloseHandle(
            handle: *mut c_void,
        ) -> i32;

        fn ReadDirectoryChangesW(
            directory_handle: *mut c_void                ,
            res_buffer      : *mut c_void                ,
            res_buffer_len  : u32                        ,
            recursive       : i32                        ,
            filter          : u32                        ,
            bytes_written   : *mut u32                   ,
            overlapped      : *mut OVERLAPPED            ,
            callback        : OverlappedCompletionRoutine,
        ) -> i32;

        fn SleepEx(
            millis   : u32,
            alertable: i32,
        ) -> u32;
    }

    /// Returns the last OS error represented as an `io::Error`.
    fn last_error() -> io::Error {
        let error = unsafe { GetLastError() };
        io::Error::from_raw_os_error(error as i32)
    }

    /// Converts the Rust &OsStr into a WinAPI WCHAR string.
    fn to_wstring(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0).into_iter()).collect()
    }

    /// Forces the thread to go to sleep for the given amount of milliseconds,
    /// allowing asynchronous operations to complete.
    fn sleep(millis: u32) {
        unsafe { SleepEx(millis, 1) };
    }

    /// Opens a file/folder for observing only.
    fn open_handle_for_observe(path: &Path) -> Result<*mut c_void> {
        let handle = unsafe { CreateFileW(
            to_wstring(path.as_os_str()).as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            ptr::null_mut()) };
        if handle == INVALID_HANDLE_VALUE {
            Err(last_error())
        }
        else {
            Ok(handle)
        }
    }

    /// Closes the file handle.
    fn close_handle(handle: *mut c_void) {
        unsafe { CloseHandle(handle) };
    }

    /// Reinterprets the given `DWORD` buffer to `FILE_NOTIFY_INFORMATION`s and
    /// feeds them into a custom user-function.
    fn read_file_notify_information<F>(buffer: *const u32, bytes_transferred: u32, mut f: F)
        where F: FnMut(&FILE_NOTIFY_INFORMATION) {
        if bytes_transferred == 0 {
            // We can't even trust the first entry.
            return;
        }
        // We can at least look at the first entry
        let mut buffer: *const FILE_NOTIFY_INFORMATION = buffer.cast();
        loop {
            let entry = unsafe{ &*buffer };
            f(entry);
            if entry.NextEntryOffset == 0 {
                // We are done
                return;
            }
            // Go to the next entry
            let buffer8: *const u8 = buffer.cast();
            buffer = unsafe { buffer8.offset(entry.NextEntryOffset as isize).cast() };
        }
    }

    /// Subscribes to the next change notification for the given handle, with
    /// the given result-buffer, callback and user-pointer.
    fn subscribe_to_next_change<U>(
        handle: *mut c_void,
        recursive: bool,
        result_buffer: &mut [u32],
        callback: OverlappedCompletionRoutine,
        user: *mut U,
    ) -> Result<()> {
        let mut bw: u32 = 0;

        let mut overlapped = OVERLAPPED::zeroed();
        overlapped.hEvent = user.cast();

        if unsafe { ReadDirectoryChangesW(
            handle,
            result_buffer.as_mut_ptr().cast(),
            (result_buffer.len() * mem::size_of::<u32>()) as u32,
            if recursive { 1 } else { 0 },
            FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE,
            &mut bw,
            &mut overlapped,
            callback) } == 0 {
            Err(last_error())
        }
        else {
            Ok(())
        }
    }

    /// The WinAPI-based watch, using `ReadDirectoryChangesW`.
    pub struct WinApiWatch {
    }

    impl Watch for WinApiWatch {
        fn new() -> Result<Self> {
            Ok(Self{ })
        }

        fn watch(&mut self, p: impl AsRef<Path>, rec: Recursion) -> Result<()> {
            let path = p.as_ref();
            if !path.exists() {
                // Watch the closest parent that DOES exist
                // Or just check if it exists when updating, roughly polling existance
                unimplemented!();
            }
            else {
                // Watch parent directory non-recursively
                // if file, then it's enough
                // if directory, then watch self recursively
                unimplemented!();
            }
        }

        fn unwatch(&mut self, p: impl AsRef<Path>) {
            unimplemented!();
        }

        fn poll_event(&mut self) -> Option<Result<Event>> {
            unimplemented!();
        }

        fn set_interval(&mut self, _interval: Duration) {
            unimplemented!();
        }
    }
}

// Choosing the default for the OS.
#[cfg(target_os = "windows")]      type DefaultWatchImpl = win32::WinApiWatch;
#[cfg(not(target_os = "windows"))] type DefaultWatchImpl = PollWatch;

#[cfg(test)]
mod tests {
    use super::*;
    use fs_path::FilePath;
    use std::thread;
    use std::io::Write;

    // TODO: Would make a nice macro
    fn cat_path(root: &Path, name: &str) -> PathBuf {
        let mut path = root.to_path_buf();
        path.push(name);
        path
    }

    fn create_file_in(root: &Path, name: &str) -> Result<fs::File> {
        fs::File::create(cat_path(root, name))
    }

    #[test]
    fn test_null_watch() -> Result<()> {
        let dir = fs_temp::directory()?;
        let mut w = NullWatch::new()?;
        w.watch(dir.path(), Recursion::Recursive)?;
        w.set_interval(Duration::from_millis(0));

        let _f = create_file_in(dir.path(), "foo.txt");

        assert!(w.poll_event().is_none());

        Ok(())
    }

    #[test]
    fn test_poll_watch_recursive_create_modify_delete() -> Result<()> {
        let dir = fs_temp::directory()?;
        let mut w = PollWatch::new()?;
        let dir_canon = fs::canonicalize(dir.path())?;
        w.watch(&dir_canon, Recursion::Recursive)?;
        w.set_interval(Duration::from_millis(0));

        println!("STAGE 1");

        assert!(w.poll_event().is_none());

        println!("STAGE 2");

        let foo_path = cat_path(&dir_canon, "foo.txt");
        println!("PATH {:?} MTIME: {:?}", dir_canon, fs::metadata(&dir_canon)?.modified());
        println!("Foo path {:?}", foo_path);
        // Create
        {
            {
                let mut f = create_file_in(&dir_canon, "foo.txt")?;
                println!("Foo ACTUAL path {:?}", f.path());
                f.write_all("Hello".as_bytes())?;
                f.sync_all()?;
            }
            {
                println!("PATH {:?} MTIME2: {:?}", dir_canon, fs::metadata(&dir_canon)?.modified());
                println!("STAGE 3");
                // An event for file creation
                let e = w.poll_event().unwrap().unwrap();
                assert_eq!(e.kind, EventKind::Create);
                assert_eq!(
                    fs::canonicalize(e.path)?,
                    fs::canonicalize(&foo_path)?
                );
                // An event for directory modification
                // TODO: This looks like not propagated event handling to me...
                return Ok(());
                let e = w.poll_event().unwrap().unwrap();
                assert_eq!(e.kind, EventKind::Modify);
                assert_eq!(
                    fs::canonicalize(e.path)?,
                    dir_canon
                );
                // No more
                assert!(w.poll_event().is_none());
            }
        }
        // Modify
        {
            {
                let mut f = create_file_in(dir.path(), "foo.txt")?;
                f.write_all("Hello".as_bytes())?;
            }
            {
                // An event for file modification
                let e = w.poll_event().unwrap().unwrap();
                assert_eq!(e.kind, EventKind::Modify);
                assert_eq!(
                    fs::canonicalize(e.path)?,
                    fs::canonicalize(&foo_path)?
                );
                // No more
                assert!(w.poll_event().is_none());
            }
        }
        // Delete
        {
            {
                fs::remove_file(&foo_path)?;
            }
            {
                // An event for file delete
                let e = w.poll_event().unwrap().unwrap();
                assert_eq!(e.kind, EventKind::Delete);
                // We can't canonicalize anymore
                assert!(e.path.ends_with("foo.txt"));
                // An event for directory modification
                let e = w.poll_event().unwrap().unwrap();
                assert_eq!(e.kind, EventKind::Modify);
                assert_eq!(
                    fs::canonicalize(e.path)?,
                    fs::canonicalize(dir.path())?
                );
                // No more
                assert!(w.poll_event().is_none());
            }
        }

        Ok(())
    }
}
