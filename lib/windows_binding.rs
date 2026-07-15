use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::common::{bytes_per_pixel, crop_buffer, error};
use napi::Status;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
  ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue,
};
use napi_derive::napi;
use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Performance::QueryPerformanceFrequency;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
  MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_QUIT,
};
use windows::core::Interface;
use windows_capture_rs::capture::{
  CaptureControl as RustCaptureControl, Context, GraphicsCaptureApiHandler,
};
use windows_capture_rs::dxgi_duplication_api::{
  DxgiDuplicationApi, DxgiDuplicationFormat as RustDxgiFormat, Error as DxgiError,
};
use windows_capture_rs::encoder::{
  AudioSettingsBuilder, AudioSettingsSubType as RustAudioSubtype, ContainerSettingsBuilder,
  ContainerSettingsSubType as RustContainerSubtype, ImageEncoder as RustImageEncoder,
  ImageEncoderPixelFormat as RustPixelFormat, ImageFormat as RustImageFormat,
  VideoEncoder as RustVideoEncoder, VideoSettingsBuilder, VideoSettingsSubType as RustVideoSubtype,
};
use windows_capture_rs::frame::Frame as RustFrame;
use windows_capture_rs::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
use windows_capture_rs::graphics_capture_picker::GraphicsCapturePicker;
use windows_capture_rs::monitor::Monitor as RustMonitor;
use windows_capture_rs::settings::{
  ColorFormat as RustColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
  MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture_rs::window::Window as RustWindow;

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum ColorFormat {
  Rgba16F,
  Rgba8,
  Bgra8,
}

impl From<ColorFormat> for RustColorFormat {
  fn from(value: ColorFormat) -> Self {
    match value {
      ColorFormat::Rgba16F => Self::Rgba16F,
      ColorFormat::Rgba8 => Self::Rgba8,
      ColorFormat::Bgra8 => Self::Bgra8,
    }
  }
}

impl From<RustColorFormat> for ColorFormat {
  fn from(value: RustColorFormat) -> Self {
    match value {
      RustColorFormat::Rgba16F => Self::Rgba16F,
      RustColorFormat::Rgba8 => Self::Rgba8,
      RustColorFormat::Bgra8 => Self::Bgra8,
    }
  }
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum ImageFormat {
  Jpeg,
  Png,
  Gif,
  Tiff,
  Bmp,
  JpegXr,
}

impl From<ImageFormat> for RustImageFormat {
  fn from(value: ImageFormat) -> Self {
    match value {
      ImageFormat::Jpeg => Self::Jpeg,
      ImageFormat::Png => Self::Png,
      ImageFormat::Gif => Self::Gif,
      ImageFormat::Tiff => Self::Tiff,
      ImageFormat::Bmp => Self::Bmp,
      ImageFormat::JpegXr => Self::JpegXr,
    }
  }
}

#[napi(object)]
pub struct DirtyRegion {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

#[napi(object)]
pub struct Rect {
  pub left: i32,
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
}

#[napi]
pub struct Frame {
  buffer: Vec<u8>,
  width: u32,
  height: u32,
  row_pitch: u32,
  depth_pitch: u32,
  timestamp: i64,
  color_format: ColorFormat,
  dirty_regions: Vec<DirtyRegion>,
}

#[napi]
impl Frame {
  #[napi(getter)]
  pub fn buffer(&self) -> Buffer {
    self.buffer.clone().into()
  }

  #[napi(getter)]
  pub const fn width(&self) -> u32 {
    self.width
  }

  #[napi(getter)]
  pub const fn height(&self) -> u32 {
    self.height
  }

  #[napi(getter)]
  pub const fn row_pitch(&self) -> u32 {
    self.row_pitch
  }

  #[napi(getter)]
  pub const fn depth_pitch(&self) -> u32 {
    self.depth_pitch
  }

  #[napi(getter)]
  pub const fn timestamp(&self) -> i64 {
    self.timestamp
  }

  #[napi(getter)]
  pub fn color_format(&self) -> ColorFormat {
    self.color_format.clone()
  }

  #[napi(getter)]
  pub fn dirty_regions(&self) -> Vec<DirtyRegion> {
    self
      .dirty_regions
      .iter()
      .map(|region| DirtyRegion {
        x: region.x,
        y: region.y,
        width: region.width,
        height: region.height,
      })
      .collect()
  }

  #[napi]
  pub fn crop(&self, start_x: u32, start_y: u32, end_x: u32, end_y: u32) -> Result<Self> {
    let (buffer, width, height) = crop_buffer(
      &self.buffer,
      self.width,
      self.height,
      self.row_pitch,
      bytes_per_pixel(matches!(self.color_format, ColorFormat::Rgba16F)),
      start_x,
      start_y,
      end_x,
      end_y,
    )?;
    let row_pitch = width * bytes_per_pixel(matches!(self.color_format, ColorFormat::Rgba16F));

    Ok(Self {
      buffer,
      width,
      height,
      row_pitch,
      depth_pitch: row_pitch * height,
      timestamp: self.timestamp,
      color_format: self.color_format.clone(),
      dirty_regions: Vec::new(),
    })
  }

  #[napi]
  pub fn encode(&self, format: ImageFormat) -> Result<Buffer> {
    let pixel_format = match self.color_format {
      ColorFormat::Rgba8 => RustPixelFormat::Rgba8,
      ColorFormat::Bgra8 => RustPixelFormat::Bgra8,
      ColorFormat::Rgba16F => return Err(error("Rgba16F cannot be encoded as an image")),
    };
    RustImageEncoder::new(format.into(), pixel_format)
      .and_then(|encoder| encoder.encode(&self.buffer, self.width, self.height))
      .map(Buffer::from)
      .map_err(error)
  }

  #[napi]
  pub fn save_as_image(&self, path: String, format: Option<ImageFormat>) -> Result<()> {
    std::fs::write(
      path,
      self.encode(format.unwrap_or(ImageFormat::Png))?.as_ref(),
    )
    .map_err(error)
  }
}

type FrameCallback =
  Arc<ThreadsafeFunction<Frame, UnknownReturnValue, Frame, Status, false, false, 1>>;
type ClosedCallback = Arc<ThreadsafeFunction<(), UnknownReturnValue, (), Status, false, false, 1>>;

struct CaptureHandler {
  on_frame_arrived: FrameCallback,
  on_closed: Option<ClosedCallback>,
  scratch: Vec<u8>,
}

impl GraphicsCaptureApiHandler for CaptureHandler {
  type Flags = (FrameCallback, Option<ClosedCallback>);
  type Error = String;

  fn new(ctx: Context<Self::Flags>) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      on_frame_arrived: ctx.flags.0,
      on_closed: ctx.flags.1,
      scratch: Vec::new(),
    })
  }

  fn on_frame_arrived(
    &mut self,
    frame: &mut RustFrame,
    _capture_control: InternalCaptureControl,
  ) -> std::result::Result<(), Self::Error> {
    let width = frame.width();
    let height = frame.height();
    let timestamp = frame.timestamp().map_err(|e| e.to_string())?.Duration;
    let color_format = ColorFormat::from(frame.color_format());
    let dirty_regions = frame
      .dirty_regions()
      .map_err(|e| e.to_string())?
      .into_iter()
      .map(|region| DirtyRegion {
        x: region.x,
        y: region.y,
        width: region.width,
        height: region.height,
      })
      .collect();
    let buffer = frame.buffer().map_err(|e| e.to_string())?;
    let row_pitch = width * bytes_per_pixel(matches!(color_format, ColorFormat::Rgba16F));
    let bytes = buffer.as_nopadding_buffer(&mut self.scratch).to_vec();

    let status = self.on_frame_arrived.call(
      Frame {
        buffer: bytes,
        width,
        height,
        row_pitch,
        depth_pitch: row_pitch * height,
        timestamp,
        color_format,
        dirty_regions,
      },
      ThreadsafeFunctionCallMode::NonBlocking,
    );

    match status {
      Status::Ok | Status::QueueFull | Status::Closing => Ok(()),
      _ => Err(format!("Failed to dispatch frame callback: {status}")),
    }
  }

  fn on_closed(&mut self) -> std::result::Result<(), Self::Error> {
    if let Some(callback) = &self.on_closed {
      let status = callback.call((), ThreadsafeFunctionCallMode::NonBlocking);
      if !matches!(status, Status::Ok | Status::QueueFull | Status::Closing) {
        return Err(format!("Failed to dispatch closed callback: {status}"));
      }
    }
    Ok(())
  }
}

