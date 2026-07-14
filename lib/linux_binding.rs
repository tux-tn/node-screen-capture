use std::io::Cursor;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::common::{bytes_per_pixel, crop_buffer, error};
use ashpd::desktop::screencast::{
  CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
  StartCastOptions, Stream as PortalStream,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, Session};
use image::{DynamicImage, ImageBuffer, Rgba};
use napi::Status;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
  ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue,
};
use napi_derive::napi;
use pipewire as pw;
use pw::properties::properties;
use pw::spa;

fn unsupported(feature: &str) -> Error {
  error(format!(
    "{feature} is unavailable on Wayland; use the XDG ScreenCast portal through ScreenCapture"
  ))
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
    let row_pitch = width * bytes_per_pixel(matches!(self.color_format, ColorFormat::Rgba16F));

    Ok(Self {
      buffer,
      width,
      height,
      row_pitch,
      depth_pitch: row_pitch * height,
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
  let expected = width as usize * height as usize * 4;
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
type ClosedCallback = Arc<ThreadsafeFunction<(), UnknownReturnValue, (), Status, false, false, 1>>;
type FrameSink = Arc<dyn Fn(Frame) + Send + Sync>;
type ClosedSink = Arc<dyn Fn() + Send + Sync>;

struct WaylandCaptureControl {
  stop: Arc<AtomicBool>,
  thread: Option<JoinHandle<std::result::Result<(), String>>>,
}

impl WaylandCaptureControl {
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
      .map_err(|_| "Wayland capture thread panicked".to_owned())?
  }
}

enum CaptureControlAction {
  Stop,
  Wait,
}

pub struct CaptureControlTask {
  control: Option<WaylandCaptureControl>,
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
  inner: Option<WaylandCaptureControl>,
}

#[napi]
impl CaptureControl {
  #[napi(getter)]
  pub fn is_finished(&self) -> bool {
    self
      .inner
      .as_ref()
      .is_none_or(WaylandCaptureControl::is_finished)
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
    if matches!(options.color_format, Some(ColorFormat::Rgba16F)) {
      return Err(error(
        "Rgba16F capture is not supported by the Wayland backend",
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
    let on_frame: FrameSink = Arc::new(move |frame| {
      let _ = callback.call(frame, ThreadsafeFunctionCallMode::NonBlocking);
    });
    let on_closed: ClosedSink = if let Some(callback) = self.on_closed.take() {
      Arc::new(move || {
        let _ = callback.call((), ThreadsafeFunctionCallMode::NonBlocking);
      })
    } else {
      Arc::new(|| {})
    };
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let options = self.options.clone();
    let thread = thread::Builder::new()
      .name("screen-capture-wayland".to_owned())
      .spawn(move || {
        let result = run_wayland_capture(options, thread_stop, on_frame);
        on_closed();
        result
      })
      .map_err(error)?;

    Ok(CaptureControl {
      inner: Some(WaylandCaptureControl {
        stop,
        thread: Some(thread),
      }),
    })
  }
}
async fn open_portal(
  options: &CaptureOptions,
) -> std::result::Result<(Session<Screencast>, PortalStream, OwnedFd), String> {
  let connection = ashpd::zbus::Connection::session()
    .await
    .map_err(|error| error.to_string())?;
  let proxy = Screencast::with_connection(connection)
    .await
    .map_err(|error| error.to_string())?;
  let session = proxy
    .create_session(CreateSessionOptions::default())
    .await
    .map_err(|error| error.to_string())?;

  let sources = if options.window_name.is_some() || options.window_handle.is_some() {
    SourceType::Window.into()
  } else if options.use_picker.unwrap_or(false) {
    SourceType::Monitor | SourceType::Window
  } else {
    SourceType::Monitor.into()
  };
  let cursor_mode = if options.cursor_capture.unwrap_or(true) {
    CursorMode::Embedded
  } else {
    CursorMode::Hidden
  };
  proxy
    .select_sources(
      &session,
      SelectSourcesOptions::default()
        .set_cursor_mode(cursor_mode)
        .set_sources(sources)
        .set_multiple(false)
        .set_persist_mode(PersistMode::DoNot),
    )
    .await
    .map_err(|error| error.to_string())?
    .response()
    .map_err(|error| error.to_string())?;

  let response = proxy
    .start(&session, None, StartCastOptions::default())
    .await
    .map_err(|error| error.to_string())?
    .response()
    .map_err(|error| error.to_string())?;
  let stream = response
    .streams()
    .first()
    .cloned()
    .ok_or_else(|| "The ScreenCast portal returned no streams".to_owned())?;
  let fd = proxy
    .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
    .await
    .map_err(|error| error.to_string())?;
  Ok((session, stream, fd))
}

fn run_wayland_capture(
  options: CaptureOptions,
  stop: Arc<AtomicBool>,
  on_frame: FrameSink,
) -> std::result::Result<(), String> {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(|error| error.to_string())?;
  let (_session, portal_stream, fd) = runtime.block_on(open_portal(&options))?;
  run_pipewire_stream(
    fd,
    portal_stream.pipe_wire_node_id(),
    options.color_format.unwrap_or(ColorFormat::Bgra8),
    stop,
    on_frame,
  )
}

fn run_pipewire_stream(
  fd: OwnedFd,
  node_id: u32,
  requested_format: ColorFormat,
  stop: Arc<AtomicBool>,
  on_frame: FrameSink,
) -> std::result::Result<(), String> {
  pw::init();
  let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(|error| error.to_string())?;
  let context = pw::context::ContextRc::new(&mainloop, None).map_err(|error| error.to_string())?;
  let core = context
    .connect_fd_rc(fd, None)
    .map_err(|error| error.to_string())?;
  let stream = pw::stream::StreamRc::new(
    core,
    "screen-capture-wayland",
    properties! {
      *pw::keys::MEDIA_TYPE => "Video",
      *pw::keys::MEDIA_CATEGORY => "Capture",
      *pw::keys::MEDIA_ROLE => "Screen",
    },
  )
  .map_err(|error| error.to_string())?;

  struct UserData {
    format: spa::param::video::VideoInfoRaw,
  }
  let started = Instant::now();
  let process_stop = Arc::clone(&stop);
  let process_loop = mainloop.clone();
  let listener = stream
    .add_local_listener_with_user_data(UserData {
      format: Default::default(),
    })
    .param_changed(|_, user_data, id, param| {
      let Some(param) = param else { return };
      if id != spa::param::ParamType::Format.as_raw() {
        return;
      }
      let Ok((media_type, media_subtype)) = spa::param::format_utils::parse_format(param) else {
        return;
      };
      if media_type == spa::param::format::MediaType::Video
        && media_subtype == spa::param::format::MediaSubtype::Raw
      {
        let _ = user_data.format.parse(param);
      }
    })
    .process(move |stream, user_data| {
      if process_stop.load(Ordering::Acquire) {
        process_loop.quit();
        return;
      }
      let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
      };
      let datas = buffer.datas_mut();
      let Some(data) = datas.first_mut() else {
        return;
      };
      let size = user_data.format.size();
      if size.width == 0 || size.height == 0 {
        return;
      }
      let stride = data.chunk().stride();
      let offset = data.chunk().offset() as usize;
      let row_stride = if stride == 0 {
        size.width as usize * 4
      } else {
        stride.unsigned_abs() as usize
      };
      let Some(payload) = data.data() else { return };
      let required = row_stride.saturating_mul(size.height as usize);
      if payload.len() < offset.saturating_add(required) {
        return;
      }

      let source_format = user_data.format.format();
      let mut pixels = vec![0_u8; size.width as usize * size.height as usize * 4];
      for output_y in 0..size.height as usize {
        let source_y = if stride < 0 {
          size.height as usize - output_y - 1
        } else {
          output_y
        };
        let source_row = offset + source_y * row_stride;
        let destination_row = output_y * size.width as usize * 4;
        for x in 0..size.width as usize {
          let source = source_row + x * 4;
          let destination = destination_row + x * 4;
          let (r, g, b, a) = match source_format {
            spa::param::video::VideoFormat::BGRx => (
              payload[source + 2],
              payload[source + 1],
              payload[source],
              255,
            ),
            spa::param::video::VideoFormat::BGRA => (
              payload[source + 2],
              payload[source + 1],
              payload[source],
              payload[source + 3],
            ),
            spa::param::video::VideoFormat::RGBx => (
              payload[source],
              payload[source + 1],
              payload[source + 2],
              255,
            ),
            spa::param::video::VideoFormat::RGBA => (
              payload[source],
              payload[source + 1],
              payload[source + 2],
              payload[source + 3],
            ),
            _ => return,
          };
          if matches!(requested_format, ColorFormat::Bgra8) {
            pixels[destination..destination + 4].copy_from_slice(&[b, g, r, a]);
          } else {
            pixels[destination..destination + 4].copy_from_slice(&[r, g, b, a]);
          }
        }
      }

      let row_pitch = size.width * 4;
      let timestamp = (started.elapsed().as_nanos() / 100).min(i64::MAX as u128) as i64;
      on_frame(Frame {
        buffer: pixels,
        width: size.width,
        height: size.height,
        row_pitch,
        depth_pitch: row_pitch * size.height,
        timestamp,
        color_format: requested_format,
        dirty_regions: Vec::new(),
      });
    })
    .register()
    .map_err(|error| error.to_string())?;

  let format_object = spa::pod::object!(
    spa::utils::SpaTypes::ObjectParamFormat,
    spa::param::ParamType::EnumFormat,
    spa::pod::property!(
      spa::param::format::FormatProperties::MediaType,
      Id,
      spa::param::format::MediaType::Video
    ),
    spa::pod::property!(
      spa::param::format::FormatProperties::MediaSubtype,
      Id,
      spa::param::format::MediaSubtype::Raw
    ),
    spa::pod::property!(
      spa::param::format::FormatProperties::VideoFormat,
      Choice,
      Enum,
      Id,
      spa::param::video::VideoFormat::BGRx,
      spa::param::video::VideoFormat::BGRx,
      spa::param::video::VideoFormat::BGRA,
      spa::param::video::VideoFormat::RGBx,
      spa::param::video::VideoFormat::RGBA,
    ),
    spa::pod::property!(
      spa::param::format::FormatProperties::VideoSize,
      Choice,
      Range,
      Rectangle,
      spa::utils::Rectangle {
        width: 1920,
        height: 1080
      },
      spa::utils::Rectangle {
        width: 1,
        height: 1
      },
      spa::utils::Rectangle {
        width: 7680,
        height: 4320
      }
    ),
    spa::pod::property!(
      spa::param::format::FormatProperties::VideoFramerate,
      Choice,
      Range,
      Fraction,
      spa::utils::Fraction { num: 60, denom: 1 },
      spa::utils::Fraction { num: 0, denom: 1 },
      spa::utils::Fraction { num: 240, denom: 1 }
    ),
  );
  let values = spa::pod::serialize::PodSerializer::serialize(
    Cursor::new(Vec::new()),
    &spa::pod::Value::Object(format_object),
  )
  .map_err(|error| error.to_string())?
  .0
  .into_inner();
  let mut params = [spa::pod::Pod::from_bytes(&values)
    .ok_or_else(|| "Failed to build PipeWire format parameters".to_owned())?];
  stream
    .connect(
      spa::utils::Direction::Input,
      Some(node_id),
      pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
      &mut params,
    )
    .map_err(|error| error.to_string())?;

  let timer_loop = mainloop.clone();
  let timer_stop = Arc::clone(&stop);
  let timer = mainloop.loop_().add_timer(move |_| {
    if timer_stop.load(Ordering::Acquire) {
      timer_loop.quit();
    }
  });
  timer
    .update_timer(
      Some(Duration::from_millis(20)),
      Some(Duration::from_millis(20)),
    )
    .into_result()
    .map_err(|error| format!("Failed to arm PipeWire stop timer: {error:?}"))?;
  mainloop.run();
  drop(listener);
  Ok(())
}

#[napi]
pub fn is_supported() -> Result<bool> {
  if std::env::var_os("WAYLAND_DISPLAY").is_none() {
    return Ok(false);
  }
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(error)?;
  Ok(
    runtime
      .block_on(async {
        let connection = ashpd::zbus::Connection::session().await.ok()?;
        let proxy = Screencast::with_connection(connection).await.ok()?;
        proxy.available_source_types().await.ok()
      })
      .is_some(),
  )
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
  let graphics_capture = is_supported()?;
  Ok(CaptureApiSupport {
    graphics_capture,
    cursor_settings: graphics_capture,
    border_settings: false,
    secondary_windows: false,
    minimum_update_interval: false,
    dirty_regions: false,
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

#[napi]
pub fn primary_monitor() -> Result<MonitorInfo> {
  Err(unsupported("Monitor discovery"))
}

#[napi]
pub fn monitor_from_index(_index: u32) -> Result<MonitorInfo> {
  Err(unsupported("Monitor discovery"))
}

#[napi]
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
  Err(unsupported("Monitor discovery"))
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

#[napi]
pub fn foreground_window() -> Result<WindowInfo> {
  Err(unsupported("Window discovery"))
}

#[napi]
pub fn window_from_name(_title: String) -> Result<WindowInfo> {
  Err(unsupported("Window discovery"))
}

#[napi]
pub fn window_from_contains_name(_title: String) -> Result<WindowInfo> {
  Err(unsupported("Window discovery"))
}

#[napi]
pub fn window_from_handle(_handle: i64) -> Result<WindowInfo> {
  Err(unsupported("Window discovery"))
}

#[napi]
pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
  Err(unsupported("Window discovery"))
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
