use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::common::{bytes_per_pixel, crop_buffer, error, frame_pitches};
use image::{DynamicImage, ImageBuffer, Rgba};
use napi::Status;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
  ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue,
};
use napi_derive::napi;
use screencapturekit::cm::{CMSampleBuffer, CMSampleBufferExt, CMSampleBufferSCExt, SCFrameStatus};
use screencapturekit::content_sharing_picker::{
  SCContentSharingPicker, SCContentSharingPickerConfiguration, SCContentSharingPickerMode,
  SCPickerOutcome,
};
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;

fn unsupported(feature: &str) -> Error {
  error(format!("{feature} is unavailable on macOS"))
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum ColorFormat {
  Rgba16F,
  Rgba8,
  Bgra8,
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
    self.color_format
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
    let bpp = bytes_per_pixel(matches!(self.color_format, ColorFormat::Rgba16F));
    let (row_pitch, depth_pitch) = frame_pitches(width, height, bpp)?;

    Ok(Self {
      buffer,
      width,
      height,
      row_pitch,
      depth_pitch,
      timestamp: self.timestamp,
      color_format: self.color_format,
      dirty_regions: Vec::new(),
    })
  }

  #[napi]
  pub fn encode(&self, format: ImageFormat) -> Result<Buffer> {
    encode_image(
      &self.buffer,
      self.width,
      self.height,
      self.color_format,
      format,
    )
    .map(Buffer::from)
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

fn encode_image(
  buffer: &[u8],
  width: u32,
  height: u32,
  pixel_format: ColorFormat,
  image_format: ImageFormat,
) -> Result<Vec<u8>> {
  if matches!(image_format, ImageFormat::JpegXr) {
    return Err(unsupported("JPEG XR image encoding"));
  }
  if matches!(pixel_format, ColorFormat::Rgba16F) {
    return Err(error("Rgba16F cannot be encoded as an image"));
  }
  const MAX_DIM: u32 = 16384;
  if width > MAX_DIM || height > MAX_DIM {
    return Err(error(format!(
      "Image dimensions ({width}×{height}) exceed maximum ({MAX_DIM}×{MAX_DIM})"
    )));
  }
  let expected: usize = (width as usize)
    .checked_mul(height as usize)
    .and_then(|pixels| pixels.checked_mul(4))
    .ok_or_else(|| error("Arithmetic overflow computing packed image buffer size"))?;
  if buffer.len() != expected {
    return Err(error(format!(
      "Image buffer has {} bytes; expected {expected}",
      buffer.len()
    )));
  }

  let mut rgba = buffer.to_vec();
  if matches!(pixel_format, ColorFormat::Bgra8) {
    for pixel in rgba.chunks_exact_mut(4) {
      pixel.swap(0, 2);
    }
  }
  let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba)
    .ok_or_else(|| error("Image dimensions do not match the buffer"))?;
  let output_format = match image_format {
    ImageFormat::Jpeg => image::ImageFormat::Jpeg,
    ImageFormat::Png => image::ImageFormat::Png,
    ImageFormat::Gif => image::ImageFormat::Gif,
    ImageFormat::Tiff => image::ImageFormat::Tiff,
    ImageFormat::Bmp => image::ImageFormat::Bmp,
    ImageFormat::JpegXr => unreachable!(),
  };
  let mut output = Cursor::new(Vec::new());
  DynamicImage::ImageRgba8(image)
    .write_to(&mut output, output_format)
    .map_err(error)?;
  Ok(output.into_inner())
}

#[derive(Clone)]
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

type FrameCallback =
  Arc<ThreadsafeFunction<Frame, UnknownReturnValue, Frame, Status, false, false, 1>>;
type ClosedCallback = Arc<
  ThreadsafeFunction<Option<String>, UnknownReturnValue, Option<String>, Status, false, false, 1>,
>;
type FrameSink = Arc<dyn Fn(Frame) + Send + Sync>;
type ClosedSink = Arc<dyn Fn(Option<String>) + Send + Sync>;

/// Holds a resolved capture target together with the `SCShareableContent`
/// that backs display / window references inside the filter.  The owner
/// must stay alive for the full lifetime of `SCStream` so that internal
/// references to the content object remain valid.
struct CaptureFilter {
  filter: SCContentFilter,
  width: u32,
  height: u32,
  #[allow(dead_code)]
  /// Picker-created filters manage their own content; monitor/window paths
  /// must retain the `SCShareableContent` fetched just before selection.
  content: Option<SCShareableContent>,
}

struct MacosCaptureControl {
  stop: Arc<AtomicBool>,
  thread: Option<JoinHandle<std::result::Result<(), String>>>,
}

impl MacosCaptureControl {
  fn is_finished(&self) -> bool {
    self.thread.as_ref().is_none_or(JoinHandle::is_finished)
  }

  fn stop(mut self) -> std::result::Result<(), String> {
    self.stop.store(true, Ordering::Release);
    self.join()
  }

  fn wait(mut self) -> std::result::Result<(), String> {
    self.join()
  }

  fn join(&mut self) -> std::result::Result<(), String> {
    self
      .thread
      .take()
      .ok_or_else(|| "Capture thread handle has already been taken".to_owned())?
      .join()
      .map_err(|_| "macOS capture thread panicked".to_owned())?
  }
}

impl Drop for MacosCaptureControl {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Release);
  }
}

