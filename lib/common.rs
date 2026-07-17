use napi::Status;
use napi::bindgen_prelude::Error;

pub(crate) fn error(message: impl ToString) -> Error {
  Error::new(Status::GenericFailure, message.to_string())
}

pub(crate) const fn bytes_per_pixel(is_float: bool) -> u32 {
  if is_float { 8 } else { 4 }
}

/// Compute (row_pitch, frame_size) from width, height, and bytes-per-pixel.
/// Uses checked arithmetic to prevent overflow.  Returns `napi::Error` for
/// invalid (zero) bpp or overflow.
///
/// Platform bindings can call this instead of open-coding `width * bpp` and
/// `height * row_pitch`.
pub(crate) fn frame_pitches(width: u32, height: u32, bpp: u32) -> Result<(u32, u32), Error> {
  if bpp == 0 {
    return Err(error("bytes_per_pixel must be greater than zero"));
  }
  let row_pitch = width
    .checked_mul(bpp)
    .ok_or_else(|| error("Arithmetic overflow: width * bytes_per_pixel"))?;
  let frame_size = height
    .checked_mul(row_pitch)
    .ok_or_else(|| error("Arithmetic overflow: height * row_pitch"))?;
  Ok((row_pitch, frame_size))
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
  // 1. Validate bytes_per_pixel > 0
  if bytes_per_pixel == 0 {
    return Err(error("bytes_per_pixel must be greater than zero"));
  }

  // 2. Validate crop rectangle within source bounds
  if start_x >= end_x || start_y >= end_y || end_x > source_width || end_y > source_height {
    return Err(error("Invalid crop rectangle"));
  }

  // 3. Validate source_row_pitch >= source_width * bytes_per_pixel (checked)
  let min_src_row_pitch = source_width
    .checked_mul(bytes_per_pixel)
    .ok_or_else(|| error("Arithmetic overflow: source_width * bytes_per_pixel"))?;
  if source_row_pitch < min_src_row_pitch {
    return Err(error(format!(
      "source_row_pitch ({source_row_pitch}) is less than minimum ({min_src_row_pitch}) for {source_width}×{source_height} @ {bytes_per_pixel} bpp"
    )));
  }

  // 4. Validate buffer length covers the declared image dimensions.
  //    Minimum = (source_height - 1) * source_row_pitch + source_width * bytes_per_pixel
  if source_height > 0 && source_width > 0 {
    // Safe: source_height > 0 so (source_height - 1) does not underflow.
    let h_minus_1 = source_height - 1;
    let min_buffer_len: usize = (|| {
      let rows_size: u32 = h_minus_1.checked_mul(source_row_pitch)?;
      let total: u32 = rows_size.checked_add(min_src_row_pitch)?;
      Some(total as usize)
    })()
    .ok_or_else(|| {
      error("Arithmetic overflow computing minimum buffer length from image dimensions")
    })?;

    if buffer.len() < min_buffer_len {
      return Err(error(format!(
        "buffer length ({}) is less than required minimum ({min_buffer_len}) for image {}×{} with row pitch {source_row_pitch}",
        buffer.len(),
        source_width,
        source_height
      )));
    }
  }

  // 5. Destination dimensions (checked subtraction cannot underflow – guaranteed by crop validation above)
  let crop_width = end_x - start_x;
  let crop_height = end_y - start_y;

  // 6. Destination row pitch and allocation size (checked)
  let dest_row_pitch = crop_width
    .checked_mul(bytes_per_pixel)
    .ok_or_else(|| error("Arithmetic overflow: crop_width * bytes_per_pixel"))?;
  let dest_alloc_size = crop_height
    .checked_mul(dest_row_pitch)
    .ok_or_else(|| error("Arithmetic overflow: crop_height * dest_row_pitch"))?;

  let dst_len: usize = dest_alloc_size as usize;
  let mut cropped = vec![0_u8; dst_len];

  // Per-row helper: checked offset for source start
  let src_row_offset = |y: u32| -> Result<u32, Error> {
    y.checked_mul(source_row_pitch)
      .ok_or_else(|| error("Arithmetic overflow: y * source_row_pitch"))
  };

  let src_x_offset = start_x
    .checked_mul(bytes_per_pixel)
    .ok_or_else(|| error("Arithmetic overflow: start_x * bytes_per_pixel"))?;

  let drp = dest_row_pitch as usize;

  // 7. Copy rows with checked offsets
  for row in 0..crop_height {
    let src_y = start_y
      .checked_add(row)
      .ok_or_else(|| error("Arithmetic overflow: start_y + row"))?;

    let source_start = src_row_offset(src_y)?
      .checked_add(src_x_offset)
      .ok_or_else(|| error("Arithmetic overflow: source_start"))? as usize;

    let source_end = source_start
      .checked_add(drp)
      .ok_or_else(|| error("Arithmetic overflow: source_end"))?;

    let dest_start = (row as usize)
      .checked_mul(drp)
      .ok_or_else(|| error("Arithmetic overflow: dest_start"))?;
    let dest_end = dest_start
      .checked_add(drp)
      .ok_or_else(|| error("Arithmetic overflow: dest_end"))?;

    // Defensive bounds check before slicing
    if source_end > buffer.len() {
      return Err(error(format!(
        "source row {row} overruns buffer: need bytes up to {source_end}, buffer has {}",
        buffer.len()
      )));
    }
    if dest_end > dst_len {
      return Err(error(format!(
        "destination row {row} overruns allocation: need bytes up to {dest_end}, allocation has {dst_len}"
      )));
    }

    cropped[dest_start..dest_end].copy_from_slice(&buffer[source_start..source_end]);
  }

  Ok((cropped, crop_width, crop_height))
}

