//! Pixels of a thumbnail on their way from the decoder to the view: a
//! plain buffer in GDK's download format (Cairo ARGB32, premultiplied,
//! native byte order), scaled down by area averaging and, for opaque
//! pictures, framed as Nautilus 2.30 framed its thumbnails
//! (`thumbnail_frame.png`: a thin dark edge and a soft shadow to the right
//! and below). Works in the worker threads; only the finished
//! `gdk::MemoryTexture` goes to the main thread.

use gtk4::prelude::*;
use gtk4::{gdk, glib};

/// Pixels in GDK's download format (B, G, R, A on little endian, alpha
/// premultiplied), 4 bytes per pixel, no padding.
#[derive(Clone, Debug, PartialEq)]
pub struct Bild {
    pub breite: u32,
    pub hoehe: u32,
    pub px: Vec<u8>,
}

/// Width of the frame left/top and right/bottom (edge + shadow).
pub const RAHMEN_LINKS: u32 = 1;
pub const RAHMEN_RECHTS: u32 = 4;

impl Bild {
    pub fn leer(breite: u32, hoehe: u32) -> Self {
        Self {
            breite,
            hoehe,
            px: vec![0; (breite * hoehe * 4) as usize],
        }
    }

    /// The pixels of `textur`.
    pub fn aus_textur(textur: &gdk::Texture) -> Option<Self> {
        let (b, h) = (textur.width(), textur.height());
        if b <= 0 || h <= 0 {
            return None;
        }
        let (b, h) = (b as u32, h as u32);
        let mut px = vec![0u8; (b * h * 4) as usize];
        textur.download(&mut px, (b * 4) as usize);
        Some(Self {
            breite: b,
            hoehe: h,
            px,
        })
    }

    /// From straight RGB(A) rows (GdkPixbuf's layout).
    pub fn aus_rgba(breite: u32, hoehe: u32, kanaele: u32, zeile: usize, daten: &[u8]) -> Self {
        let mut bild = Self::leer(breite, hoehe);
        for y in 0..hoehe as usize {
            for x in 0..breite as usize {
                let q = y * zeile + x * kanaele as usize;
                let Some(p) = daten.get(q..q + kanaele as usize) else {
                    continue;
                };
                let a = if kanaele >= 4 { p[3] } else { 255 };
                let mul = |c: u8| ((u32::from(c) * u32::from(a) + 127) / 255) as u8;
                let z = (y * breite as usize + x) * 4;
                bild.px[z..z + 4].copy_from_slice(&argb(mul(p[0]), mul(p[1]), mul(p[2]), a));
            }
        }
        bild
    }

    /// Whether any pixel is not fully opaque.
    pub fn hat_transparenz(&self) -> bool {
        self.px.chunks_exact(4).any(|p| alpha(p) < 255)
    }

    /// The size whose long edge is at most `kante` (never larger than now).
    pub fn mass_fuer(&self, kante: u32) -> (u32, u32) {
        mass_fuer(self.breite, self.hoehe, kante)
    }

