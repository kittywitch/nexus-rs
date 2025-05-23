//! Event system.
//!
//! ```no_run
//! use nexus::{
//!     event::{ADDON_LOADED, event_consume},
//!     log::{log, LogLevel}
//! };
//! use std::ptr::NonNull;
//!
//! let callback = event_consume!(|payload: Option<&i32>| {
//!     if let Some(signature) = payload {
//!         log(LogLevel::Info, "My Addon", format!("Addon {signature} loaded"));
//!     }
//! });
//!
//! ADDON_LOADED.subscribe(callback);
//! ```

mod nexus;

#[cfg(feature = "arc")]
pub mod arc;

#[cfg(feature = "extras")]
pub mod extras;

#[cfg(feature = "rtapi")]
pub mod rtapi;

use super::EventApi;
use crate::{revertible::Revertible, util::str_to_c, AddonApi};
use std::{
    ffi::{c_char, c_void},
    marker::PhantomData,
    mem,
};

pub use self::nexus::*;

/// An event identifier & payload type pair.
#[derive(Debug, Clone, Copy)]
pub struct Event<T> {
    pub identifier: &'static str,
    _phantom: PhantomData<T>,
}

impl<T> Event<T> {
    /// Creates a new event identifier & payload type pair.
    ///
    /// # Safety
    /// See [`event_subscribe_typed`].
    #[inline]
    pub const unsafe fn new(identifier: &'static str) -> Self {
        Self {
            identifier,
            _phantom: PhantomData,
        }
    }

    /// Subscribes to the event.
    #[inline]
    pub fn subscribe(
        &self,
        callback: RawEventConsume<T>,
    ) -> Revertible<impl Fn() + Send + Sync + Clone + 'static> {
        unsafe { event_subscribe_typed(self.identifier, callback) }
    }

    /// Raises the event.
    #[inline]
    pub fn raise(&self, event_data: &T) {
        unsafe { event_raise(self.identifier, event_data) }
    }
}

pub type RawEventConsume<T> = extern "C-unwind" fn(event_args: *const T);

pub type RawEventConsumeUnknown = RawEventConsume<c_void>;

pub type RawEventRaise =
    unsafe extern "C-unwind" fn(identifier: *const c_char, event_data: *const c_void);

pub type RawEventRaiseNotification = unsafe extern "C-unwind" fn(identifier: *const c_char);

pub type RawEventRaiseTargeted = unsafe extern "C-unwind" fn(
    signature: i32,
    identifier: *const c_char,
    event_data: *const c_void,
);

pub type RawEventRaiseNotificationTargeted =
    unsafe extern "C-unwind" fn(signature: i32, identifier: *const c_char);

pub type RawEventSubscribe = unsafe extern "C-unwind" fn(
    identifier: *const c_char,
    consume_callback: RawEventConsumeUnknown,
);

/// Subscribes to an event with a raw callback using an unknown payload.
///
/// Returns a [`Revertible`] to revert the subscribe.
pub fn event_subscribe_unknown(
    identifier: impl AsRef<str>,
    callback: RawEventConsumeUnknown,
) -> Revertible<impl Fn() + Send + Sync + Clone + 'static> {
    let identifier = str_to_c(identifier, "failed to convert event identifier");
    let EventApi {
        subscribe,
        unsubscribe,
        ..
    } = AddonApi::get().event;
    unsafe { subscribe(identifier.as_ptr(), callback) };
    let revert = move || unsafe { unsubscribe(identifier.as_ptr(), callback) };
    revert.into()
}

/// Subscribes to an event with a raw callback using a typed payload.
///
/// Returns a [`Revertible`] to revert the subscribe.
///
/// # Safety
/// The passed event identifier must always come with valid data of the given type.
pub unsafe fn event_subscribe_typed<T>(
    identifier: impl AsRef<str>,
    callback: RawEventConsume<T>,
) -> Revertible<impl Fn() + Send + Sync + Clone + 'static> {
    let callback =
        unsafe { mem::transmute::<RawEventConsume<T>, RawEventConsumeUnknown>(callback) };
    event_subscribe_unknown(identifier, callback)
}

/// Unsubscribes a previously registered raw event callback.
pub fn event_unsubscribe(identifier: impl AsRef<str>, callback: RawEventConsumeUnknown) {
    let identifier = str_to_c(identifier, "failed to convert event identifier");
    let EventApi { unsubscribe, .. } = AddonApi::get().event;
    unsafe { unsubscribe(identifier.as_ptr(), callback) }
}

/// Macro to wrap an event callback.
///
/// Generates a [`RawEventConsume`] wrapper around the passed callback.
///
/// # Usage
/// ```no_run
/// # use nexus::event::*;
/// let event_callback = event_consume!(|data: Option<&i32>| {
///     use nexus::log::{log, LogLevel};
///     log(LogLevel::Info, "My Addon", format!("received event with data {data:?}"));
/// });
///
/// let event_callback = event_consume!(<i32> |data| {
///     use nexus::log::{log, LogLevel};
///     log(LogLevel::Info, "My Addon", format!("received event with data {data:?}"));
/// });
/// ```
///
/// ```no_run
/// # use nexus::event::*;
/// fn event_callback(data: Option<&i32>) {
///     use nexus::log::{log, LogLevel};
///     log(LogLevel::Info, "My Addon", format!("Received event with data {data:?}"));
/// }
/// let event_callback = event_consume!(<i32> event_callback);
/// ```
///
/// Note that the payload type corresponds to the pointee in Nexus documentation.
/// If you are interested in the pointer itself, you have to cast the obtained reference back to a pointer:
/// ```no_run
/// # use nexus::event::*;
/// use std::ffi::{c_char, CStr};
///
/// let event_callback = event_consume!(<c_char> |data| {
///     if let Some(data) = data {
///         let ptr = data as *const c_char;
///         let c_str = unsafe { CStr::from_ptr(ptr) };
///     }
/// });
/// ```
#[macro_export]
macro_rules! event_consume {
    ( < $ty:ty > $callback:expr $(,)? ) => {{
        const __CALLBACK: fn(::std::option::Option<&$ty>) = ($callback);

        extern "C-unwind" fn __event_callback_wrapper(data: *const $ty) {
            let _ = unsafe { ::std::mem::transmute::<*const $ty, *const ::std::ffi::c_void>(data) }; // size check
            __CALLBACK(unsafe { data.as_ref() })
        }

        __event_callback_wrapper
    }};
    ( $ty:ty , $callback:expr $(,)? ) => {
        $crate::event::event_consume!(<$ty> $callback)
    };
    ( | $arg:ident : Option<& $ty:ty > | $body:expr $(,)? ) => {
        $crate::event::event_consume!(<$ty> |$arg: Option<& $ty >| $body)
    };
    ( $callback:expr $(,)? ) => {{
        $crate::event::event_consume!(<()> $callback)
    }};
}

