#![deny(clippy::all)]

mod common;

#[cfg(target_os = "windows")]
mod windows_binding;

#[cfg(target_os = "windows")]
pub use windows_binding::*;

#[cfg(target_os = "linux")]
mod linux_binding;

#[cfg(target_os = "linux")]
pub use linux_binding::*;

#[cfg(target_os = "macos")]
mod macos_binding;

#[cfg(target_os = "macos")]
pub use macos_binding::*;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
use napi_derive::napi;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
#[napi]
pub const fn is_supported() -> bool {
  false
}
