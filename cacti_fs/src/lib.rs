//! Various dependency-free, cross-platform utilities for filesystems.

/// Provides a single trait `FilePath` that's implemented for `fs::File`, so it
/// can return it's own pocation using a `.path()` function, if possible.
pub mod path;
/// Provides a minimal set of functions for dealing with unique temporary
/// paths, files and directories.
pub mod temp;
/// Provides ways do monitor for filesystem changes under different paths.
pub mod watch;