enum CaptureControlAction {
  Stop,
  Wait,
}

pub struct CaptureControlTask {
  control: Option<MacosCaptureControl>,
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
  inner: Option<MacosCaptureControl>,
}

#[napi]
impl CaptureControl {
  #[napi(getter)]
  pub fn is_finished(&self) -> bool {
    self
      .inner
      .as_ref()
      .is_none_or(MacosCaptureControl::is_finished)
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
    #[napi(ts_arg_type = "((error: string | null) => void) | undefined | null")] on_closed: Option<
      ClosedCallback,
    >,
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
    if matches!(options.color_format, Some(ColorFormat::Rgba16F)) {
      return Err(error(
        "Rgba16F capture is not supported by the macOS backend",
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
    let callback = self
      .on_frame_arrived
      .take()
      .ok_or_else(|| error("Capture session is already started"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let pending = Arc::new(AtomicBool::new(false));
    let failures = Arc::new(AtomicUsize::new(0));

    let on_frame: FrameSink = {
      let frame_stop = Arc::clone(&stop);
      let frame_pending = Arc::clone(&pending);
      let frame_failures = Arc::clone(&failures);
      Arc::new(move |frame| {
        if frame_stop.load(Ordering::Acquire) {
          return;
        }
        if frame_pending
          .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
          .is_err()
        {
          return;
        }
        let cb_pending = Arc::clone(&frame_pending);
        let cb_failures = Arc::clone(&frame_failures);
        let status = callback.call_with_return_value(
          frame,
          ThreadsafeFunctionCallMode::NonBlocking,
          move |_result, _env| {
            cb_pending.store(false, Ordering::Release);
            cb_failures.store(0, Ordering::Release);
            Ok(())
          },
        );
        match status {
          Status::Ok => {}
          Status::QueueFull => {
            frame_pending.store(false, Ordering::Release);
            let count = frame_failures.fetch_add(1, Ordering::AcqRel) + 1;
            if count >= 10 {
              frame_stop.store(true, Ordering::Release);
            }
          }
          _ => {
            frame_pending.store(false, Ordering::Release);
            frame_stop.store(true, Ordering::Release);
          }
        }
      })
    };

    let on_closed: ClosedSink = if let Some(callback) = self.on_closed.take() {
      Arc::new(move |error: Option<String>| {
        let status = callback.call(error, ThreadsafeFunctionCallMode::Blocking);
        match status {
          Status::Ok | Status::Closing => {}
          _ => {}
        }
      })
    } else {
      Arc::new(|_| {})
    };

    let thread_stop = Arc::clone(&stop);
    let options = self.options.clone();
    let thread = thread::Builder::new()
      .name("screen-capture-macos".to_owned())
      .spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
          run_macos_capture(options, thread_stop, on_frame)
        }));
        match result {
          Ok(Ok(())) => {
            on_closed(None);
            Ok(())
          }
          Ok(Err(e)) => {
            on_closed(Some(e.clone()));
            Err(e)
          }
          Err(panic) => {
            let msg = panic_message(panic);
            on_closed(Some(msg.clone()));
            Err(msg)
          }
        }
      })
      .map_err(error)?;

    Ok(CaptureControl {
      inner: Some(MacosCaptureControl {
        stop,
        thread: Some(thread),
      }),
    })
  }
}

