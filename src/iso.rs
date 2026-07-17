/// Isometric (2:1 diamond) projection between world tile-space and
/// screen pixel-space. World space is flat tile units (can be
/// fractional); screen space is pixels, y-down.
pub struct IsoProj {
    pub tile_w: f32,
    pub tile_h: f32,
}

impl IsoProj {
    /// World (tile units) -> screen (pixels).
    pub fn world_to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        let sx = (wx - wy) * self.tile_w / 2.0;
        let sy = (wx + wy) * self.tile_h / 2.0;
        (sx, sy)
    }

    /// Screen (pixels) -> world (tile units). Exact inverse of
    /// `world_to_screen`.
    pub fn screen_to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        let wx = sx / self.tile_w + sy / self.tile_h;
        let wy = sy / self.tile_h - sx / self.tile_w;
        (wx, wy)
    }

    /// Screen (pixels) -> tile coordinates (floor of world position).
    pub fn screen_to_tile(&self, sx: f32, sy: f32) -> (i32, i32) {
        let (wx, wy) = self.screen_to_world(sx, sy);
        (wx.floor() as i32, wy.floor() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj() -> IsoProj {
        IsoProj { tile_w: 64.0, tile_h: 32.0 }
    }

    #[test]
    fn origin_maps_to_origin() {
        let p = proj();
        assert_eq!(p.world_to_screen(0.0, 0.0), (0.0, 0.0));
        assert_eq!(p.screen_to_world(0.0, 0.0), (0.0, 0.0));
        assert_eq!(p.screen_to_tile(0.0, 0.0), (0, 0));
    }

    #[test]
    fn world_to_screen_known_points() {
        let p = proj();
        assert_eq!(p.world_to_screen(1.0, 0.0), (32.0, 16.0));
        assert_eq!(p.world_to_screen(0.0, 1.0), (-32.0, 16.0));
        assert_eq!(p.world_to_screen(2.0, 3.0), (-32.0, 80.0));
    }

    #[test]
    fn round_trip_several_points() {
        let p = proj();
        let pts = [
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 3.0),
            (-1.5, 2.25),
            (-4.0, -4.0),
            (7.75, -2.5),
        ];
        for &(wx, wy) in &pts {
            let (sx, sy) = p.world_to_screen(wx, wy);
            let (rwx, rwy) = p.screen_to_world(sx, sy);
            assert!((rwx - wx).abs() < 1e-4, "wx round trip: {} vs {}", rwx, wx);
            assert!((rwy - wy).abs() < 1e-4, "wy round trip: {} vs {}", rwy, wy);
        }
    }

    #[test]
    fn screen_to_tile_on_exact_corner_belongs_to_upper_tile() {
        // world (2.0, 2.0) is exactly the shared corner of 4 tiles;
        // floor convention assigns it to tile (2, 2), the tile whose
        // min corner it is.
        let p = proj();
        let (sx, sy) = p.world_to_screen(2.0, 2.0);
        assert_eq!(p.screen_to_tile(sx, sy), (2, 2));
    }

    #[test]
    fn screen_to_tile_mid_tile() {
        let p = proj();
        let (sx, sy) = p.world_to_screen(1.5, 1.5);
        assert_eq!(p.screen_to_tile(sx, sy), (1, 1));
    }

    #[test]
    fn screen_to_tile_negative_floors_toward_negative_infinity() {
        let p = proj();
        let (sx, sy) = p.world_to_screen(-0.5, -0.5);
        assert_eq!(p.screen_to_tile(sx, sy), (-1, -1));
    }
}
