//! Turning a drawn RGBA picture into the 4:2:0 planes H.264 takes.

use rusty_h264::YuvFrame;

/// The RGB to YCbCr weights a picture of this size is read with. A stream with no color tag is
/// read by players as BT.709 from 720 lines up and BT.601 below, so the picture is made the
/// way it will be read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Matrix {
    kr: f32,
    kb: f32,
}

impl Matrix {
    pub(crate) fn for_size(width: usize, height: usize) -> Self {
        if height >= 720 || width >= 1280 {
            Self {
                kr: 0.2126,
                kb: 0.0722,
            }
        } else {
            Self {
                kr: 0.299,
                kb: 0.114,
            }
        }
    }

    fn kg(self) -> f32 {
        1.0 - self.kr - self.kb
    }

    /// Limited-range (16 to 235, 16 to 240) Y, Cb and Cr of an 8-bit RGB color.
    pub(crate) fn ycc(self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let y = self.kr * r + self.kg() * g + self.kb * b;
        let cb = (b - y) / (2.0 * (1.0 - self.kb));
        let cr = (r - y) / (2.0 * (1.0 - self.kr));
        (
            16.0 + y * 219.0 / 255.0,
            128.0 + cb * 224.0 / 255.0,
            128.0 + cr * 224.0 / 255.0,
        )
    }

    /// The inverse of [`Matrix::ycc`]: what a player shows for these samples.
    #[cfg(test)]
    pub(crate) fn rgb(self, y: f32, cb: f32, cr: f32) -> (f32, f32, f32) {
        let y = (y - 16.0) * 255.0 / 219.0;
        let cb = (cb - 128.0) * 255.0 / 224.0;
        let cr = (cr - 128.0) * 255.0 / 224.0;
        let r = y + 2.0 * (1.0 - self.kr) * cr;
        let b = y + 2.0 * (1.0 - self.kb) * cb;
        let g = (y - self.kr * r - self.kb * b) / self.kg();
        (r, g, b)
    }
}

fn byte(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// `rgba` (`width * height` pixels, even sides) as planes. Each chroma sample is the average of
/// the four pixels it covers.
pub(crate) fn to_i420(rgba: &[u8], width: usize, height: usize) -> YuvFrame {
    let matrix = Matrix::for_size(width, height);
    let px = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        (
            f32::from(rgba[i]),
            f32::from(rgba[i + 1]),
            f32::from(rgba[i + 2]),
        )
    };
    let mut luma = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let (r, g, b) = px(x, y);
            luma[y * width + x] = byte(matrix.ycc(r, g, b).0);
        }
    }
    let (cw, ch) = (width / 2, height / 2);
    let mut u = vec![0u8; cw * ch];
    let mut v = vec![0u8; cw * ch];
    for cy in 0..ch {
        for cx in 0..cw {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (pr, pg, pb) = px(cx * 2 + dx, cy * 2 + dy);
                r += pr;
                g += pg;
                b += pb;
            }
            let (_, cb, cr) = matrix.ycc(r / 4.0, g / 4.0, b / 4.0);
            u[cy * cw + cx] = byte(cb);
            v[cy * cw + cx] = byte(cr);
        }
    }
    YuvFrame {
        width,
        height,
        y: luma,
        u,
        v,
    }
}