#[napi(object)]
pub struct CaptureOptions {
  pub monitor_index: Option<u32>,
  pub window_name: Option<String>,
  pub window_handle: Option<i64>,
  pub use_picker: Option<bool>,
  pub cursor_capture: Option<bool>,
  pub draw_border: Option<bool>,
  pub include_secondary_windows: Option<bool>,
  pub minimum_update_interval_ms: Option<u32>,
  pub dirty_regions: Option<bool>,
  pub color_format: Option<ColorFormat>,
}

fn cursor_setting(value: Option<bool>) -> CursorCaptureSettings {
  match value {
    Some(true) => CursorCaptureSettings::WithCursor,
    Some(false) => CursorCaptureSettings::WithoutCursor,
    None => CursorCaptureSettings::Default,
  }
}

fn border_setting(value: Option<bool>) -> DrawBorderSettings {
  match value {
    Some(true) => DrawBorderSettings::WithBorder,
    Some(false) => DrawBorderSettings::WithoutBorder,
    None => DrawBorderSettings::Default,
  }
}

fn secondary_setting(value: Option<bool>) -> SecondaryWindowSettings {
  match value {
    Some(true) => SecondaryWindowSettings::Include,
    Some(false) => SecondaryWindowSettings::Exclude,
    None => SecondaryWindowSettings::Default,
  }
}

fn interval_setting(value: Option<u32>) -> MinimumUpdateIntervalSettings {
  value.map_or(MinimumUpdateIntervalSettings::Default, |milliseconds| {
    MinimumUpdateIntervalSettings::Custom(Duration::from_millis(milliseconds.into()))
  })
}

fn dirty_setting(value: Option<bool>) -> DirtyRegionSettings {
  match value {
    Some(true) => DirtyRegionSettings::ReportAndRender,
    Some(false) => DirtyRegionSettings::ReportOnly,
    None => DirtyRegionSettings::Default,
  }
}

enum NativeCaptureControl {
  FreeThreaded(RustCaptureControl<CaptureHandler, String>),
  Picker(PickerCaptureControl),
}

impl NativeCaptureControl {
  fn is_finished(&self) -> bool {
    match self {
      Self::FreeThreaded(control) => control.is_finished(),
      Self::Picker(control) => control.is_finished(),
    }
  }

  fn stop(self) -> std::result::Result<(), String> {
    match self {
      Self::FreeThreaded(control) => control.stop().map_err(|error| error.to_string()),
      Self::Picker(control) => control.stop(),
    }
  }

