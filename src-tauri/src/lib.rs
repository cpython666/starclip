#[cfg(feature = "desktop")]
mod desktop;
pub mod handwriting;
pub mod storage;
pub mod store;
#[cfg(feature = "desktop")]
pub use desktop::run;
