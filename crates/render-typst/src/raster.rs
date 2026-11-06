//! From Typst's pixels to the file a display shows: turned as the model asks,
//! mapped to the nearest color of its palette, and written as a PNG of the fewest
//! bits per pixel that hold the palette (firmware 1.8.18 decodes 1 and 2-bit gray
//! and indexed PNGs, `png_draw` in `display.cpp`).

use png::{BitDepth, ColorType, Encoder};
use tiny_skia::Pixmap;

#[derive(Debug, thiserror::Error)]
#[error("PNG encoding: {0}")]
pub struct EncodeError(#[from] png::EncodingError);

/// Turns the image clockwise by `degrees`, a multiple of 90.
fn rotate(pixels: &[[u8; 3]], width: u32, height: u32, degrees: u32) -> (Vec<[u8; 3]>, u32, u32) {
    let (w, h) = (width as usize, height as usize);
    match degrees % 360 {
        90 => {
            let mut out = vec![[255; 3]; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[x * h + (h - 1 - y)] = pixels[y * w + x];
                }
            }
            (out, height, width)
        }
        180 => (pixels.iter().rev().copied().collect(), width, height),
        270 => {
            let mut out = vec![[255; 3]; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[(w - 1 - x) * h + y] = pixels[y * w + x];
                }
            }
            (out, height, width)
        }
        _ => (pixels.to_vec(), width, height),
    }
}

/// The pixmap's colors over white, as RGB.
fn opaque(pixmap: &Pixmap) -> Vec<[u8; 3]> {
    pixmap
        .pixels()
        .iter()
        .map(|pixel| {
            let color = pixel.demultiply();
            let alpha = u16::from(color.alpha());
            let over_white = |channel: u8| {
                u8::try_from((u16::from(channel) * alpha + 255 * (255 - alpha)) / 255)
                    .unwrap_or(255)
            };
            [
                over_white(color.red()),
                over_white(color.green()),
                over_white(color.blue()),
            ]
        })
        .collect()
}

/// Luminance below which a pixel is ink on a two-color palette. Thin strokes of
/// pixel fonts land between device pixels and come out half covered; a midpoint
/// threshold would drop them, while TRMNL's dither tiles are pure black and white
/// and are unaffected.
const INK_LUMA: u32 = 192;

/// Luminance below which a pixel is ink where it lies on a dark surface, such as
/// an inverse element: there the thin strokes are light, so a half-covered
/// pixel stays paper. It is the mirror of `INK_LUMA`.
const SURFACE_INK_LUMA: u32 = 255 - INK_LUMA;

/// How far around a pixel its surface is judged: a square of this radius, wider
/// than the strokes of text.
const SURFACE_RADIUS: usize = 3;