struct FrameHandler {
  on_frame: FrameSink,
  requested_format: ColorFormat,
  timestamp_origin: Mutex<Option<i128>>,
}

/// Returns true when the frame status indicates the buffer may carry displayable
/// content.  Unknown statuses (None) are treated optimistically so the image
/// buffer can be validated downstream.
const fn should_process_frame(status: Option<SCFrameStatus>) -> bool {
  match status {
    None | Some(SCFrameStatus::Complete) | Some(SCFrameStatus::Started) => true,
    Some(SCFrameStatus::Idle)
    | Some(SCFrameStatus::Blank)
    | Some(SCFrameStatus::Suspended)
    | Some(SCFrameStatus::Stopped) => false,
  }
}

impl SCStreamOutputTrait for FrameHandler {
  fn did_output_sample_buffer(&self, sample: CMSampleBuffer, output_type: SCStreamOutputType) {
    if !matches!(output_type, SCStreamOutputType::Screen) {
      return;
    }
    if !should_process_frame(sample.frame_status()) {
      return;
    }
    let presentation_time = sample.output_presentation_timestamp();
    if !presentation_time.is_valid()
      || presentation_time.is_indefinite()
      || presentation_time.timescale <= 0
    {
      return;
    }
    let absolute_timestamp =
      i128::from(presentation_time.value) * 10_000_000 / i128::from(presentation_time.timescale);
    let Ok(mut timestamp_origin) = self.timestamp_origin.lock() else {
      return;
    };
    let origin = *timestamp_origin.get_or_insert(absolute_timestamp);
    let timestamp = absolute_timestamp
      .saturating_sub(origin)
      .clamp(0, i128::from(i64::MAX)) as i64;
    drop(timestamp_origin);

    let Some(pixel_buffer) = sample.image_buffer() else {
      return;
    };
    let Ok(guard) = pixel_buffer.lock(CVPixelBufferLockFlags::READ_ONLY) else {
      return;
    };
    let width = guard.width();
    let height = guard.height();
    let source_stride = guard.bytes_per_row();
    let row_bytes = width.saturating_mul(4);
    let source = guard.as_slice();
    if width == 0
      || height == 0
      || source_stride < row_bytes
      || source.len() < source_stride.saturating_mul(height)
    {
      return;
    }

    let mut pixels = vec![0_u8; row_bytes.saturating_mul(height)];
    for y in 0..height {
      let source_start = y * source_stride;
      let destination_start = y * row_bytes;
      pixels[destination_start..destination_start + row_bytes]
        .copy_from_slice(&source[source_start..source_start + row_bytes]);
    }
    if matches!(self.requested_format, ColorFormat::Rgba8) {
      for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
      }
    }

    let Ok(w) = u32::try_from(width) else {
      return;
    };
    let Ok(h) = u32::try_from(height) else {
      return;
    };
    let bpp = bytes_per_pixel(false);
    let Ok((row_pitch, depth_pitch)) = frame_pitches(w, h, bpp) else {
      return;
    };
    (self.on_frame)(Frame {
      buffer: pixels,
      width: w,
      height: h,
      row_pitch,
      depth_pitch,
      timestamp,
      color_format: self.requested_format,
      dirty_regions: Vec::new(),
    });
  }
}

#[cfg(target_os = "macos")]
fn macos_major_version() -> std::result::Result<u32, String> {
  unsafe extern "C" {
    fn sysctlbyname(
      name: *const std::ffi::c_char,
      oldp: *mut std::ffi::c_void,
      oldlenp: *mut usize,
      newp: *const std::ffi::c_void,
      newlen: usize,
    ) -> i32;
  }

  let name = c"kern.osproductversion";
  let mut length = 0;
  if unsafe {
    sysctlbyname(
      name.as_ptr(),
      std::ptr::null_mut(),
      &mut length,
      std::ptr::null(),
      0,
    )
  } != 0
    || length == 0
  {
    return Err("Unable to determine the macOS version".to_owned());
  }

  let mut value = vec![0_u8; length];
  if unsafe {
    sysctlbyname(
      name.as_ptr(),
      value.as_mut_ptr().cast(),
      &mut length,
      std::ptr::null(),
      0,
    )
  } != 0
  {
    return Err("Unable to determine the macOS version".to_owned());
  }
  let version = std::str::from_utf8(value.get(..length.saturating_sub(1)).unwrap_or_default())
    .map_err(|_| "macOS returned an invalid version string")?;
  version
    .split('.')
    .next()
    .and_then(|major| major.parse().ok())
    .ok_or_else(|| format!("Unable to parse macOS version \"{version}\""))
}

