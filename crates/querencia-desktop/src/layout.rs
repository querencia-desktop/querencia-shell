//! Free icon positions on the desktop (HMP-10), the parts without a display.
//!
//! Every icon occupies one tile (`Geometry::tile`, icon plus at most two
//! lines of label) and may sit anywhere inside the usable area, the icon
//! layer between the panels, keeping `margin` pixels to its edges. Tiles
//! never overlap: a tile dropped onto another one moves to the nearest free
//! spot.
//!
//! A position is remembered as a [`Place`]: for each axis an edge (start,
//! centre, end) and the distance to it. Icons the user put near the right
//! or bottom edge stay there when the area changes size (another
//! resolution, scale factor or panel height); icons placed automatically or
//! by "Nach Namen aufräumen" hang from the top left corner. Whatever is
//! remembered, an icon is always shown inside the area (`clamp`), and the
//! remembered place is not changed by that: when the area grows back, the
//! icon returns to it.

/// The edge an axis position is measured from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Center,
    End,
}

/// One axis of a remembered position: the edge and the distance from it
/// (for `Center`: from the centred position, may be negative).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub anchor: Anchor,
    pub offset: i32,
}

/// A remembered position of a tile's top left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub x: Axis,
    pub y: Axis,
}

/// A tile's top left corner in the area, in logical pixels.
pub type Point = (i32, i32);

/// The usable area and the tile size, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub width: i32,
    pub height: i32,
    pub tile: (i32, i32),
    pub margin: i32,
}

fn resolve_axis(axis: Axis, length: i32, tile: i32) -> i32 {
    match axis.anchor {
        Anchor::Start => axis.offset,
        Anchor::End => length - tile - axis.offset,
        Anchor::Center => (length - tile) / 2 + axis.offset,
    }
}

/// The edge nearest to a tile at `start`: the outer thirds of the area
/// belong to their edge, the middle third to the centre.
fn free_axis(start: i32, length: i32, tile: i32) -> Axis {
    let centre = start + tile / 2;
    if centre * 3 < length {
        Axis {
            anchor: Anchor::Start,
            offset: start,
        }
    } else if centre * 3 > length * 2 {
        Axis {
            anchor: Anchor::End,
            offset: length - tile - start,
        }
    } else {
        Axis {
            anchor: Anchor::Center,
            offset: start - (length - tile) / 2,
        }
    }
}

