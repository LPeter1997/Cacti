//! Cross-platform and dependency-free archive handling.
// TODO: doc, introduce library

mod deflate;
pub use deflate::Inflate;

mod zip;
//pub use zip::ZipArchive;
