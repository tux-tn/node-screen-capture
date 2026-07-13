#![deny(clippy::all)]

#[cfg(target_os = "windows")]
mod windows_binding;

#[cfg(target_os = "windows")]
pub use windows_binding::*;

#[cfg(not(target_os = "windows"))]
use napi_derive::napi;

#[cfg(not(target_os = "windows"))]
#[napi]
pub const fn is_supported() -> bool {
  false
}
