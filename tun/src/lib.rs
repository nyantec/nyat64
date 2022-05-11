///! Tun devices
mod sync;

#[cfg(feature = "async")]
mod r#async;

#[cfg(feature = "async")]
#[doc(inline)]
pub use r#async::AsyncTunSocket;
#[doc(inline)]
pub use sync::TunSocket;