#[cfg(test)]
mod tests {
  use super::*;

  // ---------------------------------------------------------------------------
  // frame_pitches
  // ---------------------------------------------------------------------------

  #[test]
  fn pitches_normal() {
    let (rp, fs) = frame_pitches(1920, 1080, 4).expect("valid pitches");
    assert_eq!(rp, 7680); // 1920 * 4
    assert_eq!(fs, 8_294_400); // 1080 * 7680
  }

  #[test]
  fn pitches_bpp_zero() {
    let e = frame_pitches(100, 100, 0).unwrap_err();
    assert!(e.to_string().contains("bytes_per_pixel"));
  }

  #[test]
  fn pitches_overflow_row_pitch() {
    // 0x5555_5555 * 4 = 0x1_5555_5554 > u32::MAX
    let e = frame_pitches(0x5555_5555, 1, 4).unwrap_err();
    assert!(e.to_string().contains("overflow"));
  }

  #[test]
  fn pitches_overflow_frame_size() {
    // row_pitch = 4, height = 0x4000_0001 => overflows u32
    let e = frame_pitches(1, 0x4000_0001, 4).unwrap_err();
    assert!(e.to_string().contains("overflow"));
  }

  #[test]
  fn pitches_zero_dimensions() {
    let (rp, fs) = frame_pitches(0, 0, 4).expect("zero dims ok");
    assert_eq!(rp, 0);
    assert_eq!(fs, 0);

    let (rp, fs) = frame_pitches(100, 0, 4).expect("zero height ok");
    assert_eq!(rp, 400);
    assert_eq!(fs, 0);
  }

  // ---------------------------------------------------------------------------
  // crop_buffer – invalid inputs
  // ---------------------------------------------------------------------------

  #[test]
  fn crop_bpp_zero() {
    let buf = vec![0u8; 100];
    let e = crop_buffer(&buf, 10, 10, 40, 0, 0, 0, 10, 10).unwrap_err();
    assert!(e.to_string().contains("bytes_per_pixel"));
  }

  #[test]
  fn crop_invalid_rectangle() {
    let buf = vec![0u8; 400];
    // end_x < start_x (inverted)
    let e = crop_buffer(&buf, 10, 10, 40, 4, 6, 0, 5, 10).unwrap_err();
    assert!(e.to_string().contains("Invalid crop rectangle"));

    // end_y < start_y (inverted)
    let e = crop_buffer(&buf, 10, 10, 40, 4, 0, 6, 10, 5).unwrap_err();
    assert!(e.to_string().contains("Invalid crop rectangle"));

    // end_x > source_width
    let e = crop_buffer(&buf, 10, 10, 40, 4, 0, 0, 11, 10).unwrap_err();
    assert!(e.to_string().contains("Invalid crop rectangle"));

    // end_y > source_height
    let e = crop_buffer(&buf, 10, 10, 40, 4, 0, 0, 10, 11).unwrap_err();
    assert!(e.to_string().contains("Invalid crop rectangle"));
  }

  #[test]
  fn crop_row_pitch_too_small() {
    let buf = vec![0u8; 1000];
    // source_width=10, bpp=4 => min row pitch = 40.  Feed 20.
    let e = crop_buffer(&buf, 10, 10, 20, 4, 0, 0, 10, 10).unwrap_err();
    assert!(e.to_string().contains("source_row_pitch"));
  }