pub use event_consume;

/// Macro to subscribe to an event with a wrapped callback.
///
/// This macro is [unsafe](https://doc.rust-lang.org/std/keyword.unsafe.html).
/// See [`event_subscribe_typed`] for more information.
///
/// Returns a [`Revertible`] to revert the subscribe.
///
/// # Usage
/// ```no_run
/// # use nexus::event::*;
/// unsafe {
///     event_subscribe!("MY_EVENT" => i32, |data| {
///         use nexus::log::{log, LogLevel};
///         log(LogLevel::Info, "My Addon", format!("Received MY_EVENT with {data:?}"));
///     })
/// }.revert_on_unload();
/// ```
///
/// The event identifier may be dynamic and the callback can be a function name.
/// ```no_run
/// # use nexus::event::*;
/// let event: &str = "MY_EVENT";
/// fn event_callback(data: Option<&i32>) {
///     use nexus::log::{log, LogLevel};
///     log(LogLevel::Info, "My Addon", format!("Received MY_EVENT with {data:?}"));
/// }
/// let revertible = unsafe { event_subscribe!(event => i32, event_callback) };
/// revertible.revert();
/// ```
///
/// The `unsafe` keyword can be moved into the macro call:
/// ```no_run
/// # use nexus::event::*;
/// # fn event_callback(_: Option<&()>) {}
/// event_subscribe!(unsafe "MY_EVENT" => (), event_callback);
/// ```
/// Note that the payload type corresponds to the pointee in Nexus documentation.
/// If you are interested in the pointer itself, you have to cast the obtained reference back to a pointer:
/// ```no_run
/// # use nexus::event::*;
/// use std::ffi::{c_char, CStr};
///
/// event_subscribe!(unsafe "EV_ACCOUNT_NAME" => c_char, |data| {
///     if let Some(data) = data {
///         let ptr = data as *const c_char;
///         let c_str = unsafe { CStr::from_ptr(ptr) };
///     }
/// });
/// ```
///
/// # Safety
/// See [`event_subscribe_typed`].
#[macro_export]
macro_rules! event_subscribe {
    ( unsafe $event:expr , $ty:ty , $callback:expr $(,)? ) => {
        unsafe { $crate::event::event_subscribe!($event => $ty, $callback) }
    };
    ( unsafe $event:expr => $ty:ty , $callback:expr $(,)? ) => {
        unsafe { $crate::event::event_subscribe!($event => $ty, $callback) }
    };
    ( $event:expr , $ty:ty , $callback:expr $(,)? ) => {
        $crate::event::event_subscribe!($event => $ty, $callback)
    };
    ( $event:expr => $ty:ty , $callback:expr $(,)? ) => {
        $crate::event::event_subscribe_typed($event, $crate::event::event_consume!(<$ty> $callback))
    };
}

pub use event_subscribe;

/// Raises an event to all subscribing addons.
///
/// # Safety
/// The passed event identifier must be associated with data of the given type.
pub unsafe fn event_raise<T>(identifier: impl AsRef<str>, event_data: &T) {
    let identifier = str_to_c(identifier, "failed to convert event identifier");
    let data: *const _ = event_data;
    let EventApi { raise, .. } = AddonApi::get().event;
    unsafe { raise(identifier.as_ptr(), data.cast()) }
}

/// Raises an event without payload to all subscribing addons.
pub fn event_raise_notification(identifier: impl AsRef<str>) {
    let identifier = str_to_c(identifier, "failed to convert event identifier");
    let EventApi {
        raise_notification, ..
    } = AddonApi::get().event;
    unsafe { raise_notification(identifier.as_ptr()) }
}

/// Raises an event for a specific subscribing addon.
///
/// # Safety
/// See [`event_raise`].
pub unsafe fn event_raise_targeted<T>(signature: i32, identifier: impl AsRef<str>, event_data: &T) {
    let identifier = str_to_c(identifier, "failed to convert event identifier");
    let data: *const _ = event_data;
    let EventApi { raise_targeted, .. } = AddonApi::get().event;
    unsafe { raise_targeted(signature, identifier.as_ptr(), data.cast()) }
}

/// Raises an event without payload for a specific subscribing addon.
pub fn event_raise_notification_targeted(signature: i32, identifier: impl AsRef<str>) {
    let identifier = str_to_c(identifier, "failed to convert event identifier");
    let EventApi {
        raise_notification_targeted,
        ..
    } = AddonApi::get().event;
    unsafe { raise_notification_targeted(signature, identifier.as_ptr()) }
}