#[cfg(not(target_os = "macos"))]
fn macos_major_version() -> std::result::Result<u32, String> {
  Err("macOS version detection is unavailable on this platform".to_owned())
}

fn picker_filter(stop: &Arc<AtomicBool>) -> std::result::Result<CaptureFilter, String> {
  if macos_major_version()? < 14 {
    return Err("The native content picker requires macOS 14 or newer".to_owned());
  }

  let mut config = SCContentSharingPickerConfiguration::new();
  config.set_allowed_picker_modes(&[
    SCContentSharingPickerMode::SingleWindow,
    SCContentSharingPickerMode::SingleDisplay,
  ]);
  let (sender, receiver) = std::sync::mpsc::sync_channel(1);
  SCContentSharingPicker::show(&config, move |outcome| {
    let _ = sender.send(outcome);
  });
  let outcome = loop {
    match receiver.recv_timeout(Duration::from_millis(20)) {
      Ok(outcome) => break outcome,
      Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
        if stop.load(Ordering::Acquire) {
          SCContentSharingPicker::set_active(false);
          return Err("Screen capture stopped before content selection".to_owned());
        }
      }
      Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
        return Err("The macOS content picker closed without a result".to_owned());
      }
    }
  };
  SCContentSharingPicker::set_active(false);
  match outcome {
    SCPickerOutcome::Picked(result) => {
      let (width, height) = result.pixel_size();
      Ok(CaptureFilter {
        filter: result.filter(),
        width,
        height,
        content: None,
      })
    }
    SCPickerOutcome::Cancelled => Err("Screen capture selection was cancelled".to_owned()),
    SCPickerOutcome::Error(message) => Err(message),
  }
}

fn window_capture_filter(window: &SCWindow) -> (SCContentFilter, u32, u32) {
  let filter = SCContentFilter::create().with_window(window).build();
  let frame = window.frame();
  let scale = f64::from(filter.point_pixel_scale()).max(1.0);
  (
    filter,
    dimension(frame.size.width * scale).max(1),
    dimension(frame.size.height * scale).max(1),
  )
}

fn capture_filter(
  options: &CaptureOptions,
  stop: &Arc<AtomicBool>,
) -> std::result::Result<CaptureFilter, String> {
  if options.use_picker.unwrap_or(false) {
    return picker_filter(stop);
  }

  let content = SCShareableContent::get().map_err(|error| error.to_string())?;

  // Fetch display / window vectors once to avoid redundant retrieval
  // across the target selection branches below.
  let mut displays = content.displays();
  let windows = content.windows();

  if let Some(handle) = options.window_handle {
    let window_id =
      u32::try_from(handle).map_err(|_| "windowHandle must be a valid macOS window ID")?;
    let window = windows
      .iter()
      .find(|window| window.window_id() == window_id)
      .ok_or_else(|| format!("Window {window_id} was not found"))?;
    let (filter, width, height) = window_capture_filter(window);
    return Ok(CaptureFilter {
      filter,
      width,
      height,
      content: Some(content),
    });
  }
  if let Some(title) = options.window_name.as_deref() {
    let window = windows
      .iter()
      .find(|window| {
        window
          .title()
          .is_some_and(|window_title| window_title.contains(title))
      })
      .ok_or_else(|| format!("No window title contains \"{title}\""))?;
    let (filter, width, height) = window_capture_filter(window);
    return Ok(CaptureFilter {
      filter,
      width,
      height,
      content: Some(content),
    });
  }

  let index = options.monitor_index.unwrap_or(1);
  if index == 0 {
    return Err("monitorIndex is one-based".to_owned());
  }
  // Reorder so the primary display (origin 0,0) comes first.
  if let Some(primary_index) = displays.iter().position(|display| {
    let origin = display.frame().origin;
    origin.x.abs() < f64::EPSILON && origin.y.abs() < f64::EPSILON
  }) {
    displays.swap(0, primary_index);
  }
  let display = displays
    .get(index as usize - 1)
    .ok_or_else(|| format!("Monitor index {index} was not found"))?;
  Ok(CaptureFilter {
    filter: SCContentFilter::create()
      .with_display(display)
      .with_excluding_windows(&[])
      .build(),
    width: display.width(),
    height: display.height(),
    content: Some(content),
  })
}

