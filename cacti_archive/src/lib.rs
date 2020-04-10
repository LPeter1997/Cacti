//! Cross-platform and dependency-free archive handling.

mod deflate;
pub use deflate::Inflate;

mod zip;
//pub use zip::ZipArchive;