  fn wait(self) -> std::result::Result<(), String> {
    match self {
      Self::FreeThreaded(control) => control.wait().map_err(|error| error.to_string()),
      Self::Picker(control) => control.wait(),
    }
  }
}

struct PickerCaptureControl {
  thread_handle: Option<JoinHandle<std::result::Result<(), String>>>,
  thread_id: u32,
}

impl PickerCaptureControl {
  fn is_finished(&self) -> bool {
    self
      .thread_handle
      .as_ref()
      .is_none_or(JoinHandle::is_finished)
  }

  fn stop(mut self) -> std::result::Result<(), String> {
    let post_result = if self.is_finished() {
      Ok(())
    } else {
      unsafe {
        PostThreadMessageW(
          self.thread_id,
          WM_QUIT,
          WPARAM::default(),
          LPARAM::default(),
        )
      }
      .map_err(|error| error.to_string())
    };
    let join_result = self.join();
    post_result.and(join_result)
  }

  fn wait(mut self) -> std::result::Result<(), String> {
    self.join()
  }

  fn join(&mut self) -> std::result::Result<(), String> {
    let thread_handle = self
      .thread_handle
      .take()
      .ok_or_else(|| "Capture thread handle has already been taken".to_owned())?;
    thread_handle
      .join()
      .map_err(|_| "Failed to join capture thread".to_owned())?
  }
}