fn luma([r, g, b]: [u8; 3]) -> u32 {
    (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000
}

/// For a two-color palette, whether each pixel is ink: darker than `INK_LUMA`,
/// or than `SURFACE_INK_LUMA` where more of its surroundings are dark than light
/// (light content on a dark surface). Grays between count as neither, so a gray
/// area stays ink as before.
fn ink(pixels: &[[u8; 3]], width: usize, height: usize) -> Vec<bool> {
    let lumas: Vec<u32> = pixels.iter().copied().map(luma).collect();
    // Counts of dark and light pixels above and left of each corner, so a
    // square's counts take four lookups.
    let stride = width + 1;
    let mut dark = vec![0u32; stride * (height + 1)];
    let mut light = vec![0u32; stride * (height + 1)];
    for y in 0..height {
        for x in 0..width {
            let value = lumas[y * width + x];
            let (at, up, left, corner) = (
                (y + 1) * stride + x + 1,
                y * stride + x + 1,
                (y + 1) * stride + x,
                y * stride + x,
            );
            dark[at] = u32::from(value < SURFACE_INK_LUMA) + dark[up] + dark[left] - dark[corner];
            light[at] = u32::from(value >= INK_LUMA) + light[up] + light[left] - light[corner];
        }
    }
    let count = |table: &[u32], (x0, y0, x1, y1): (usize, usize, usize, usize)| {
        table[y1 * stride + x1] + table[y0 * stride + x0]
            - table[y0 * stride + x1]
            - table[y1 * stride + x0]
    };
    (0..width * height)
        .map(|at| {
            let (x, y) = (at % width, at / width);
            let square = (
                x.saturating_sub(SURFACE_RADIUS),
                y.saturating_sub(SURFACE_RADIUS),
                (x + SURFACE_RADIUS + 1).min(width),
                (y + SURFACE_RADIUS + 1).min(height),
            );
            let on_dark = count(&dark, square) > count(&light, square);
            lumas[at] < if on_dark { SURFACE_INK_LUMA } else { INK_LUMA }
        })
        .collect()
}

/// The index of the nearest palette color for `pixel`.
fn nearest(palette: &[[u8; 3]], pixel: [u8; 3]) -> u8 {
    let distance = |color: &[u8; 3]| {
        color
            .iter()
            .zip(pixel)
            .map(|(a, b)| (i32::from(*a) - i32::from(b)).pow(2))
            .sum::<i32>()
    };
    let index = (0..palette.len())
        .min_by_key(|&i| distance(&palette[i]))
        .unwrap_or(0);
    u8::try_from(index).unwrap_or(0)
}

/// Encodes the pixmap for a palette of fixed colors, or as full color when the
/// palette is `None`.
pub fn encode(
    pixmap: &Pixmap,
    rotation: u32,
    palette: Option<&[[u8; 3]]>,
) -> Result<Vec<u8>, EncodeError> {
    let pixels = opaque(pixmap);
    let (pixels, width, height) = rotate(&pixels, pixmap.width(), pixmap.height(), rotation);
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, width, height);
    encoder.set_compression(png::Compression::High);
    let Some(palette) = palette.filter(|palette| palette.len() <= 256) else {
        encoder.set_color(ColorType::Rgb);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(pixels.as_flattened())?;
        writer.finish()?;
        return Ok(out);
    };
    let bits: u8 = match palette.len() {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    };
    encoder.set_color(ColorType::Indexed);
    encoder.set_depth(BitDepth::from_u8(bits).expect("1, 2, 4 or 8 bits"));
    encoder.set_palette(palette.as_flattened().to_vec());
    let per_byte = 8 / usize::from(bits);
    let row_bytes = (width as usize).div_ceil(per_byte);
    let mut data = vec![0u8; row_bytes * height as usize];
    // On a two-color palette, the darker color for ink and the lighter one
    // otherwise; on any other, the nearest color.
    let two_color = if let [first, second] = palette {
        Some((
            u8::from(luma(*first) > luma(*second)),
            ink(&pixels, width as usize, height as usize),
        ))
    } else {
        None
    };
    for (y, row) in pixels.chunks(width as usize).enumerate() {
        for (x, pixel) in row.iter().enumerate() {
            let index = match &two_color {
                Some((dark, ink)) => {
                    if ink[y * width as usize + x] {
                        *dark
                    } else {
                        1 - dark
                    }
                }
                None => nearest(palette, *pixel),
            };
            let shift = 8 - usize::from(bits) * (x % per_byte + 1);
            data[y * row_bytes + x / per_byte] |= index << shift;
        }
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&data)?;
    writer.finish()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> (png::OutputInfo, Vec<u8>, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let palette = reader.info().palette.clone().unwrap_or_default().to_vec();
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        (info, buffer, palette)
    }

    fn pixmap(width: u32, height: u32, gray: &[u8]) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        for (pixel, value) in pixmap.pixels_mut().iter_mut().zip(gray) {
            *pixel = tiny_skia::ColorU8::from_rgba(*value, *value, *value, 255).premultiply();
        }
        pixmap
    }

    #[test]
    fn four_grays_make_a_two_bit_indexed_png_of_the_nearest_levels() {
        let palette = [[0; 3], [85; 3], [170; 3], [255; 3]];
        let bytes = encode(&pixmap(4, 1, &[10, 90, 160, 250]), 0, Some(&palette)).unwrap();
        let (info, data, colors) = decode(&bytes);
        assert_eq!(
            (info.color_type, info.bit_depth),
            (ColorType::Indexed, BitDepth::Two)
        );
        assert_eq!(colors, palette.as_flattened());
        assert_eq!(data, [0b0001_1011]);
    }

    #[test]
    fn half_covered_strokes_stay_ink_in_black_and_white() {
        let palette = [[0; 3], [255; 3]];
        let bytes = encode(&pixmap(3, 1, &[128, 180, 200]), 0, Some(&palette)).unwrap();
        let (_, data, _) = decode(&bytes);
        assert_eq!(data, [0b0010_0000]);
    }

    #[test]
    fn half_covered_light_strokes_on_a_dark_surface_stay_paper() {
        let palette = [[0; 3], [255; 3]];
        // A 9 by 9 black square with a half-covered light stroke down its middle,
        // beside the same stroke in dark on white: each keeps its stroke.
        let mut dark = vec![0u8; 81];
        let mut light = vec![255u8; 81];
        for y in 0..9 {
            dark[y * 9 + 4] = 128;
            light[y * 9 + 4] = 128;
        }
        let ink_of = |gray: &[u8]| {
            let bytes = encode(&pixmap(9, 9, gray), 0, Some(&palette)).unwrap();
            let (_, data, _) = decode(&bytes);
            // Two bytes a row; the middle pixel is bit 4 of the first, counting
            // from its high bit.
            (0..9)
                .map(|y| data[y * 2] & 0b0000_1000 == 0)
                .collect::<Vec<_>>()
        };
        assert_eq!(ink_of(&dark), [false; 9], "light stroke on dark");
        assert_eq!(ink_of(&light), [true; 9], "dark stroke on light");
    }

    #[test]
    fn a_uniform_gray_stays_ink_in_black_and_white() {
        let palette = [[0; 3], [255; 3]];
        let bytes = encode(&pixmap(9, 9, &[100; 81]), 0, Some(&palette)).unwrap();
        let (_, data, _) = decode(&bytes);
        assert!(data.iter().step_by(2).all(|byte| *byte == 0), "{data:?}");
    }

    #[test]
    fn black_and_white_makes_one_bit_and_turns_with_the_model() {
        let palette = [[0; 3], [255; 3]];
        // Two columns, one row: black then white; turned 90 degrees it is one
        // column of two rows, black on top.
        let bytes = encode(&pixmap(2, 1, &[0, 255]), 90, Some(&palette)).unwrap();
        let (info, data, _) = decode(&bytes);
        assert_eq!(
            (info.width, info.height, info.bit_depth),
            (1, 2, BitDepth::One)
        );
        assert_eq!(data, [0b0000_0000, 0b1000_0000]);
    }

    #[test]
    fn a_palette_of_unlisted_colors_makes_full_color() {
        let bytes = encode(&pixmap(1, 1, &[128]), 0, None).unwrap();
        let (info, data, _) = decode(&bytes);
        assert_eq!(info.color_type, ColorType::Rgb);
        assert_eq!(data, [128, 128, 128]);
    }
}
