//! Colores: parseo #RRGGBB, ajustes de luminosidad HSL y formatos de registro de Windows.

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn desde_hex(s: &str) -> Result<Rgb> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("color inválido: {s:?} (se espera #RRGGBB)");
        }
        let n = u32::from_str_radix(h, 16)?;
        Ok(Rgb { r: (n >> 16) as u8, g: (n >> 8) as u8, b: n as u8 })
    }

    pub fn a_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Formato de `AccentColor`, `AccentColorMenu`, `StartColorMenu`.
    pub fn abgr(&self) -> u32 {
        0xFF00_0000 | (self.b as u32) << 16 | (self.g as u32) << 8 | self.r as u32
    }

    /// Formato de `DWM\ColorizationColor`.
    pub fn argb(&self, alfa: u8) -> u32 {
        (alfa as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    /// Suma `delta` (−1.0..=1.0) a la luminosidad HSL.
    pub fn con_luz(&self, delta: f32) -> Rgb {
        let (h, s, l) = rgb_a_hsl(*self);
        hsl_a_rgb(h, s, (l + delta).clamp(0.0, 1.0))
    }
}

fn rgb_a_hsl(c: Rgb) -> (f32, f32, f32) {
    let r = c.r as f32 / 255.0;
    let g = c.g as f32 / 255.0;
    let b = c.b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_a_rgb(h: f32, s: f32, l: f32) -> Rgb {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return Rgb { r: v, g: v, b: v };
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let canal = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    Rgb { r: canal(h + 1.0 / 3.0), g: canal(h), b: canal(h - 1.0 / 3.0) }
}

/// Variaciones de luminosidad de los 7 primeros tonos de `AccentPalette`.
pub const PASOS_PALETA: [f32; 7] = [0.30, 0.20, 0.10, 0.0, -0.10, -0.20, -0.30];

/// `AccentPalette`: 8 tonos de 4 bytes (R, G, B, 0). El 8.º es gris fijo.
pub fn paleta_acento(base: Rgb) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, d) in PASOS_PALETA.iter().enumerate() {
        let c = base.con_luz(*d);
        out[i * 4..i * 4 + 4].copy_from_slice(&[c.r, c.g, c.b, 0]);
    }
    out[28..32].copy_from_slice(&[0x69, 0x69, 0x69, 0]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cerca(a: Rgb, b: Rgb) -> bool {
        (a.r as i16 - b.r as i16).abs() <= 1
            && (a.g as i16 - b.g as i16).abs() <= 1
            && (a.b as i16 - b.b as i16).abs() <= 1
    }

    #[test]
    fn parsea_hex_con_y_sin_numeral() {
        assert_eq!(Rgb::desde_hex("#C9A227").unwrap(), Rgb { r: 0xC9, g: 0xA2, b: 0x27 });
        assert_eq!(Rgb::desde_hex("c9a227").unwrap(), Rgb { r: 0xC9, g: 0xA2, b: 0x27 });
    }

    #[test]
    fn rechaza_hex_invalido() {
        assert!(Rgb::desde_hex("#12345").is_err());
        assert!(Rgb::desde_hex("#GGGGGG").is_err());
        assert!(Rgb::desde_hex("").is_err());
    }

    #[test]
    fn a_hex_en_mayusculas() {
        assert_eq!(Rgb { r: 0xc9, g: 0xa2, b: 0x27 }.a_hex(), "#C9A227");
    }

    #[test]
    fn abgr_y_argb() {
        let c = Rgb { r: 0x11, g: 0x22, b: 0x33 };
        assert_eq!(c.abgr(), 0xFF33_2211);
        assert_eq!(c.argb(0xC4), 0xC411_2233);
    }

    #[test]
    fn con_luz_cero_no_cambia() {
        let c = Rgb::desde_hex("#C9A227").unwrap();
        assert!(cerca(c.con_luz(0.0), c));
    }

    #[test]
    fn con_luz_extremos() {
        let c = Rgb::desde_hex("#C9A227").unwrap();
        assert_eq!(c.con_luz(1.0), Rgb { r: 255, g: 255, b: 255 });
        assert_eq!(c.con_luz(-1.0), Rgb { r: 0, g: 0, b: 0 });
    }

    #[test]
    fn paleta_tiene_8_tonos_con_la_base_en_el_medio() {
        let base = Rgb::desde_hex("#C9A227").unwrap();
        let p = paleta_acento(base);
        assert_eq!(p.len(), 32);
        assert!(cerca(Rgb { r: p[12], g: p[13], b: p[14] }, base));
        assert_eq!(&p[28..32], &[0x69, 0x69, 0x69, 0x00]);
        let suma = |i: usize| p[i] as u32 + p[i + 1] as u32 + p[i + 2] as u32;
        assert!(suma(0) > suma(12), "el primer tono es más claro que la base");
        assert!(suma(24) < suma(12), "el séptimo tono es más oscuro que la base");
        for i in 0..8 {
            assert_eq!(p[i * 4 + 3], 0, "el cuarto byte de cada tono es 0");
        }
    }
}
