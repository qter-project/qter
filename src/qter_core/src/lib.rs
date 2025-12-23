#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::missing_panics_doc
)]

pub mod architectures;
mod shared_facelet_detection;
pub mod table_encoding;

mod runtime;
pub use runtime::*;