fn ordered_displays(content: &SCShareableContent) -> Vec<SCDisplay> {
  let mut displays = content.displays();
  if let Some(primary_index) = displays.iter().position(|display| {
    let origin = display.frame().origin;
    origin.x.abs() < f64::EPSILON && origin.y.abs() < f64::EPSILON
  }) {
    displays.swap(0, primary_index);
  }
  displays
}

fn run_macos_capture(
  options: CaptureOptions,
  stop: Arc<AtomicBool>,
  on_frame: FrameSink,
) -> std::result::Result<(), String> {
  let capture = capture_filter(&options, &stop)?;
  let filter = &capture.filter;
  let width = capture.width;
  let height = capture.height;

  let config = SCStreamConfiguration::new()
    .with_width(width)
    .with_height(height)
    .with_pixel_format(PixelFormat::BGRA)
    .with_shows_cursor(options.cursor_capture.unwrap_or(true));

  let handler = FrameHandler {
    on_frame,
    requested_format: options.color_format.unwrap_or(ColorFormat::Bgra8),
    timestamp_origin: Mutex::new(None),
  };
  let stream_error = Arc::new(Mutex::new(None));
  let delegate_stop = Arc::clone(&stop);
  let delegate_error = Arc::clone(&stream_error);
  let delegate = ErrorHandler::new(move |error| {
    if let Ok(mut captured_error) = delegate_error.lock() {
      *captured_error = Some(error.to_string());
    }
    delegate_stop.store(true, Ordering::Release);
  });
  let mut stream = SCStream::new_with_delegate(filter, &config, delegate);
  stream
    .add_output_handler(handler, SCStreamOutputType::Screen)
    .ok_or_else(|| "Failed to register the macOS screen frame handler".to_owned())?;

  stream.start_capture().map_err(|error| error.to_string())?;

  while !stop.load(Ordering::Acquire) {
    thread::sleep(Duration::from_millis(20));
  }
  if let Some(error) = stream_error
    .lock()
    .map_err(|_| "macOS stream error state was poisoned".to_owned())?
    .take()
  {
    return Err(error);
  }
  stream.stop_capture().map_err(|error| error.to_string())?;
  // `capture` is dropped here, releasing the retained SCShareableContent
  // (if any) after the stream has been fully stopped.
  drop(capture);
  Ok(())
}

