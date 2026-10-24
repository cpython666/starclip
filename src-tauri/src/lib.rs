#[cfg(feature = "desktop")]
mod desktop;
pub mod handwriting;
pub mod store;
#[cfg(feature = "desktop")]
pub use desktop::run;