  #[test]
  fn crop_undersized_buffer() {
    // 10×10 @ 4 bpp, row_pitch=40 => minimum = (10-1)*40 + 40 = 400 bytes
    let buf = vec![0u8; 200]; // too small
    let e = crop_buffer(&buf, 10, 10, 40, 4, 0, 0, 10, 10).unwrap_err();
    assert!(e.to_string().contains("buffer length"));
  }

  #[test]
  fn crop_source_width_overflow() {
    // 0x4000_0001 * 4 > u32::MAX
    let buf = vec![0u8; 16];
    let e = crop_buffer(&buf, 0x4000_0001, 1, 40, 4, 0, 0, 1, 1).unwrap_err();
    assert!(e.to_string().contains("overflow"));
  }

  #[test]
  fn crop_source_height_times_row_pitch_overflow() {
    // source_width=1, source_height=0x4000_0001, row_pitch=4 => min buffer calc overflows
    let buf = vec![0u8; 16];
    let e = crop_buffer(&buf, 1, 0x4000_0001, 4, 4, 0, 0, 1, 1).unwrap_err();
    assert!(e.to_string().contains("overflow"));
  }

  // ---------------------------------------------------------------------------
  // crop_buffer – valid padded-row crop
  // ---------------------------------------------------------------------------

  #[test]
  fn crop_valid_full_frame() {
    // 4×3 RGBA image, row_pitch = 20 (5 pixel stride → 16 pixel padding bytes per row)
    let width = 4u32;
    let height = 3u32;
    let row_pitch = 20u32;
    let bpp = 4u32;
    // Fill with distinguishable pattern: pixel (x,y) = pattern byte repeated 4×
    let mut buf = vec![0u8; 60]; // 3 * 20
    for y in 0..height {
      for x in 0..width {
        let p = ((y * width + x) * 4) as u8;
        let off = (y * row_pitch + x * bpp) as usize;
        buf[off] = p;
        buf[off + 1] = p + 1;
        buf[off + 2] = p + 2;
        buf[off + 3] = p + 3;
      }
    }

    let (cropped, cw, ch) = crop_buffer(&buf, width, height, row_pitch, bpp, 0, 0, width, height)
      .expect("full-frame crop");
    assert_eq!(cw, width);
    assert_eq!(ch, height);
    assert_eq!(cropped.len(), (width * height * bpp) as usize);
    // First pixel
    assert_eq!(cropped[0], 0);
    assert_eq!(cropped[3], 3);
    // Last pixel is at source offset: row 2 * 20 + x 3 * 4 = 52
    let last_src_off = ((height - 1) * row_pitch + (width - 1) * bpp) as usize;
    let last_dst_off = ((height - 1) * width + (width - 1)) as usize * bpp as usize;
    assert_eq!(cropped[last_dst_off], buf[last_src_off]);
  }

  #[test]
  fn crop_valid_sub_rect_with_padding() {
    // 6×6 RGBA image, row_pitch = 40 (> 6*4=24)
    let mut buf = vec![0u8; 6 * 40];
    for y in 0..6u32 {
      for x in 0..6u32 {
        let v = (y * 6 + x) as u8;
        let off = (y * 40 + x * 4) as usize;
        buf[off] = v;
        buf[off + 1] = v;
        buf[off + 2] = v;
        buf[off + 3] = v;
      }
    }

    // Crop [2,2] → [5,5] (3×3)
    let (cropped, cw, ch) = crop_buffer(&buf, 6, 6, 40, 4, 2, 2, 5, 5).expect("sub-rect crop");
    assert_eq!(cw, 3);
    assert_eq!(ch, 3);
    assert_eq!(cropped.len(), 36);

    // Verify first cropped pixel comes from source (2,2) = row offset 2*40 + 2*4 = 88
    // Source value at (2,2) = 2*6+2 = 14
    assert_eq!(cropped[0], 14);
    assert_eq!(buf[88], 14);

    // Verify last cropped pixel = source (4,4) = row 4*40 + 4*4 = 176, value 4*6+4=28
    let last_src_off = 4 * 40 + 4 * 4;
    let last_dst_off = (3 - 1) * 12 + (3 - 1) * 4; // row 2 * 12 + 8 = 32
    assert_eq!(cropped[last_dst_off as usize], 28);
    assert_eq!(buf[last_src_off as usize], 28);
  }

  #[test]
  fn crop_zero_area() {
    let buf: Vec<u8> = vec![];
    let error = crop_buffer(&buf, 0, 0, 0, 4, 0, 0, 0, 0).unwrap_err();
    assert!(error.to_string().contains("Invalid crop rectangle"));
  }
}
