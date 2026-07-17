/// An axis-aligned screen-space rectangle, always stored normalized
/// (`min` <= `max` on both axes).
pub struct Rect {
    pub min: (f32, f32),
    pub max: (f32, f32),
}

impl Rect {
    /// Builds a normalized rect from two corners given in any order.
    pub fn from_corners(a: (f32, f32), b: (f32, f32)) -> Rect {
        Rect {
            min: (a.0.min(b.0), a.1.min(b.1)),
            max: (a.0.max(b.0), a.1.max(b.1)),
        }
    }

    fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.min.0 && p.0 <= self.max.0 && p.1 >= self.min.1 && p.1 <= self.max.1
    }
}

/// Picks the nearest item whose screen distance from `point` is within
/// its own pick radius. Ties (equal distance) are broken by greater
/// screen-y first (drawn in front in isometric view), then by smaller
/// id, so the result is deterministic regardless of input order.
pub fn pick<I>(point: (f32, f32), items: I) -> Option<u32>
where
    I: IntoIterator<Item = (u32, (f32, f32), f32)>,
{
    let mut best: Option<(f32, (f32, f32), u32)> = None;
    for (id, center, radius) in items {
        let dx = center.0 - point.0;
        let dy = center.1 - point.1;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > radius {
            continue;
        }
        let candidate = (dist, center, id);
        best = Some(match best {
            None => candidate,
            Some(cur) => {
                if candidate.0 < cur.0 {
                    candidate
                } else if candidate.0 > cur.0 {
                    cur
                } else if candidate.1 .1 > cur.1 .1 {
                    candidate
                } else if candidate.1 .1 < cur.1 .1 {
                    cur
                } else if candidate.2 < cur.2 {
                    candidate
                } else {
                    cur
                }
            }
        });
    }
    best.map(|(_, _, id)| id)
}

/// Returns the ids of all items whose center lies inside `rect`
/// (inclusive of the boundary), sorted by id.
pub fn drag_box<I>(rect: Rect, items: I) -> Vec<u32>
where
    I: IntoIterator<Item = (u32, (f32, f32), f32)>,
{
    let mut out: Vec<u32> = items
        .into_iter()
        .filter(|(_, center, _)| rect.contains(*center))
        .map(|(id, _, _)| id)
        .collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_from_corners_normalizes_any_order() {
        let r = Rect::from_corners((10.0, 10.0), (0.0, 0.0));
        assert_eq!(r.min, (0.0, 0.0));
        assert_eq!(r.max, (10.0, 10.0));

        let r = Rect::from_corners((0.0, 10.0), (10.0, 0.0));
        assert_eq!(r.min, (0.0, 0.0));
        assert_eq!(r.max, (10.0, 10.0));
    }

    #[test]
    fn pick_nearest_within_radius() {
        let items = vec![
            (1u32, (0.0, 0.0), 25.0),
            (2u32, (20.0, 0.0), 5.0),
        ];
        // point is within both items' radii (dist 18 vs dist 2), but
        // item 2 is nearer -- nearest wins.
        assert_eq!(pick((18.0, 0.0), items), Some(2));
    }

    #[test]
    fn pick_nothing_in_radius_returns_none() {
        let items = vec![(1u32, (0.0, 0.0), 1.0)];
        assert_eq!(pick((100.0, 100.0), items), None);
    }

    #[test]
    fn pick_tie_breaks_by_greater_screen_y_then_smaller_id() {
        // Both items are exactly distance 5.0 from the pick point.
        let items = vec![
            (7u32, (4.0, -3.0), 10.0), // screen-y = -3
            (5u32, (3.0, 4.0), 10.0),  // screen-y = 4 (drawn in front)
        ];
        assert_eq!(pick((0.0, 0.0), items), Some(5));
    }

    #[test]
    fn pick_tie_same_y_breaks_by_smaller_id() {
        let items = vec![
            (2u32, (10.0, 10.0), 5.0),
            (1u32, (10.0, 10.0), 5.0),
        ];
        assert_eq!(pick((10.0, 10.0), items), Some(1));
    }

    #[test]
    fn drag_box_selects_centers_inside_inclusive() {
        let rect = Rect::from_corners((0.0, 0.0), (10.0, 10.0));
        let items = vec![
            (3u32, (0.0, 0.0), 1.0),   // on min corner: included
            (1u32, (10.0, 10.0), 1.0), // on max corner: included
            (2u32, (5.0, 5.0), 1.0),   // interior: included
            (4u32, (11.0, 5.0), 1.0),  // just outside: excluded
            (5u32, (5.0, -0.1), 1.0),  // just outside: excluded
        ];
        assert_eq!(drag_box(rect, items), vec![1, 2, 3]);
    }

    #[test]
    fn drag_box_normalizes_reversed_corners() {
        let rect = Rect::from_corners((10.0, 10.0), (0.0, 0.0));
        let items = vec![(1u32, (5.0, 5.0), 1.0)];
        assert_eq!(drag_box(rect, items), vec![1]);
    }
}