#[napi]
pub fn is_supported() -> bool {
  SCShareableContent::get().is_ok()
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
pub fn capture_api_support() -> CaptureApiSupport {
  let graphics_capture = is_supported();
  CaptureApiSupport {
    graphics_capture,
    cursor_settings: graphics_capture,
    border_settings: false,
    secondary_windows: false,
    minimum_update_interval: false,
    dirty_regions: false,
  }
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

fn monitor_info(index: usize, display: &SCDisplay) -> MonitorInfo {
  let id = display.display_id();
  MonitorInfo {
    index: u32::try_from(index + 1).unwrap_or(u32::MAX),
    name: format!("Display {id}"),
    device_name: format!("Display {id}"),
    device_string: "macOS ScreenCaptureKit display".to_owned(),
    width: display.width(),
    height: display.height(),
    refresh_rate: 0,
    handle: i64::from(id),
  }
}

#[napi]
pub fn primary_monitor() -> Result<MonitorInfo> {
  monitor_from_index(1)
}

#[napi]
pub fn monitor_from_index(index: u32) -> Result<MonitorInfo> {
  if index == 0 {
    return Err(error("monitorIndex is one-based"));
  }
  let content = SCShareableContent::get().map_err(error)?;
  ordered_displays(&content)
    .get(index as usize - 1)
    .map(|display| monitor_info(index as usize - 1, display))
    .ok_or_else(|| error(format!("Monitor index {index} was not found")))
}

#[napi]
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
  let content = SCShareableContent::get().map_err(error)?;
  Ok(
    ordered_displays(&content)
      .into_iter()
      .enumerate()
      .map(|(index, display)| monitor_info(index, &display))
      .collect(),
  )
}

#[napi(object)]
pub struct WindowInfo {
  pub title: String,
  pub process_id: u32,
  pub process_name: String,
  pub rect: Rect,
  pub title_bar_height: i32,
  pub width: u32,
  pub height: u32,
  pub is_valid: bool,
  pub handle: i64,
  pub monitor_index: Option<u32>,
}

fn coordinate(value: f64) -> i32 {
  value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

fn dimension(value: f64) -> u32 {
  value.round().clamp(0.0, u32::MAX as f64) as u32
}

fn window_info(window: &SCWindow) -> WindowInfo {
  let frame = window.frame();
  let process_id = window
    .owning_application()
    .and_then(|application| u32::try_from(application.process_id()).ok())
    .unwrap_or(0);
  let process_name = window
    .owning_application()
    .map(|application| application.application_name())
    .unwrap_or_default();
  let left = coordinate(frame.origin.x);
  let top = coordinate(frame.origin.y);
  let width = dimension(frame.size.width);
  let height = dimension(frame.size.height);
  WindowInfo {
    title: window.title().unwrap_or_default(),
    process_id,
    process_name,
    rect: Rect {
      left,
      top,
      right: left.saturating_add(i32::try_from(width).unwrap_or(i32::MAX)),
      bottom: top.saturating_add(i32::try_from(height).unwrap_or(i32::MAX)),
    },
    title_bar_height: 0,
    width,
    height,
    is_valid: window.is_on_screen(),
    handle: i64::from(window.window_id()),
    monitor_index: None,
  }
}

fn find_window(predicate: impl Fn(&SCWindow) -> bool) -> Result<WindowInfo> {
  let content = SCShareableContent::get().map_err(error)?;
  content
    .windows()
    .iter()
    .find(|window| predicate(window))
    .map(window_info)
    .ok_or_else(|| error("Window was not found"))
}

#[napi]
pub fn foreground_window() -> Result<WindowInfo> {
  find_window(SCWindow::is_active)
}

#[napi]
pub fn window_from_name(title: String) -> Result<WindowInfo> {
  find_window(|window| window.title().is_some_and(|value| value == title))
}

#[napi]
pub fn window_from_contains_name(title: String) -> Result<WindowInfo> {
  find_window(|window| {
    window
      .title()
      .is_some_and(|value| value.contains(title.as_str()))
  })
}

#[napi]
pub fn window_from_handle(handle: i64) -> Result<WindowInfo> {
  let window_id =
    u32::try_from(handle).map_err(|_| error("handle must be a valid macOS window ID"))?;
  find_window(|window| window.window_id() == window_id)
}

#[napi]
pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
  let content = SCShareableContent::get().map_err(error)?;
  Ok(
    content
      .windows()
      .iter()
      .filter(|window| window.is_on_screen())
      .map(window_info)
      .collect(),
  )
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

#[napi(object)]
pub struct DxgiSessionOptions {
  pub monitor_index: Option<u32>,
  pub supported_formats: Option<Vec<DxgiDuplicationFormat>>,
}

#[napi]
pub struct DxgiDuplicationSession;

#[napi]
impl DxgiDuplicationSession {
  #[napi(constructor)]
  pub fn new(_options: Option<DxgiSessionOptions>) -> Result<Self> {
    Err(unsupported("DXGI Desktop Duplication"))
  }

  #[napi(getter)]
  pub const fn width(&self) -> u32 {
    0
  }

  #[napi(getter)]
  pub const fn height(&self) -> u32 {
    0
  }

  #[napi(getter)]
  pub const fn format(&self) -> DxgiDuplicationFormat {
    DxgiDuplicationFormat::Bgra8
  }

  #[napi(getter)]
  pub fn refresh_rate(&self) -> Vec<u32> {
    vec![0, 1]
  }

  #[napi]
  pub fn acquire_next_frame(&mut self, _timeout_ms: Option<u32>) -> Result<Option<Frame>> {
    Err(unsupported("DXGI Desktop Duplication"))
  }

  #[napi]
  pub fn recreate(&mut self) -> Result<()> {
    Err(unsupported("DXGI Desktop Duplication"))
  }

  #[napi]
  pub fn switch_monitor(&mut self, _monitor_index: u32) -> Result<()> {
    Err(unsupported("DXGI Desktop Duplication"))
  }
}

#[derive(Clone, Copy)]
#[napi(string_enum = "camelCase")]
pub enum ImageEncoderPixelFormat {
  Rgb16F,
  Bgra8,
  Rgba8,
}

#[napi]
pub struct ImageEncoder {
  format: ImageFormat,
  pixel_format: ImageEncoderPixelFormat,
}

#[napi]
impl ImageEncoder {
  #[napi(constructor)]
  pub fn new(format: ImageFormat, pixel_format: ImageEncoderPixelFormat) -> Result<Self> {
    if matches!(format, ImageFormat::JpegXr) {
      return Err(unsupported("JPEG XR image encoding"));
    }
    if matches!(pixel_format, ImageEncoderPixelFormat::Rgb16F) {
      return Err(unsupported("RGB16F image encoding"));
    }
    Ok(Self {
      format,
      pixel_format,
    })
  }

  #[napi]
  pub fn encode(&self, buffer: Buffer, width: u32, height: u32) -> Result<Buffer> {
    let pixel_format = match self.pixel_format {
      ImageEncoderPixelFormat::Bgra8 => ColorFormat::Bgra8,
      ImageEncoderPixelFormat::Rgba8 => ColorFormat::Rgba8,
      ImageEncoderPixelFormat::Rgb16F => unreachable!(),
    };
    encode_image(&buffer, width, height, pixel_format, self.format).map(Buffer::from)
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

#[napi]
pub struct VideoEncoder;

#[napi]
impl VideoEncoder {
  #[napi(constructor)]
  pub fn new(_options: VideoEncoderOptions) -> Result<Self> {
    Err(unsupported("Video encoding"))
  }

  #[napi]
  pub fn send_frame(&mut self, _frame: &Frame) -> Result<()> {
    Err(unsupported("Video encoding"))
  }

  #[napi]
  pub fn send_frame_with_audio(&mut self, _frame: &Frame, _audio_buffer: Buffer) -> Result<()> {
    Err(unsupported("Video encoding"))
  }

  #[napi]
  pub fn send_frame_buffer(&mut self, _buffer: Buffer, _timestamp: i64) -> Result<()> {
    Err(unsupported("Video encoding"))
  }

  #[napi]
  pub fn send_audio_buffer(&mut self, _buffer: Buffer, _timestamp: Option<i64>) -> Result<()> {
    Err(unsupported("Video encoding"))
  }

  #[napi]
  pub fn finish(&mut self) -> Result<Option<Buffer>> {
    Err(unsupported("Video encoding"))
  }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
  if let Some(s) = panic.downcast_ref::<String>() {
    s.clone()
  } else if let Some(s) = panic.downcast_ref::<&str>() {
    s.to_string()
  } else {
    "macOS capture thread panicked with an unknown payload".to_owned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn should_process_known_content_statuses() {
    assert!(should_process_frame(Some(SCFrameStatus::Complete)));
    assert!(should_process_frame(Some(SCFrameStatus::Started)));
  }

  #[test]
  fn should_skip_explicit_non_content_statuses() {
    assert!(!should_process_frame(Some(SCFrameStatus::Idle)));
    assert!(!should_process_frame(Some(SCFrameStatus::Blank)));
    assert!(!should_process_frame(Some(SCFrameStatus::Suspended)));
    assert!(!should_process_frame(Some(SCFrameStatus::Stopped)));
  }

  #[test]
  fn should_process_unknown_status() {
    assert!(should_process_frame(None));
  }

  #[test]
  fn panic_message_from_string() {
    let msg = panic_message(Box::new("test panic".to_string()));
    assert_eq!(msg, "test panic");
  }

  #[test]
  fn panic_message_from_str() {
    let msg = panic_message(Box::new("test panic str"));
    assert_eq!(msg, "test panic str");
  }

  #[test]
  fn panic_message_from_unknown() {
    let msg = panic_message(Box::new(42_u32));
    assert!(msg.contains("unknown payload"));
  }
}
