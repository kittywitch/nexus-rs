pub mod alert;
pub mod data_link;
pub mod event;
pub mod font;
pub mod gamebind;
pub mod gui;
pub mod hook;
pub mod keybind;
pub mod localization;
pub mod log;
pub mod paths;
pub mod quick_access;
pub mod texture;
pub mod updater;
pub mod v2;
pub mod v3;
pub mod v4;
pub mod v6;
pub mod wnd_proc;

#[cfg(feature = "rtapi")]
pub mod rtapi;

// export current supported version
pub use v6::*;

impl AddonApi {
    /// Returns the Nexus [`AddonApi`] instance.
    ///
    /// Panics if called before initialization.
    #[inline]
    pub fn get() -> &'static Self {
        crate::globals::addon_api()
    }
}