impl Geometry {
    pub fn known(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    fn max(&self) -> Point {
        (
            (self.width - self.margin - self.tile.0).max(self.margin),
            (self.height - self.margin - self.tile.1).max(self.margin),
        )
    }

    /// The nearest position at which the whole tile is inside the area.
    pub fn clamp(&self, (x, y): Point) -> Point {
        let (max_x, max_y) = self.max();
        (x.clamp(self.margin, max_x), y.clamp(self.margin, max_y))
    }

    /// Where `place` is in this area (possibly outside; see `clamp`).
    pub fn resolve(&self, place: Place) -> Point {
        (
            resolve_axis(place.x, self.width, self.tile.0),
            resolve_axis(place.y, self.height, self.tile.1),
        )
    }

    /// A place the user chose by dragging: measured from the nearest edge.
    pub fn free_place(&self, (x, y): Point) -> Place {
        Place {
            x: free_axis(x, self.width, self.tile.0),
            y: free_axis(y, self.height, self.tile.1),
        }
    }

    /// A place found automatically: measured from the top left corner.
    pub fn top_left_place(&self, (x, y): Point) -> Place {
        Place {
            x: Axis {
                anchor: Anchor::Start,
                offset: x,
            },
            y: Axis {
                anchor: Anchor::Start,
                offset: y,
            },
        }
    }

    /// Whether tiles at `a` and `b` overlap.
    pub fn overlaps(&self, a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < self.tile.0 && (a.1 - b.1).abs() < self.tile.1
    }

    fn is_free(&self, p: Point, taken: &[Point]) -> bool {
        taken.iter().all(|&t| !self.overlaps(p, t))
    }

    /// The free position inside the area nearest to `want` (which is
    /// clamped first); `None` when the area is full.
    ///
    /// The nearest free spot either keeps a coordinate of `want`, touches
    /// a taken tile or lies on the area's edge; so only those coordinates
    /// are tried, and a moved tile ends up flush with its neighbour.
    pub fn nearest_free(&self, want: Point, taken: &[Point]) -> Option<Point> {
        let want = self.clamp(want);
        if self.is_free(want, taken) {
            return Some(want);
        }
        let (max_x, max_y) = self.max();
        let mut xs = vec![want.0, self.margin, max_x];
        let mut ys = vec![want.1, self.margin, max_y];
        for &(x, y) in taken {
            xs.extend([x - self.tile.0, x + self.tile.0]);
            ys.extend([y - self.tile.1, y + self.tile.1]);
        }
        let inside = |v: &i32, low: i32, high: i32| (low..=high).contains(v);
        xs.retain(|x| inside(x, self.margin, max_x));
        ys.retain(|y| inside(y, self.margin, max_y));
        xs.sort_unstable();
        xs.dedup();
        ys.sort_unstable();
        ys.dedup();
        let distance = |p: Point| {
            let (dx, dy) = (i64::from(p.0 - want.0), i64::from(p.1 - want.1));
            dx * dx + dy * dy
        };
        let mut best: Option<(i64, i32, i32)> = None;
        for &y in &ys {
            for &x in &xs {
                // ties: the upper, then the left one (deterministic)
                let key = (distance((x, y)), y, x);
                if best.is_none_or(|b| key < b) && self.is_free((x, y), taken) {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, y, x)| (x, y))
    }

    /// The first free cell of the column grid (columns from the left, each
    /// from the top), where GNOME 2 put new icons.
    pub fn grid_free(&self, taken: &[Point]) -> Option<Point> {
        let (max_x, max_y) = self.max();
        let mut x = self.margin;
        while x <= max_x {
            let mut y = self.margin;
            while y <= max_y {
                if self.is_free((x, y), taken) {
                    return Some((x, y));
                }
                y += self.tile.1;
            }
            x += self.tile.0;
        }
        None
    }

    /// A position for a new icon: the first free grid cell, else any free
    /// spot, else (a full desktop) the first cell.
    pub fn new_position(&self, taken: &[Point]) -> Point {
        self.grid_free(taken)
            .or_else(|| self.nearest_free((self.margin, self.margin), taken))
            .unwrap_or((self.margin, self.margin))
    }

    /// Positions for all icons, in the given order. `wanted[i]` is icon
    /// i's remembered place, `None` for an icon without one (it gets a new
    /// position, see `new_position`). Icons whose place lies inside the
    /// area come first and keep it; then those that had to be moved into
    /// the area; then the new ones. A taken spot moves an icon to the
    /// nearest free one.
    pub fn place_all(&self, wanted: &[Option<Place>]) -> Vec<Point> {
        let mut result: Vec<Option<Point>> = vec![None; wanted.len()];
        let mut taken: Vec<Point> = Vec::with_capacity(wanted.len());
        let resolved: Vec<Option<(Point, bool)>> = wanted
            .iter()
            .map(|place| {
                place.map(|place| {
                    let at = self.resolve(place);
                    (self.clamp(at), self.clamp(at) == at)
                })
            })
            .collect();
        for inside in [true, false] {
            for (i, entry) in resolved.iter().enumerate() {
                if let Some((at, fits)) = *entry
                    && fits == inside
                {
                    let at = self.nearest_free(at, &taken).unwrap_or(at);
                    taken.push(at);
                    result[i] = Some(at);
                }
            }
        }
        for slot in &mut result {
            if slot.is_none() {
                let at = self.new_position(&taken);
                taken.push(at);
                *slot = Some(at);
            }
        }
        result.into_iter().map(Option::unwrap_or_default).collect()
    }

    /// "Nach Namen aufräumen": `count` positions in grid order.
    pub fn arranged(&self, count: usize) -> Vec<Point> {
        let mut taken = Vec::with_capacity(count);
        for _ in 0..count {
            let at = self.new_position(&taken);
            taken.push(at);
        }
        taken
    }

    /// The tile containing `point`, if any, among `positions`.
    pub fn hit(&self, positions: &[Point], point: (f64, f64)) -> Option<usize> {
        positions.iter().position(|&(x, y)| {
            point.0 >= f64::from(x)
                && point.0 < f64::from(x + self.tile.0)
                && point.1 >= f64::from(y)
                && point.1 < f64::from(y + self.tile.1)
        })
    }
}

/// Whether a press-and-move of `(dx, dy)` is a drag (GTK's drag threshold,
/// `gtk-dnd-drag-threshold`, 8 px by default) rather than a click.
pub fn is_drag(dx: f64, dy: f64, threshold: i32) -> bool {
    dx * dx + dy * dy > f64::from(threshold.max(1)).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Geometry = Geometry {
        width: 1280,
        height: 672,
        tile: (100, 88),
        margin: 8,
    };

    fn sized(width: i32, height: i32) -> Geometry {
        Geometry {
            width,
            height,
            ..AREA
        }
    }

    #[test]
    fn new_icons_fill_columns_from_the_top_left_like_hmp_09() {
        // the same cells as HMP-09's grid (margin 8, cell 100×88, 7 rows)
        let positions = AREA.arranged(9);
        assert_eq!(positions[0], (8, 8));
        assert_eq!(positions[6], (8, 8 + 6 * 88));
        assert_eq!(positions[7], (108, 8));
        assert_eq!(positions[8], (108, 96));
    }

    #[test]
    fn a_new_icon_takes_the_first_gap_and_moves_nobody() {
        let taken = [(8, 8), (8, 184), (1172, 8)];
        assert_eq!(AREA.new_position(&taken), (8, 96));
        let mut all = taken.to_vec();
        all.push(AREA.new_position(&taken));
        assert_eq!(&all[..3], &taken, "vorhandene Symbole bleiben, wo sie sind");
    }

    #[test]
    fn positions_stay_inside_the_area_and_off_the_panels() {
        // the area is the space between the panels: 0..width × 0..height
        assert_eq!(AREA.clamp((-50, -50)), (8, 8));
        assert_eq!(AREA.clamp((5000, 5000)), (1280 - 8 - 100, 672 - 8 - 88));
        // a very small area still keeps the margin at the top left
        assert_eq!(sized(90, 60).clamp((40, 40)), (8, 8));
    }

    #[test]
    fn corners_are_remembered_from_their_edge() {
        let top_right = AREA.clamp((5000, 0));
        let place = AREA.free_place(top_right);
        assert_eq!(place.x.anchor, Anchor::End);
        assert_eq!(place.y.anchor, Anchor::Start);
        // another resolution: still top right
        let big = sized(1920, 1032);
        assert_eq!(big.resolve(place), (1920 - 8 - 100, 8));
        let bottom_right = AREA.clamp((5000, 5000));
        let place = AREA.free_place(bottom_right);
        assert_eq!(big.resolve(place), (1920 - 108, 1032 - 96));
        // the middle stays in the middle
        let middle = (590, 292);
        let place = AREA.free_place(middle);
        assert_eq!(
            (place.x.anchor, place.y.anchor),
            (Anchor::Center, Anchor::Center)
        );
        assert_eq!(AREA.resolve(place), middle);
        assert_eq!(big.resolve(place), ((1920 - 100) / 2, (1032 - 88) / 2));
        // automatic places hang from the top left
        let place = AREA.top_left_place((108, 360));
        assert_eq!(big.resolve(place), (108, 360));
    }

    #[test]
    fn a_taken_spot_moves_the_icon_to_the_nearest_free_one() {
        let taken = [(500, 300)];
        assert_eq!(AREA.nearest_free((700, 300), &taken), Some((700, 300)));
        let free = AREA.nearest_free((510, 310), &taken).expect("frei");
        assert!(!AREA.overlaps(free, taken[0]));
        // flush beside it: the shortest way out is downwards (78 px)
        assert_eq!(free, (510, 300 + 88));
        // at the top edge the way up is closed
        let taken = [(8, 8)];
        let free = AREA.nearest_free((8, 8), &taken).expect("frei");
        assert_eq!(free, (8, 96));
    }

    #[test]
    fn a_full_desktop_has_no_free_spot() {
        let small = sized(216, 192);
        let full = small.arranged(4);
        assert_eq!(full, [(8, 8), (8, 96), (108, 8), (108, 96)]);
        assert_eq!(small.nearest_free((50, 50), &full), None);
        assert_eq!(small.grid_free(&full), None);
    }

    #[test]
    fn smaller_screens_pull_icons_in_without_overlap() {
        let big = sized(1920, 1032);
        let wanted = [
            Some(big.free_place((1812, 8))),     // rechts oben
            Some(big.top_left_place((1400, 8))), // weit rechts, links verankert
            Some(big.top_left_place((8, 8))),
            None,
        ];
        let small = sized(1024, 672);
        let positions = small.place_all(&wanted);
        assert_eq!(
            positions[0],
            (1024 - 108, 8),
            "rechts oben bleibt rechts oben"
        );
        assert_eq!(positions[2], (8, 8));
        for (i, a) in positions.iter().enumerate() {
            assert_eq!(small.clamp(*a), *a, "sichtbar: {a:?}");
            for b in &positions[i + 1..] {
                assert!(!small.overlaps(*a, *b), "{a:?} überlappt {b:?}");
            }
        }
        // the remembered places are untouched: back on the big screen
        let again = big.place_all(&wanted);
        assert_eq!(again[1], (1400, 8));
    }

    #[test]
    fn icons_inside_the_area_keep_their_spot_against_pulled_in_ones() {
        let big = sized(1920, 1032);
        let small = sized(1024, 672);
        let wanted = [
            Some(big.top_left_place((1600, 8))), // must be pulled in …
            Some(small.free_place((916, 8))),    // … onto this one's spot
        ];
        let positions = small.place_all(&wanted);
        assert_eq!(positions[1], (916, 8), "sitzt schon richtig, bleibt");
        assert_ne!(positions[0], (916, 8));
    }

    #[test]
    fn hits_are_found_by_tile() {
        let positions = [(8, 8), (500, 300)];
        assert_eq!(AREA.hit(&positions, (10.0, 10.0)), Some(0));
        assert_eq!(AREA.hit(&positions, (599.0, 387.0)), Some(1));
        assert_eq!(AREA.hit(&positions, (600.0, 300.0)), None);
        assert_eq!(AREA.hit(&positions, (4.0, 4.0)), None);
    }

    #[test]
    fn small_movements_are_clicks() {
        assert!(!is_drag(0.0, 0.0, 8));
        assert!(!is_drag(5.0, 6.0, 8));
        assert!(is_drag(6.0, 6.0, 8));
        assert!(is_drag(0.0, -9.0, 8));
        assert!(is_drag(2.0, 0.0, 0), "Schwelle mindestens 1 px");
    }
}
