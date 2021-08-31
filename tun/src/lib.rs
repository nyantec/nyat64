///! Tun devices
mod sync;

#[cfg(feature = "async")]
mod r#async;

#[doc(inline)]
pub use sync::TunSocket;

#[cfg(feature = "async")]
#[doc(inline)]
pub use r#async::AsyncTunSocket;
