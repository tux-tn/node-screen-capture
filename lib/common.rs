use napi::Status;
use napi::bindgen_prelude::Error;

pub(crate) fn error(message: impl ToString) -> Error {
  Error::new(Status::GenericFailure, message.to_string())
}

pub(crate) const fn bytes_per_pixel(is_float: bool) -> u32 {
  if is_float { 8 } else { 4 }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn crop_buffer(
  buffer: &[u8],
  source_width: u32,
  source_height: u32,
  source_row_pitch: u32,
  bytes_per_pixel: u32,
  start_x: u32,
  start_y: u32,
  end_x: u32,
  end_y: u32,
) -> Result<(Vec<u8>, u32, u32), Error> {
  if start_x >= end_x || start_y >= end_y || end_x > source_width || end_y > source_height {
    return Err(error("Invalid crop rectangle"));
  }

  let width = end_x - start_x;
  let height = end_y - start_y;
  let row_pitch = width * bytes_per_pixel;
  let mut cropped = vec![0; (row_pitch * height) as usize];

  for row in 0..height {
    let source_start = ((start_y + row) * source_row_pitch + start_x * bytes_per_pixel) as usize;
    let source_end = source_start + row_pitch as usize;
    let destination_start = (row * row_pitch) as usize;
    cropped[destination_start..destination_start + row_pitch as usize]
      .copy_from_slice(&buffer[source_start..source_end]);
  }

  Ok((cropped, width, height))
}