    /// Scaled down by area averaging so the long edge is at most `kante`;
    /// smaller pictures stay as they are.
    pub fn verkleinert(&self, kante: u32) -> Self {
        let (nb, nh) = self.mass_fuer(kante);
        if (nb, nh) == (self.breite, self.hoehe) {
            return self.clone();
        }
        let (b, h) = (self.breite as usize, self.hoehe as usize);
        let (nb_, nh_) = (nb as usize, nh as usize);
        let mut out = Self::leer(nb, nh);
        // each target pixel averages the source rectangle it covers
        // (with fractional edges), in premultiplied space
        let sx = b as f64 / nb_ as f64;
        let sy = h as f64 / nh_ as f64;
        let mut summe = vec![0f64; nb_ * 4];
        for ty in 0..nh_ {
            summe.iter_mut().for_each(|s| *s = 0.0);
            let y0 = ty as f64 * sy;
            let y1 = y0 + sy;
            let mut y = y0.floor() as usize;
            while (y as f64) < y1 && y < h {
                let wy = (y1.min(y as f64 + 1.0) - y0.max(y as f64)).max(0.0);
                let zeile = &self.px[y * b * 4..(y + 1) * b * 4];
                for tx in 0..nb_ {
                    let x0 = tx as f64 * sx;
                    let x1 = x0 + sx;
                    let mut x = x0.floor() as usize;
                    let s = &mut summe[tx * 4..tx * 4 + 4];
                    while (x as f64) < x1 && x < b {
                        let w = wy * (x1.min(x as f64 + 1.0) - x0.max(x as f64)).max(0.0);
                        let p = &zeile[x * 4..x * 4 + 4];
                        for k in 0..4 {
                            s[k] += w * f64::from(p[k]);
                        }
                        x += 1;
                    }
                }
                y += 1;
            }
            let flaeche = sx * sy;
            for tx in 0..nb_ {
                for k in 0..4 {
                    let v = (summe[tx * 4 + k] / flaeche).round().clamp(0.0, 255.0);
                    out.px[(ty * nb_ + tx) * 4 + k] = v as u8;
                }
            }
        }
        out
    }

    /// The picture in a frame like Nautilus 2.30's: a 1 px edge of 35 %
    /// black around it and a shadow of three fading pixels to the right
    /// and below, offset by 2 px. The result is [`RAHMEN_LINKS`] +
    /// [`RAHMEN_RECHTS`] px wider and higher.
    pub fn gerahmt(&self) -> Self {
        let rand = RAHMEN_LINKS;
        let (b, h) = (self.breite, self.hoehe);
        let mut out = Self::leer(b + rand + RAHMEN_RECHTS, h + rand + RAHMEN_RECHTS);
        let ob = out.breite;
        let setzen = |out: &mut Bild, x: u32, y: u32, a: u8| {
            let z = ((y * ob + x) * 4) as usize;
            out.px[z..z + 4].copy_from_slice(&argb(0, 0, 0, a));
        };
        // shadow: 2 px offset, three pixels fading out
        for (i, a) in [64u8, 36, 14].into_iter().enumerate() {
            let i = i as u32;
            let rechts = b + 2 * rand + i;
            for y in (2 + i)..(h + 2 * rand + i + 1) {
                setzen(&mut out, rechts, y, a);
            }
            let unten = h + 2 * rand + i;
            for x in (2 + i)..(b + 2 * rand + i) {
                setzen(&mut out, x, unten, a);
            }
        }
        // the edge
        for x in 0..b + 2 * rand {
            setzen(&mut out, x, 0, 90);
            setzen(&mut out, x, h + 2 * rand - 1, 90);
        }
        for y in 0..h + 2 * rand {
            setzen(&mut out, 0, y, 90);
            setzen(&mut out, b + 2 * rand - 1, y, 90);
        }
        // the picture
        for y in 0..h {
            let q = (y * b * 4) as usize;
            let z = (((y + rand) * ob + rand) * 4) as usize;
            out.px[z..z + (b * 4) as usize].copy_from_slice(&self.px[q..q + (b * 4) as usize]);
        }
        out
    }

    /// A texture of the pixels (any thread: GDK textures are immutable).
    pub fn textur(&self) -> gdk::Texture {
        let bytes = glib::Bytes::from(&self.px[..]);
        gdk::MemoryTexture::new(
            self.breite as i32,
            self.hoehe as i32,
            if cfg!(target_endian = "little") {
                gdk::MemoryFormat::B8g8r8a8Premultiplied
            } else {
                gdk::MemoryFormat::A8r8g8b8Premultiplied
            },
            &bytes,
            (self.breite * 4) as usize,
        )
        .upcast()
    }

    /// A PNG of the pixels (no text chunks yet).
    pub fn png(&self) -> Vec<u8> {
        self.textur().save_to_png_bytes().to_vec()
    }
}

