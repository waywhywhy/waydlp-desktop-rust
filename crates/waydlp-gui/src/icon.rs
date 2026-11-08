//! Procedurally rasterised application icon (same geometry as
//! `assets/icons/waydlp.svg`), so the binary needs no image files at runtime.

pub fn app_icon(size: u32) -> egui::IconData {
    egui::IconData { rgba: rasterize(size), width: size, height: size }
}

fn rounded_rect_sdf(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let hx = (x1 - x0) / 2.0 - r;
    let hy = (y1 - y0) / 2.0 - r;
    let dx = (px - cx).abs() - hx;
    let dy = (py - cy).abs() - hy;
    let ox = dx.max(0.0);
    let oy = dy.max(0.0);
    (ox * ox + oy * oy).sqrt() + dx.max(dy).min(0.0) - r
}

fn segment_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let (wx, wy) = (px - a.0, py - a.1);
    let t = ((wx * vx + wy * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    let (dx, dy) = (px - (a.0 + t * vx), py - (a.1 + t * vy));
    (dx * dx + dy * dy).sqrt()
}

fn blend(dst: &mut [f32; 4], src: [f32; 3], alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    for i in 0..3 {
        dst[i] = dst[i] * (1.0 - a) + src[i] * a;
    }
    dst[3] = dst[3] + a * (1.0 - dst[3]);
}

/// Render the icon in a 512-unit design space scaled to `size`.
pub fn rasterize(size: u32) -> Vec<u8> {
    let s = 512.0 / size as f32;
    let hex = |h: u32| [((h >> 16) & 0xff) as f32, ((h >> 8) & 0xff) as f32, (h & 0xff) as f32];
    let (light, dark) = (hex(0xB56BFF), hex(0x5A189A));
    let accent = hex(0x9D4EDD);
    let accent_hover = hex(0xB56BFF);
    let wave = [(128.0, 176.0), (192.0, 296.0), (256.0, 206.0), (320.0, 296.0), (384.0, 176.0)];
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = ((x as f32 + 0.5) * s, (y as f32 + 0.5) * s);
            let aa = s; // one pixel in design units
            let mut c = [0.0f32, 0.0, 0.0, 0.0];
            // Outer gradient rounded square.
            let d_outer = rounded_rect_sdf(px, py, 16.0, 16.0, 496.0, 496.0, 120.0);
            let t = ((px + py) / 1024.0).clamp(0.0, 1.0);
            let grad = [
                light[0] + (dark[0] - light[0]) * t,
                light[1] + (dark[1] - light[1]) * t,
                light[2] + (dark[2] - light[2]) * t,
            ];
            blend(&mut c, grad, 0.5 - d_outer / aa);
            // Inner black.
            let d_inner = rounded_rect_sdf(px, py, 44.0, 44.0, 468.0, 468.0, 96.0);
            blend(&mut c, [0.0, 0.0, 0.0], 0.5 - d_inner / aa);
            // Wave polyline (stroke 44, round caps/joins).
            let dw = wave.windows(2).map(|w| segment_dist(px, py, w[0], w[1])).fold(f32::MAX, f32::min) - 22.0;
            let cov = c[3].min(1.0);
            blend(&mut c, accent, (0.5 - dw / aa) * cov);
            // Base line.
            let dl = segment_dist(px, py, (150.0, 376.0), (362.0, 376.0)) - 16.0;
            let cov = c[3].min(1.0);
            blend(&mut c, accent_hover, (0.5 - dl / aa) * cov);
            out.extend_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, (c[3] * 255.0).round() as u8]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_has_expected_size_and_transparent_corner() {
        let px = super::rasterize(64);
        assert_eq!(px.len(), 64 * 64 * 4);
        assert_eq!(px[3], 0, "top-left corner is transparent");
        let center = ((32 * 64 + 32) * 4) as usize;
        assert_eq!(px[center + 3], 255, "center is opaque");
    }
}
