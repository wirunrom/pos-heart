//! ESC/POS byte stream for a raster receipt: init, image bands, feed, cut,
//! and optionally the cash-drawer pulse on the printer's drawer port.
use image::GrayImage;

const BAND: u32 = 128;

/// Image rows are sent in bands so small printer buffers keep up.
pub fn encode(img: &GrayImage, cut: bool, drawer: bool) -> Vec<u8> {
    let mut out = vec![0x1B, b'@'];
    if drawer {
        out.extend(drawer_pulse());
    }
    let width = img.width();
    let bytes_per_row = width.div_ceil(8);
    let mut y = 0;
    while y < img.height() {
        let rows = BAND.min(img.height() - y);
        // GS v 0 m xL xH yL yH
        out.extend([0x1D, b'v', b'0', 0]);
        out.extend(u16_le(bytes_per_row));
        out.extend(u16_le(rows));
        for row in y..y + rows {
            for byte in 0..bytes_per_row {
                let mut bits = 0u8;
                for bit in 0..8 {
                    let x = byte * 8 + bit;
                    if x < width && img.get_pixel(x, row).0[0] < 128 {
                        bits |= 0x80 >> bit;
                    }
                }
                out.push(bits);
            }
        }
        y += rows;
    }
    // ESC d 4: feed so the last line clears the cutter.
    out.extend([0x1B, b'd', 4]);
    if cut {
        // GS V 66 0: feed to cut position and partial cut.
        out.extend([0x1D, b'V', 66, 0]);
    }
    out
}
/// ESC p 0 25 250: pulse drawer pin 2 (50 ms on, 500 ms off).
pub fn drawer_pulse() -> [u8; 5] {
    [0x1B, b'p', 0, 25, 250]
}
fn u16_le(n: u32) -> [u8; 2] {
    let n = u16::try_from(n).unwrap_or(u16::MAX);
    n.to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;
    #[test]
    fn raster_bands_pack_black_pixels_msb_first() {
        let mut img = GrayImage::from_pixel(10, 130, Luma([255]));
        img.put_pixel(0, 0, Luma([0]));
        img.put_pixel(9, 0, Luma([0]));
        let bytes = encode(&img, true, true);
        assert_eq!(&bytes[..2], &[0x1B, b'@']);
        assert_eq!(&bytes[2..7], &drawer_pulse());
        // First band header: 2 bytes per row, 128 rows.
        assert_eq!(&bytes[7..15], &[0x1D, b'v', b'0', 0, 2, 0, 128, 0]);
        assert_eq!(&bytes[15..17], &[0x80, 0x40]);
        // Second band holds the last 2 rows.
        let second = 15 + 2 * 128;
        assert_eq!(
            &bytes[second..second + 8],
            &[0x1D, b'v', b'0', 0, 2, 0, 2, 0]
        );
        assert!(bytes.ends_with(&[0x1B, b'd', 4, 0x1D, b'V', 66, 0]));
    }
}