/// The size whose long edge is at most `kante`, keeping the aspect ratio
/// (at least 1 px; never larger than `b`×`h`).
pub fn mass_fuer(b: u32, h: u32, kante: u32) -> (u32, u32) {
    let lang = b.max(h);
    if lang <= kante || lang == 0 {
        return (b.max(1), h.max(1));
    }
    let f = f64::from(kante) / f64::from(lang);
    (
        ((f64::from(b) * f).round() as u32).max(1),
        ((f64::from(h) * f).round() as u32).max(1),
    )
}

/// One pixel in the download format from premultiplied components.
fn argb(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    if cfg!(target_endian = "little") {
        [b, g, r, a]
    } else {
        [a, r, g, b]
    }
}

fn alpha(p: &[u8]) -> u8 {
    if cfg!(target_endian = "little") {
        p[3]
    } else {
        p[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voll(b: u32, h: u32, farbe: [u8; 4]) -> Bild {
        let mut bild = Bild::leer(b, h);
        for p in bild.px.chunks_exact_mut(4) {
            p.copy_from_slice(&farbe);
        }
        bild
    }

    #[test]
    fn sizes_keep_the_aspect_ratio_and_never_grow() {
        assert_eq!(mass_fuer(4000, 3000, 128), (128, 96));
        assert_eq!(mass_fuer(3000, 4000, 128), (96, 128));
        assert_eq!(mass_fuer(100, 50, 128), (100, 50));
        assert_eq!(mass_fuer(10_000, 10, 128), (128, 1));
        assert_eq!(mass_fuer(0, 0, 128), (1, 1));
    }

    #[test]
    fn scaling_averages_areas() {
        // left half black, right half white: two target pixels keep it
        let mut bild = Bild::leer(4, 2);
        for y in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                let z = (y * 4 + x) * 4;
                bild.px[z..z + 4].copy_from_slice(&argb(v, v, v, 255));
            }
        }
        let klein = bild.verkleinert(2);
        assert_eq!((klein.breite, klein.hoehe), (2, 1));
        assert_eq!(&klein.px[0..4], &argb(0, 0, 0, 255));
        assert_eq!(&klein.px[4..8], &argb(255, 255, 255, 255));
        // a uniform colour stays the same at an odd factor
        let f = argb(10, 200, 30, 255);
        let k = voll(7, 5, f).verkleinert(3);
        assert_eq!((k.breite, k.hoehe), (3, 2));
        assert!(k.px.chunks_exact(4).all(|p| p == f));
        assert!(!k.hat_transparenz());
    }

    #[test]
    fn the_frame_surrounds_the_picture() {
        let f = argb(200, 100, 50, 255);
        let g = voll(10, 6, f).gerahmt();
        assert_eq!(
            (g.breite, g.hoehe),
            (
                10 + RAHMEN_LINKS + RAHMEN_RECHTS,
                6 + RAHMEN_LINKS + RAHMEN_RECHTS
            )
        );
        let px = |x: u32, y: u32| {
            let z = ((y * g.breite + x) * 4) as usize;
            g.px[z..z + 4].to_vec()
        };
        assert_eq!(px(1, 1), f.to_vec(), "Bild ab (1,1)");
        assert_eq!(alpha(&px(0, 0)), 90, "Rand");
        assert_eq!(alpha(&px(11, 3)), 90, "rechter Rand");
        assert_eq!(alpha(&px(12, 3)), 64, "Schatten");
        assert_eq!(alpha(&px(14, 4)), 14, "Schatten aussen");
        assert_eq!(alpha(&px(13, 0)), 0, "oben rechts frei");
        assert!(g.hat_transparenz());
    }

    #[test]
    fn straight_rgba_is_premultiplied() {
        let daten = [255u8, 0, 0, 128, 0, 255, 0, 255];
        let b = Bild::aus_rgba(2, 1, 4, 8, &daten);
        assert_eq!(&b.px[0..4], &argb(128, 0, 0, 128));
        assert_eq!(&b.px[4..8], &argb(0, 255, 0, 255));
        let rgb = [1u8, 2, 3];
        assert_eq!(&Bild::aus_rgba(1, 1, 3, 3, &rgb).px, &argb(1, 2, 3, 255));
    }
}
