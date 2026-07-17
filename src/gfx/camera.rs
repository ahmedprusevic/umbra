/// 2D camera over world-pixel space. `zoom` 1.0 = 1 world px per screen px.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub center: (f32, f32),
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { center: (0.0, 0.0), zoom: 1.0 }
    }
}

impl Camera {
    /// Column-major orthographic view-projection (world px → NDC), ready to
    /// upload as a WGSL `mat4x4<f32>`.
    pub fn view_proj(&self, (vw, vh): (u32, u32)) -> [[f32; 4]; 4] {
        let a = 2.0 * self.zoom / vw as f32;
        let b = -2.0 * self.zoom / vh as f32; // world y-down → NDC y-up
        let tx = -a * self.center.0;
        let ty = -b * self.center.1;
        [
            [a, 0.0, 0.0, 0.0],
            [0.0, b, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [tx, ty, 0.0, 1.0],
        ]
    }

    pub fn screen_to_world(&self, (sx, sy): (f32, f32), (vw, vh): (u32, u32)) -> (f32, f32) {
        (
            (sx - vw as f32 / 2.0) / self.zoom + self.center.0,
            (sy - vh as f32 / 2.0) / self.zoom + self.center.1,
        )
    }

    pub fn world_to_screen(&self, (wx, wy): (f32, f32), (vw, vh): (u32, u32)) -> (f32, f32) {
        (
            (wx - self.center.0) * self.zoom + vw as f32 / 2.0,
            (wy - self.center.1) * self.zoom + vh as f32 / 2.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mul(m: [[f32; 4]; 4], p: (f32, f32)) -> (f32, f32) {
        // column-major: out = M * [x, y, 0, 1]
        (
            m[0][0] * p.0 + m[1][0] * p.1 + m[3][0],
            m[0][1] * p.0 + m[1][1] * p.1 + m[3][1],
        )
    }

    #[test]
    fn center_maps_to_ndc_origin() {
        let cam = Camera { center: (400.0, 300.0), zoom: 1.0 };
        let m = cam.view_proj((800, 600));
        let ndc = mul(m, (400.0, 300.0));
        assert!(ndc.0.abs() < 1e-6 && ndc.1.abs() < 1e-6);
    }

    #[test]
    fn viewport_edges_map_to_unit_ndc() {
        let cam = Camera { center: (0.0, 0.0), zoom: 1.0 };
        let m = cam.view_proj((800, 600));
        let right = mul(m, (400.0, 0.0));
        assert!((right.0 - 1.0).abs() < 1e-6);
        let bottom = mul(m, (0.0, 300.0));
        assert!((bottom.1 + 1.0).abs() < 1e-6, "world y-down maps to NDC y-up");
    }

    #[test]
    fn zoom_scales_ndc() {
        let cam = Camera { center: (0.0, 0.0), zoom: 2.0 };
        let m = cam.view_proj((800, 600));
        let p = mul(m, (100.0, 0.0));
        assert!((p.0 - 0.5).abs() < 1e-6); // 2x zoom: 100px spans half the 400px half-width
    }

    #[test]
    fn screen_world_roundtrip() {
        let cam = Camera { center: (250.0, -80.0), zoom: 1.5 };
        let w = cam.screen_to_world((123.0, 456.0), (800, 600));
        let s = cam.world_to_screen(w, (800, 600));
        assert!((s.0 - 123.0).abs() < 1e-3 && (s.1 - 456.0).abs() < 1e-3);
    }
}