fn start_picker_capture(
  cursor: CursorCaptureSettings,
  border: DrawBorderSettings,
  secondary: SecondaryWindowSettings,
  interval: MinimumUpdateIntervalSettings,
  dirty: DirtyRegionSettings,
  color: RustColorFormat,
  flags: <CaptureHandler as GraphicsCaptureApiHandler>::Flags,
) -> Result<NativeCaptureControl> {
  let (thread_id_sender, thread_id_receiver) = mpsc::sync_channel(1);
  let thread_handle = thread::spawn(move || {
    let mut message = MSG::default();
    unsafe {
      PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let thread_id = unsafe { GetCurrentThreadId() };
    thread_id_sender
      .send(thread_id)
      .map_err(|_| "Failed to initialize capture thread".to_owned())?;
    let item = GraphicsCapturePicker::pick_item()
      .map_err(|error| error.to_string())?
      .ok_or_else(|| "Capture picker was cancelled".to_owned())?;
    CaptureHandler::start(Settings::new(
      item, cursor, border, secondary, interval, dirty, color, flags,
    ))
    .map_err(|error| error.to_string())
  });
  let thread_id = thread_id_receiver
    .recv()
    .map_err(|_| error("Failed to initialize capture thread"))?;
  Ok(NativeCaptureControl::Picker(PickerCaptureControl {
    thread_handle: Some(thread_handle),
    thread_id,
  }))
}

enum CaptureControlAction {
  Stop,
  Wait,
}

pub struct CaptureControlTask {
  control: Option<NativeCaptureControl>,
  action: CaptureControlAction,
}

impl Task for CaptureControlTask {
  type Output = ();
  type JsValue = ();

  fn compute(&mut self) -> Result<Self::Output> {
    if let Some(control) = self.control.take() {
      match self.action {
        CaptureControlAction::Stop => control.stop(),
        CaptureControlAction::Wait => control.wait(),
      }
      .map_err(error)?;
    }
    Ok(())
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub struct CaptureControl {
  inner: Option<NativeCaptureControl>,
}

#[napi]
impl CaptureControl {
  #[napi(getter)]
  pub fn is_finished(&self) -> bool {
    self
      .inner
      .as_ref()
      .is_none_or(NativeCaptureControl::is_finished)
  }

  #[napi]
  pub fn stop(&mut self) -> AsyncTask<CaptureControlTask> {
    AsyncTask::new(CaptureControlTask {
      control: self.inner.take(),
      action: CaptureControlAction::Stop,
    })
  }

  #[napi]
  pub fn wait(&mut self) -> AsyncTask<CaptureControlTask> {
    AsyncTask::new(CaptureControlTask {
      control: self.inner.take(),
      action: CaptureControlAction::Wait,
    })
  }
}

#[napi]
pub struct ScreenCapture {
  options: CaptureOptions,
  on_frame_arrived: Option<FrameCallback>,
  on_closed: Option<ClosedCallback>,
}

#[napi]
impl ScreenCapture {
  #[napi(constructor)]
  pub fn new(
    options: CaptureOptions,
    #[napi(ts_arg_type = "(frame: Frame) => void")] on_frame_arrived: FrameCallback,
    #[napi(ts_arg_type = "(() => void) | undefined | null")] on_closed: Option<ClosedCallback>,
  ) -> Result<Self> {
    let target_count = [
      options.monitor_index.is_some(),
      options.window_name.is_some(),
      options.window_handle.is_some(),
      options.use_picker.unwrap_or(false),
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if target_count > 1 {
      return Err(error(
        "Specify only one of monitorIndex, windowName, windowHandle, or usePicker",
      ));
    }
    Ok(Self {
      options,
      on_frame_arrived: Some(on_frame_arrived),
      on_closed,
    })
  }

  #[napi]
  pub fn start(&mut self) -> Result<CaptureControl> {
    let flags = (
      self
        .on_frame_arrived
        .take()
        .ok_or_else(|| error("Capture session is already started"))?,
      self.on_closed.take(),
    );
    let cursor = cursor_setting(self.options.cursor_capture);
    let border = border_setting(self.options.draw_border);
    let secondary = secondary_setting(self.options.include_secondary_windows);
    let interval = interval_setting(self.options.minimum_update_interval_ms);
    let dirty = dirty_setting(self.options.dirty_regions);
    let color = self
      .options
      .color_format
      .clone()
      .unwrap_or(ColorFormat::Bgra8)
      .into();

    let inner = if self.options.use_picker.unwrap_or(false) {
      start_picker_capture(cursor, border, secondary, interval, dirty, color, flags)?
    } else if let Some(handle) = self.options.window_handle {
      let window = RustWindow::from_raw_hwnd(handle as isize as *mut std::ffi::c_void);
      NativeCaptureControl::FreeThreaded(
        CaptureHandler::start_free_threaded(Settings::new(
          window, cursor, border, secondary, interval, dirty, color, flags,
        ))
        .map_err(error)?,
      )
    } else if let Some(name) = &self.options.window_name {
      let window = RustWindow::from_contains_name(name).map_err(error)?;
      NativeCaptureControl::FreeThreaded(
        CaptureHandler::start_free_threaded(Settings::new(
          window, cursor, border, secondary, interval, dirty, color, flags,
        ))
        .map_err(error)?,
      )
    } else {
      let monitor =
        RustMonitor::from_index(self.options.monitor_index.unwrap_or(1) as usize).map_err(error)?;
      NativeCaptureControl::FreeThreaded(
        CaptureHandler::start_free_threaded(Settings::new(
          monitor, cursor, border, secondary, interval, dirty, color, flags,
        ))
        .map_err(error)?,
      )
    };

    Ok(CaptureControl { inner: Some(inner) })
  }
}
#[napi]
pub fn is_supported() -> Result<bool> {
  GraphicsCaptureApi::is_supported().map_err(error)
}

#[napi(object)]
pub struct CaptureApiSupport {
  pub graphics_capture: bool,
  pub cursor_settings: bool,
  pub border_settings: bool,
  pub secondary_windows: bool,
  pub minimum_update_interval: bool,
  pub dirty_regions: bool,
}

#[napi]
pub fn capture_api_support() -> Result<CaptureApiSupport> {
  Ok(CaptureApiSupport {
    graphics_capture: GraphicsCaptureApi::is_supported().map_err(error)?,
    cursor_settings: GraphicsCaptureApi::is_cursor_settings_supported().map_err(error)?,
    border_settings: GraphicsCaptureApi::is_border_settings_supported().map_err(error)?,
    secondary_windows: GraphicsCaptureApi::is_secondary_windows_supported().map_err(error)?,
    minimum_update_interval: GraphicsCaptureApi::is_minimum_update_interval_supported()
      .map_err(error)?,
    dirty_regions: GraphicsCaptureApi::is_dirty_region_supported().map_err(error)?,
  })
}

#[napi(object)]
pub struct MonitorInfo {
  pub index: u32,
  pub name: String,
  pub device_name: String,
  pub device_string: String,
  pub width: u32,
  pub height: u32,
  pub refresh_rate: u32,
  pub handle: i64,
}

fn monitor_info(monitor: RustMonitor) -> Result<MonitorInfo> {
  Ok(MonitorInfo {
    index: monitor.index().map_err(error)? as u32,
    name: monitor.name().map_err(error)?,
    device_name: monitor.device_name().map_err(error)?,
    device_string: monitor.device_string().map_err(error)?,
    width: monitor.width().map_err(error)?,
    height: monitor.height().map_err(error)?,
    refresh_rate: monitor.refresh_rate().map_err(error)?,
    handle: monitor.as_raw_hmonitor() as isize as i64,
  })
}

#[napi]
pub fn primary_monitor() -> Result<MonitorInfo> {
  monitor_info(RustMonitor::primary().map_err(error)?)
}

#[napi]
pub fn monitor_from_index(index: u32) -> Result<MonitorInfo> {
  monitor_info(RustMonitor::from_index(index as usize).map_err(error)?)
}

#[napi]
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
  RustMonitor::enumerate()
    .map_err(error)?
    .into_iter()
    .map(monitor_info)
    .collect()
}

#[napi(object)]
pub struct WindowInfo {
  pub title: String,
  pub process_id: u32,
  pub process_name: String,
  pub rect: Rect,
  pub title_bar_height: u32,
  pub width: i32,
  pub height: i32,
  pub is_valid: bool,
  pub handle: i64,
  pub monitor_index: Option<u32>,
}

fn window_info(window: RustWindow) -> Result<WindowInfo> {
  let rect = window.rect().map_err(error)?;
  Ok(WindowInfo {
    title: window.title().map_err(error)?,
    process_id: window.process_id().map_err(error)?,
    process_name: window.process_name().map_err(error)?,
    rect: Rect {
      left: rect.left,
      top: rect.top,
      right: rect.right,
      bottom: rect.bottom,
    },
    title_bar_height: window.title_bar_height().map_err(error)?,
    width: window.width().map_err(error)?,
    height: window.height().map_err(error)?,
    is_valid: window.is_valid(),
    handle: window.as_raw_hwnd() as isize as i64,
    monitor_index: window
      .monitor()
      .and_then(|monitor| monitor.index().ok())
      .map(|index| index as u32),
  })
}

#[napi]
pub fn foreground_window() -> Result<WindowInfo> {
  window_info(RustWindow::foreground().map_err(error)?)
}

#[napi]
pub fn window_from_name(title: String) -> Result<WindowInfo> {
  window_info(RustWindow::from_name(&title).map_err(error)?)
}

#[napi]
pub fn window_from_contains_name(title: String) -> Result<WindowInfo> {
  window_info(RustWindow::from_contains_name(&title).map_err(error)?)
}

#[napi]
pub fn window_from_handle(handle: i64) -> Result<WindowInfo> {
  window_info(RustWindow::from_raw_hwnd(
    handle as isize as *mut std::ffi::c_void,
  ))
}

#[napi]
pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
  RustWindow::enumerate()
    .map_err(error)?
    .into_iter()
    .map(window_info)
    .collect()
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum DxgiDuplicationFormat {
  Rgba16F,
  Rgb10A2,
  Rgb10XrA2,
  Rgba8,
  Rgba8Srgb,
  Bgra8,
  Bgra8Srgb,
}

impl From<DxgiDuplicationFormat> for RustDxgiFormat {
  fn from(value: DxgiDuplicationFormat) -> Self {
    match value {
      DxgiDuplicationFormat::Rgba16F => Self::Rgba16F,
      DxgiDuplicationFormat::Rgb10A2 => Self::Rgb10A2,
      DxgiDuplicationFormat::Rgb10XrA2 => Self::Rgb10XrA2,
      DxgiDuplicationFormat::Rgba8 => Self::Rgba8,
      DxgiDuplicationFormat::Rgba8Srgb => Self::Rgba8Srgb,
      DxgiDuplicationFormat::Bgra8 => Self::Bgra8,
      DxgiDuplicationFormat::Bgra8Srgb => Self::Bgra8Srgb,
    }
  }
}

impl From<RustDxgiFormat> for DxgiDuplicationFormat {
  fn from(value: RustDxgiFormat) -> Self {
    match value {
      RustDxgiFormat::Rgba16F => Self::Rgba16F,
      RustDxgiFormat::Rgb10A2 => Self::Rgb10A2,
      RustDxgiFormat::Rgb10XrA2 => Self::Rgb10XrA2,
      RustDxgiFormat::Rgba8 => Self::Rgba8,
      RustDxgiFormat::Rgba8Srgb => Self::Rgba8Srgb,
      RustDxgiFormat::Bgra8 => Self::Bgra8,
      RustDxgiFormat::Bgra8Srgb => Self::Bgra8Srgb,
    }
  }
}

fn convert_rgb10a2(buffer: &[u8], xr_bias: bool) -> Vec<u8> {
  let mut converted = Vec::with_capacity(buffer.len());
  for pixel in buffer.chunks_exact(4) {
    let packed = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
    let red = packed & 0x3ff;
    let green = (packed >> 10) & 0x3ff;
    let blue = (packed >> 20) & 0x3ff;
    let alpha = (packed >> 30) & 0x03;
    let channel = |value: u32| {
      let normalized = if xr_bias {
        (value as f32 - 384.0) / 510.0
      } else {
        value as f32 / 1023.0
      };
      (normalized.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    converted.extend_from_slice(&[
      channel(red),
      channel(green),
      channel(blue),
      ((alpha * 255 + 1) / 3) as u8,
    ]);
  }
  converted
}

fn qpc_timestamp_100ns(value: i64, frequency: i64) -> i64 {
  if value <= 0 || frequency <= 0 {
    return 0;
  }
  (i128::from(value) * 10_000_000 / i128::from(frequency)).min(i128::from(i64::MAX)) as i64
}

#[napi(object)]
pub struct DxgiSessionOptions {
  pub monitor_index: Option<u32>,
  pub supported_formats: Option<Vec<DxgiDuplicationFormat>>,
}

#[napi]
pub struct DxgiDuplicationSession {
  inner: Option<DxgiDuplicationApi>,
  monitor: RustMonitor,
  formats: Vec<RustDxgiFormat>,
  qpc_frequency: i64,
}

#[napi]
impl DxgiDuplicationSession {
  #[napi(constructor)]
  pub fn new(options: Option<DxgiSessionOptions>) -> Result<Self> {
    let options = options.unwrap_or(DxgiSessionOptions {
      monitor_index: None,
      supported_formats: None,
    });
    let monitor =
      RustMonitor::from_index(options.monitor_index.unwrap_or(1) as usize).map_err(error)?;
    let formats: Vec<RustDxgiFormat> = options
      .supported_formats
      .unwrap_or_default()
      .into_iter()
      .map(Into::into)
      .collect();
    let mut qpc_frequency = 0;
    unsafe { QueryPerformanceFrequency(&mut qpc_frequency) }.map_err(error)?;
    let inner = if formats.is_empty() {
      DxgiDuplicationApi::new(monitor).map_err(error)?
    } else {
      DxgiDuplicationApi::new_options(monitor, &formats).map_err(error)?
    };
    Ok(Self {
      inner: Some(inner),
      monitor,
      formats,
      qpc_frequency,
    })
  }

  #[napi(getter)]
  pub fn width(&self) -> u32 {
    self.inner.as_ref().map_or(0, DxgiDuplicationApi::width)
  }

  #[napi(getter)]
  pub fn height(&self) -> u32 {
    self.inner.as_ref().map_or(0, DxgiDuplicationApi::height)
  }

  #[napi(getter)]
  pub fn format(&self) -> Result<DxgiDuplicationFormat> {
    self
      .inner
      .as_ref()
      .map(|inner| inner.format().into())
      .ok_or_else(|| error("DXGI session is not active"))
  }

  #[napi(getter)]
  pub fn refresh_rate(&self) -> Result<Vec<u32>> {
    let (numerator, denominator) = self
      .inner
      .as_ref()
      .ok_or_else(|| error("DXGI session is not active"))?
      .refresh_rate();
    Ok(vec![numerator, denominator])
  }

  #[napi]
  pub fn acquire_next_frame(&mut self, timeout_ms: Option<u32>) -> Result<Option<Frame>> {
    let inner = self
      .inner
      .as_mut()
      .ok_or_else(|| error("DXGI session is not active"))?;
    let mut frame = match inner.acquire_next_frame(timeout_ms.unwrap_or(16)) {
      Ok(frame) => frame,
      Err(DxgiError::Timeout) => return Ok(None),
      Err(other) => return Err(error(other)),
    };
    let width = frame.width();
    let height = frame.height();
    let native_format = frame.format();
    let timestamp = qpc_timestamp_100ns(frame.frame_info().LastPresentTime, self.qpc_frequency);
    let buffer = frame.buffer().map_err(error)?;
    let mut scratch = Vec::new();
    let packed = buffer.as_nopadding_buffer(&mut scratch);
    let (bytes, color_format) = match native_format {
      RustDxgiFormat::Rgba16F => (packed.to_vec(), ColorFormat::Rgba16F),
      RustDxgiFormat::Rgb10A2 => (convert_rgb10a2(packed, false), ColorFormat::Rgba8),
      RustDxgiFormat::Rgb10XrA2 => (convert_rgb10a2(packed, true), ColorFormat::Rgba8),
      RustDxgiFormat::Rgba8 | RustDxgiFormat::Rgba8Srgb => (packed.to_vec(), ColorFormat::Rgba8),
      RustDxgiFormat::Bgra8 | RustDxgiFormat::Bgra8Srgb => (packed.to_vec(), ColorFormat::Bgra8),
    };
    let row_pitch = width * bytes_per_pixel(matches!(color_format, ColorFormat::Rgba16F));
    Ok(Some(Frame {
      buffer: bytes,
      width,
      height,
      row_pitch,
      depth_pitch: row_pitch * height,
      timestamp,
      color_format,
      dirty_regions: Vec::new(),
    }))
  }

  #[napi]
  pub fn recreate(&mut self) -> Result<()> {
    let recreated = if let Some(current) = self.inner.take() {
      if self.formats.is_empty() {
        current.recreate()
      } else {
        current.recreate_options(&self.formats)
      }
    } else if self.formats.is_empty() {
      DxgiDuplicationApi::new(self.monitor)
    } else {
      DxgiDuplicationApi::new_options(self.monitor, &self.formats)
    };
    self.inner = Some(recreated.map_err(error)?);
    Ok(())
  }

  #[napi]
  pub fn switch_monitor(&mut self, monitor_index: u32) -> Result<()> {
    let monitor = RustMonitor::from_index(monitor_index as usize).map_err(error)?;
    self.inner = Some(if self.formats.is_empty() {
      DxgiDuplicationApi::new(monitor).map_err(error)?
    } else {
      DxgiDuplicationApi::new_options(monitor, &self.formats).map_err(error)?
    });
    self.monitor = monitor;
    Ok(())
  }
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum ImageEncoderPixelFormat {
  Rgb16F,
  Bgra8,
  Rgba8,
}

impl From<ImageEncoderPixelFormat> for RustPixelFormat {
  fn from(value: ImageEncoderPixelFormat) -> Self {
    match value {
      ImageEncoderPixelFormat::Rgb16F => Self::Rgb16F,
      ImageEncoderPixelFormat::Bgra8 => Self::Bgra8,
      ImageEncoderPixelFormat::Rgba8 => Self::Rgba8,
    }
  }
}

#[napi]
pub struct ImageEncoder {
  inner: RustImageEncoder,
}

#[napi]
impl ImageEncoder {
  #[napi(constructor)]
  pub fn new(format: ImageFormat, pixel_format: ImageEncoderPixelFormat) -> Result<Self> {
    Ok(Self {
      inner: RustImageEncoder::new(format.into(), pixel_format.into()).map_err(error)?,
    })
  }

  #[napi]
  pub fn encode(&self, buffer: Buffer, width: u32, height: u32) -> Result<Buffer> {
    self
      .inner
      .encode(&buffer, width, height)
      .map(Buffer::from)
      .map_err(error)
  }
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum VideoCodec {
  Argb32,
  Bgra8,
  D16,
  H263,
  H264,
  H264Es,
  Hevc,
  HevcEs,
  Iyuv,
  L8,
  L16,
  Mjpg,
  Nv12,
  Mpeg1,
  Mpeg2,
  Rgb24,
  Rgb32,
  Wmv3,
  Wvc1,
  Vp9,
  Yuy2,
  Yv12,
}

impl From<VideoCodec> for RustVideoSubtype {
  fn from(value: VideoCodec) -> Self {
    match value {
      VideoCodec::Argb32 => Self::ARGB32,
      VideoCodec::Bgra8 => Self::BGRA8,
      VideoCodec::D16 => Self::D16,
      VideoCodec::H263 => Self::H263,
      VideoCodec::H264 => Self::H264,
      VideoCodec::H264Es => Self::H264ES,
      VideoCodec::Hevc => Self::HEVC,
      VideoCodec::HevcEs => Self::HEVCES,
      VideoCodec::Iyuv => Self::IYUV,
      VideoCodec::L8 => Self::L8,
      VideoCodec::L16 => Self::L16,
      VideoCodec::Mjpg => Self::MJPG,
      VideoCodec::Nv12 => Self::NV12,
      VideoCodec::Mpeg1 => Self::MPEG1,
      VideoCodec::Mpeg2 => Self::MPEG2,
      VideoCodec::Rgb24 => Self::RGB24,
      VideoCodec::Rgb32 => Self::RGB32,
      VideoCodec::Wmv3 => Self::WMV3,
      VideoCodec::Wvc1 => Self::WVC1,
      VideoCodec::Vp9 => Self::VP9,
      VideoCodec::Yuy2 => Self::YUY2,
      VideoCodec::Yv12 => Self::YV12,
    }
  }
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum AudioCodec {
  Aac,
  Ac3,
  AacAdts,
  AacHdcp,
  Ac3Spdif,
  Ac3Hdcp,
  Adts,
  Alac,
  AmrNb,
  AwrWb,
  Dts,
  Eac3,
  Flac,
  Float,
  Mp3,
  Mpeg,
  Opus,
  Pcm,
  Wma8,
  Wma9,
  Vorbis,
}

impl From<AudioCodec> for RustAudioSubtype {
  fn from(value: AudioCodec) -> Self {
    match value {
      AudioCodec::Aac => Self::AAC,
      AudioCodec::Ac3 => Self::AC3,
      AudioCodec::AacAdts => Self::AACADTS,
      AudioCodec::AacHdcp => Self::AACHDCP,
      AudioCodec::Ac3Spdif => Self::AC3SPDIF,
      AudioCodec::Ac3Hdcp => Self::AC3HDCP,
      AudioCodec::Adts => Self::ADTS,
      AudioCodec::Alac => Self::ALAC,
      AudioCodec::AmrNb => Self::AMRNB,
      AudioCodec::AwrWb => Self::AWRWB,
      AudioCodec::Dts => Self::DTS,
      AudioCodec::Eac3 => Self::EAC3,
      AudioCodec::Flac => Self::FLAC,
      AudioCodec::Float => Self::Float,
      AudioCodec::Mp3 => Self::MP3,
      AudioCodec::Mpeg => Self::MPEG,
      AudioCodec::Opus => Self::OPUS,
      AudioCodec::Pcm => Self::PCM,
      AudioCodec::Wma8 => Self::WMA8,
      AudioCodec::Wma9 => Self::WMA9,
      AudioCodec::Vorbis => Self::Vorbis,
    }
  }
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum ContainerFormat {
  Asf,
  Mp3,
  Mpeg4,
  Avi,
  Mpeg2,
  Wave,
  AacAdts,
  Adts,
  ThreeGp,
  Amr,
  Flac,
}

impl From<ContainerFormat> for RustContainerSubtype {
  fn from(value: ContainerFormat) -> Self {
    match value {
      ContainerFormat::Asf => Self::ASF,
      ContainerFormat::Mp3 => Self::MP3,
      ContainerFormat::Mpeg4 => Self::MPEG4,
      ContainerFormat::Avi => Self::AVI,
      ContainerFormat::Mpeg2 => Self::MPEG2,
      ContainerFormat::Wave => Self::WAVE,
      ContainerFormat::AacAdts => Self::AACADTS,
      ContainerFormat::Adts => Self::ADTS,
      ContainerFormat::ThreeGp => Self::GP3,
      ContainerFormat::Amr => Self::AMR,
      ContainerFormat::Flac => Self::FLAC,
    }
  }
}

#[napi(object)]
pub struct VideoSettings {
  pub width: u32,
  pub height: u32,
  pub codec: Option<VideoCodec>,
  pub bitrate: Option<u32>,
  pub frame_rate: Option<u32>,
  pub pixel_aspect_ratio_numerator: Option<u32>,
  pub pixel_aspect_ratio_denominator: Option<u32>,
  pub disabled: Option<bool>,
}

#[napi(object)]
pub struct AudioSettings {
  pub codec: Option<AudioCodec>,
  pub bitrate: Option<u32>,
  pub channel_count: Option<u32>,
  pub sample_rate: Option<u32>,
  pub bits_per_sample: Option<u32>,
  pub disabled: Option<bool>,
}

#[napi(object)]
pub struct VideoEncoderOptions {
  pub path: Option<String>,
  pub video: VideoSettings,
  pub audio: Option<AudioSettings>,
  pub container: Option<ContainerFormat>,
}

enum VideoEncoderOutput {
  File,
  Memory(InMemoryRandomAccessStream),
}

#[napi]
pub struct VideoEncoder {
  inner: Option<RustVideoEncoder>,
  output: VideoEncoderOutput,
  width: u32,
  height: u32,
}

#[napi]
impl VideoEncoder {
  #[napi(constructor)]
  pub fn new(options: VideoEncoderOptions) -> Result<Self> {
    let width = options.video.width;
    let height = options.video.height;
    let mut video = VideoSettingsBuilder::new(options.video.width, options.video.height);
    if let Some(codec) = options.video.codec {
      video = video.sub_type(codec.into());
    }
    if let Some(bitrate) = options.video.bitrate {
      video = video.bitrate(bitrate);
    }
    if let Some(frame_rate) = options.video.frame_rate {
      video = video.frame_rate(frame_rate);
    }
    if options.video.pixel_aspect_ratio_numerator.is_some()
      || options.video.pixel_aspect_ratio_denominator.is_some()
    {
      video = video.pixel_aspect_ratio((
        options.video.pixel_aspect_ratio_numerator.unwrap_or(1),
        options.video.pixel_aspect_ratio_denominator.unwrap_or(1),
      ));
    }
    if let Some(disabled) = options.video.disabled {
      video = video.disabled(disabled);
    }

    let mut audio = AudioSettingsBuilder::new();
    if let Some(settings) = options.audio {
      if let Some(codec) = settings.codec {
        audio = audio.sub_type(codec.into());
      }
      if let Some(bitrate) = settings.bitrate {
        audio = audio.bitrate(bitrate);
      }
      if let Some(channel_count) = settings.channel_count {
        audio = audio.channel_count(channel_count);
      }
      if let Some(sample_rate) = settings.sample_rate {
        audio = audio.sample_rate(sample_rate);
      }
      if let Some(bits_per_sample) = settings.bits_per_sample {
        audio = audio.bit_per_sample(bits_per_sample);
      }
      if let Some(disabled) = settings.disabled {
        audio = audio.disabled(disabled);
      }
    } else {
      audio = audio.disabled(true);
    }

    let mut container = ContainerSettingsBuilder::new();
    if let Some(format) = options.container {
      container = container.sub_type(format.into());
    }

    let (inner, output) = if let Some(path) = options.path {
      (
        RustVideoEncoder::new(video, audio, container, path).map_err(error)?,
        VideoEncoderOutput::File,
      )
    } else {
      let stream = InMemoryRandomAccessStream::new().map_err(error)?;
      let encoder =
        RustVideoEncoder::new_from_stream(video, audio, container, stream.cast().map_err(error)?)
          .map_err(error)?;
      (encoder, VideoEncoderOutput::Memory(stream))
    };

    Ok(Self {
      inner: Some(inner),
      output,
      width,
      height,
    })
  }

  fn expected_buffer_len(&self) -> Result<usize> {
    usize::try_from(self.width)
      .ok()
      .and_then(|width| width.checked_mul(4))
      .and_then(|row_bytes| {
        usize::try_from(self.height)
          .ok()
          .and_then(|height| row_bytes.checked_mul(height))
      })
      .ok_or_else(|| error("Video dimensions exceed the supported buffer size"))
  }

  #[napi]
  pub fn send_frame(&mut self, frame: &Frame) -> Result<()> {
    let expected = self.expected_buffer_len()?;
    if frame.width != self.width || frame.height != self.height {
      return Err(error(format!(
        "Frame dimensions are {}x{}; encoder expects {}x{}",
        frame.width, frame.height, self.width, self.height
      )));
    }
    if frame.buffer.len() != expected {
      return Err(error(format!(
        "Frame buffer has {} bytes; expected {expected}",
        frame.buffer.len()
      )));
    }
    if matches!(frame.color_format, ColorFormat::Rgba16F) {
      return Err(error("Video encoding does not accept Rgba16F frames"));
    }

    let row_bytes = self.width as usize * 4;
    let mut bottom_up_bgra = vec![0_u8; expected];
    for output_y in 0..self.height as usize {
      let source_y = self.height as usize - output_y - 1;
      let source = &frame.buffer[source_y * row_bytes..(source_y + 1) * row_bytes];
      let destination = &mut bottom_up_bgra[output_y * row_bytes..(output_y + 1) * row_bytes];
      if matches!(frame.color_format, ColorFormat::Bgra8) {
        destination.copy_from_slice(source);
      } else {
        for (source, destination) in source.chunks_exact(4).zip(destination.chunks_exact_mut(4)) {
          destination.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
        }
      }
    }

    self
      .inner
      .as_mut()
      .ok_or_else(|| error("Video encoder is already finished"))?
      .send_frame_buffer(&bottom_up_bgra, frame.timestamp)
      .map_err(error)
  }

  #[napi]
  pub fn send_frame_with_audio(&mut self, frame: &Frame, audio_buffer: Buffer) -> Result<()> {
    self.send_frame(frame)?;
    self.send_audio_buffer(audio_buffer, Some(frame.timestamp))
  }

  #[napi]
  pub fn send_frame_buffer(&mut self, buffer: Buffer, timestamp: i64) -> Result<()> {
    let expected = self.expected_buffer_len()?;
    if buffer.len() != expected {
      return Err(error(format!(
        "Video frame buffer has {} bytes; expected {expected}",
        buffer.len()
      )));
    }
    self
      .inner
      .as_mut()
      .ok_or_else(|| error("Video encoder is already finished"))?
      .send_frame_buffer(&buffer, timestamp)
      .map_err(error)
  }

  #[napi]
  pub fn send_audio_buffer(&mut self, buffer: Buffer, timestamp: Option<i64>) -> Result<()> {
    self
      .inner
      .as_mut()
      .ok_or_else(|| error("Video encoder is already finished"))?
      .send_audio_buffer(&buffer, timestamp.unwrap_or(0))
      .map_err(error)
  }

  #[napi]
  pub fn finish(&mut self) -> Result<Option<Buffer>> {
    if let Some(encoder) = self.inner.take() {
      encoder.finish().map_err(error)?;
    }

    match &self.output {
      VideoEncoderOutput::File => Ok(None),
      VideoEncoderOutput::Memory(stream) => {
        let size = stream.Size().map_err(error)?;
        let size_u32 = u32::try_from(size).map_err(|_| error("Encoded stream exceeds 4 GiB"))?;
        let input = stream.GetInputStreamAt(0).map_err(error)?;
        let reader = DataReader::CreateDataReader(&input).map_err(error)?;
        reader
          .LoadAsync(size_u32)
          .map_err(error)?
          .join()
          .map_err(error)?;
        let mut bytes = vec![0; size as usize];
        reader.ReadBytes(&mut bytes).map_err(error)?;
        Ok(Some(bytes.into()))
      }
    }
  }
}
