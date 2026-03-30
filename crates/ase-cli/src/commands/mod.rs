//! CLI command implementations.

mod init;
mod validate;
mod list;
mod select;
mod test;
mod bench;
mod optimize;
mod stats;
mod index;
mod export;

#[cfg(feature = "http-server")]
mod serve;

pub use init::{init, InitArgs};
pub use validate::{validate, ValidateArgs};
pub use list::{list, ListArgs};
pub use select::{select, SelectArgs};
pub use test::{test, TestArgs};
pub use bench::{bench, BenchArgs};
pub use optimize::{optimize, OptimizeArgs};
pub use stats::{stats, StatsArgs};
pub use index::{index, IndexArgs};
pub use export::{export, ExportArgs};

#[cfg(feature = "http-server")]
pub use serve::{serve, ServeArgs};
