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
//!
//! HMP-10a: with `snap` (the default, "Am Raster ausrichten"), every icon
//! sits on a cell of an invisible grid. The grid belongs to the area, not
//! to the screen: as many columns and rows of whole tiles as fit, spread
//! evenly so that the first and the last one keep the margin to their edge
//! (uniform spacing, never closer than a tile). Remembered places stay as
//! they are (pixels from an edge); they are shown on the nearest cell, so
//! older positions are never lost and the grid follows any area size.
//! Several icons move as a group by whole cells and keep their arrangement.

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
    /// HMP-10a: icons on the grid ("Am Raster ausrichten").
    pub snap: bool,
}

/// A grid cell: column and row, from the top left.
pub type Cell = (i32, i32);

/// Cells along one axis: count and pitch (spread evenly, at least `tile`).
fn grid_axis(length: i32, tile: i32, margin: i32) -> (i32, f64) {
    let room = length - 2 * margin;
    let count = (room / tile.max(1)).max(1);
    let pitch = if count > 1 {
        f64::from(room - tile) / f64::from(count - 1)
    } else {
        f64::from(tile)
    };
    (count, pitch)
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

    /// Columns and rows of the grid.
    pub fn cells(&self) -> Cell {
        (
            grid_axis(self.width, self.tile.0, self.margin).0,
            grid_axis(self.height, self.tile.1, self.margin).0,
        )
    }

    /// The top left corner of a grid cell.
    pub fn cell_point(&self, (column, row): Cell) -> Point {
        let (_, px) = grid_axis(self.width, self.tile.0, self.margin);
        let (_, py) = grid_axis(self.height, self.tile.1, self.margin);
        (
            self.margin + (f64::from(column) * px).round() as i32,
            self.margin + (f64::from(row) * py).round() as i32,
        )
    }

    /// The grid cell nearest to `point`.
    pub fn cell_of(&self, (x, y): Point) -> Cell {
        let (columns, px) = grid_axis(self.width, self.tile.0, self.margin);
        let (rows, py) = grid_axis(self.height, self.tile.1, self.margin);
        let index = |v: i32, pitch: f64, count: i32| {
            ((f64::from(v - self.margin) / pitch).round() as i32).clamp(0, count - 1)
        };
        (index(x, px, columns), index(y, py, rows))
    }

    /// `point` on the grid: the corner of its nearest cell.
    pub fn snapped(&self, point: Point) -> Point {
        self.cell_point(self.cell_of(point))
    }

    /// A place with the anchors of `like` for the position `at` (used when
    /// icons are put on the grid: their edge stays, the distance changes).
    pub fn place_like(&self, like: Place, (x, y): Point) -> Place {
        let offset = |anchor: Anchor, v: i32, length: i32, tile: i32| match anchor {
            Anchor::Start => v,
            Anchor::End => length - tile - v,
            Anchor::Center => v - (length - tile) / 2,
        };
        Place {
            x: Axis {
                anchor: like.x.anchor,
                offset: offset(like.x.anchor, x, self.width, self.tile.0),
            },
            y: Axis {
                anchor: like.y.anchor,
                offset: offset(like.y.anchor, y, self.height, self.tile.1),
            },
        }
    }

    /// The free grid cell nearest to `want` (snap mode).
    fn nearest_free_cell(&self, want: Point, taken: &[Point]) -> Option<Point> {
        let want = self.clamp(want);
        let (columns, rows) = self.cells();
        let mut best: Option<(i64, i32, i32)> = None;
        for row in 0..rows {
            for column in 0..columns {
                let (x, y) = self.cell_point((column, row));
                let (dx, dy) = (i64::from(x - want.0), i64::from(y - want.1));
                let key = (dx * dx + dy * dy, y, x);
                if best.is_none_or(|b| key < b) && self.is_free((x, y), taken) {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, y, x)| (x, y))
    }

    /// The free position inside the area nearest to `want` (which is
    /// clamped first); `None` when the area is full. With `snap`, the
    /// nearest free grid cell.
    ///
    /// The nearest free spot either keeps a coordinate of `want`, touches
    /// a taken tile or lies on the area's edge; so only those coordinates
    /// are tried, and a moved tile ends up flush with its neighbour.
    pub fn nearest_free(&self, want: Point, taken: &[Point]) -> Option<Point> {
        if self.snap {
            let cell = self.snapped(self.clamp(want));
            if self.is_free(cell, taken) {
                return Some(cell);
            }
            return self.nearest_free_cell(want, taken);
        }
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
        if self.snap {
            let (columns, rows) = self.cells();
            return (0..columns)
                .flat_map(|column| (0..rows).map(move |row| (column, row)))
                .map(|cell| self.cell_point(cell))
                .find(|&p| self.is_free(p, taken));
        }
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

    /// QE-03: a spot for a newly mounted volume: the first free one below
    /// `anchor` (the trash) in its column, then in the next columns towards
    /// the middle, each from the top. Without an anchor or room there, like
    /// any new icon.
    pub fn below(&self, anchor: Option<Point>, taken: &[Point]) -> Point {
        let Some(anchor) = anchor else {
            return self.new_position(taken);
        };
        let found = if self.snap {
            let (columns, rows) = self.cells();
            let (column, row) = self.cell_of(anchor);
            let inward = if column * 2 >= columns - 1 { -1 } else { 1 };
            ((row + 1)..rows)
                .map(|r| (column, r))
                .chain(
                    (1..columns)
                        .map(|k| column + inward * k)
                        .filter(|c| (0..columns).contains(c))
                        .flat_map(|c| (0..rows).map(move |r| (c, r))),
                )
                .map(|cell| self.cell_point(cell))
                .find(|&p| self.is_free(p, taken))
        } else {
            let max_y = self.max().1;
            (1..)
                .map(|n| (anchor.0, anchor.1 + n * self.tile.1))
                .take_while(|p| p.1 <= max_y)
                .find(|&p| self.is_free(p, taken))
                .or_else(|| self.nearest_free((anchor.0, anchor.1 + self.tile.1), taken))
        };
        found.unwrap_or_else(|| self.new_position(taken))
    }

    /// QE-03: positions for the icons that are not remembered (volumes),
    /// after the others took `taken`: a place from this session where it
    /// is free (else the nearest free spot), the rest `below` the anchor.
    /// The icons in `taken` never move.
    pub fn place_transient(
        &self,
        taken: &[Point],
        wanted: &[Option<Place>],
        anchor: Option<Point>,
    ) -> Vec<Point> {
        let mut taken = taken.to_vec();
        let mut result = vec![(self.margin, self.margin); wanted.len()];
        for (i, place) in wanted.iter().enumerate() {
            if let Some(place) = place {
                let at = self.clamp(self.resolve(*place));
                let at = self.nearest_free(at, &taken).unwrap_or(at);
                taken.push(at);
                result[i] = at;
            }
        }
        for (i, place) in wanted.iter().enumerate() {
            if place.is_none() {
                let at = self.below(anchor, &taken);
                taken.push(at);
                result[i] = at;
            }
        }
        result
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

    /// HMP-10a: new positions for a group of icons moved together, the
    /// icon `lead` (the one under the pointer) as close to `want` as
    /// possible. The group keeps its arrangement (with `snap`: moves by
    /// whole cells), stays inside the area (stops at the edge) and never
    /// overlaps `others`; the nearest such move wins. `None` when no move
    /// keeps the arrangement (the caller then places icons one by one).
    pub fn place_group(
        &self,
        group: &[Point],
        lead: usize,
        want: Point,
        others: &[Point],
    ) -> Option<Vec<Point>> {
        let &start = group.get(lead)?;
        if self.snap {
            let cells: Vec<Cell> = group.iter().map(|&p| self.cell_of(p)).collect();
            // icons that share a cell here (a smaller area) cannot keep
            // their arrangement
            let mut distinct = cells.clone();
            distinct.sort_unstable();
            distinct.dedup();
            if distinct.len() != cells.len() {
                return None;
            }
            let (columns, rows) = self.cells();
            let range = |values: &mut dyn Iterator<Item = i32>, count: i32| {
                let values: Vec<i32> = values.collect();
                let low = -values.iter().min().copied().unwrap_or(0);
                let high = count - 1 - values.iter().max().copied().unwrap_or(0);
                (low, high)
            };
            let (low_c, high_c) = range(&mut cells.iter().map(|c| c.0), columns);
            let (low_r, high_r) = range(&mut cells.iter().map(|c| c.1), rows);
            if low_c > high_c || low_r > high_r {
                return None;
            }
            let target = self.cell_of(self.clamp(want));
            let goal = self.cell_point((
                (target.0 - cells[lead].0).clamp(low_c, high_c) + cells[lead].0,
                (target.1 - cells[lead].1).clamp(low_r, high_r) + cells[lead].1,
            ));
            let mut moves: Vec<(i64, Vec<Point>)> = Vec::new();
            for dr in low_r..=high_r {
                for dc in low_c..=high_c {
                    let moved: Vec<Point> = cells
                        .iter()
                        .map(|&(c, r)| self.cell_point((c + dc, r + dr)))
                        .collect();
                    let at = moved[lead];
                    let (dx, dy) = (i64::from(at.0 - goal.0), i64::from(at.1 - goal.1));
                    moves.push((dx * dx + dy * dy, moved));
                }
            }
            moves.sort_by_key(|(d, m)| (*d, m[lead].1, m[lead].0));
            return moves
                .into_iter()
                .map(|(_, m)| m)
                .find(|m| m.iter().all(|&p| self.is_free(p, others)));
        }
        // free placement: pixel moves inside the area
        let (max_x, max_y) = self.max();
        let min_gx = group.iter().map(|p| p.0).min()?;
        let max_gx = group.iter().map(|p| p.0).max()?;
        let min_gy = group.iter().map(|p| p.1).min()?;
        let max_gy = group.iter().map(|p| p.1).max()?;
        let (low_x, high_x) = (self.margin - min_gx, max_x - max_gx);
        let (low_y, high_y) = (self.margin - min_gy, max_y - max_gy);
        if low_x > high_x || low_y > high_y {
            return None;
        }
        let goal = (
            (want.0 - start.0).clamp(low_x, high_x),
            (want.1 - start.1).clamp(low_y, high_y),
        );
        // as in `nearest_free`: a coordinate of the goal, flush with a
        // neighbour or at the edge
        let mut dxs = vec![goal.0, low_x, high_x];
        let mut dys = vec![goal.1, low_y, high_y];
        for &o in others {
            for &g in group {
                dxs.extend([o.0 - self.tile.0 - g.0, o.0 + self.tile.0 - g.0]);
                dys.extend([o.1 - self.tile.1 - g.1, o.1 + self.tile.1 - g.1]);
            }
        }
        dxs.retain(|d| (low_x..=high_x).contains(d));
        dys.retain(|d| (low_y..=high_y).contains(d));
        dxs.sort_unstable();
        dxs.dedup();
        dys.sort_unstable();
        dys.dedup();
        let mut best: Option<(i64, i32, i32)> = None;
        for &dy in &dys {
            for &dx in &dxs {
                let (ex, ey) = (i64::from(dx - goal.0), i64::from(dy - goal.1));
                let key = (ex * ex + ey * ey, dy, dx);
                if best.is_none_or(|b| key < b)
                    && group
                        .iter()
                        .all(|&(x, y)| self.is_free((x + dx, y + dy), others))
                {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, dy, dx)| group.iter().map(|&(x, y)| (x + dx, y + dy)).collect())
    }

    /// HMP-10a: where a group dropped with its icon `lead` at `want`
    /// goes, or `None`: the move is refused and every icon stays where it
    /// is. First the group as a whole ([`Geometry::place_group`]); if its
    /// arrangement fits nowhere, each icon on its own nearest free spot,
    /// but only if every one of them finds one. Never onto another icon.
    pub fn move_group(
        &self,
        group: &[Point],
        lead: usize,
        want: Point,
        others: &[Point],
    ) -> Option<Vec<Point>> {
        if let Some(moved) = self.place_group(group, lead, want, others) {
            return Some(moved);
        }
        let &start = group.get(lead)?;
        let mut taken = others.to_vec();
        let mut moved = Vec::with_capacity(group.len());
        for &(x, y) in group {
            let wish = (x + want.0 - start.0, y + want.1 - start.1);
            let at = self.nearest_free(wish, &taken)?;
            taken.push(at);
            moved.push(at);
        }
        Some(moved)
    }

    /// HMP-10a: the icons whose tile meets the rectangle between the two
    /// corners `a` and `b` (any drag direction): the rubber band.
    pub fn in_rectangle(&self, positions: &[Point], a: (f64, f64), b: (f64, f64)) -> Vec<usize> {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        positions
            .iter()
            .enumerate()
            .filter(|(_, (x, y))| {
                f64::from(*x) <= x1
                    && f64::from(x + self.tile.0) >= x0
                    && f64::from(*y) <= y1
                    && f64::from(y + self.tile.1) >= y0
            })
            .map(|(i, _)| i)
            .collect()
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

/// HMP-10a: Umschalt+Klick: the icons from `from` to `to` in the desktop's
/// reading order (columns from the left, each from the top), both included.
pub fn range_between(positions: &[Point], from: usize, to: usize) -> Vec<usize> {
    let (Some(&a), Some(&b)) = (positions.get(from), positions.get(to)) else {
        return Vec::new();
    };
    let (low, high) = if a <= b { (a, b) } else { (b, a) };
    positions
        .iter()
        .enumerate()
        .filter(|(_, p)| **p >= low && **p <= high)
        .map(|(i, _)| i)
        .collect()
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
        snap: false,
    };

    const GRID: Geometry = Geometry {
        width: 1280,
        height: 656,
        tile: (100, 88),
        margin: 8,
        snap: true,
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

    #[test]
    fn the_grid_fills_the_area_evenly_from_edge_to_edge() {
        // 1280×656 between the panels: 12 columns, 7 rows
        assert_eq!(GRID.cells(), (12, 7));
        assert_eq!(GRID.cell_point((0, 0)), (8, 8));
        assert_eq!(GRID.cell_point((11, 6)), (1280 - 8 - 100, 656 - 8 - 88));
        // uniform spacing, never closer than a tile
        let xs: Vec<i32> = (0..12).map(|c| GRID.cell_point((c, 0)).0).collect();
        for pair in xs.windows(2) {
            assert!((105..=107).contains(&(pair[1] - pair[0])), "{xs:?}");
        }
        let ys: Vec<i32> = (0..7).map(|r| GRID.cell_point((0, r)).1).collect();
        assert!(ys.windows(2).all(|p| p[1] - p[0] == 92), "{ys:?}");
        // other sizes: the grid is the area's, not the screen's
        let big = Geometry {
            width: 1920,
            height: 1016,
            ..GRID
        };
        assert_eq!(big.cells(), (19, 11));
        assert_eq!(big.cell_point((18, 10)), (1920 - 108, 1016 - 96));
        let tiny = Geometry {
            width: 90,
            height: 60,
            ..GRID
        };
        assert_eq!(tiny.cells(), (1, 1));
        assert_eq!(tiny.snapped((40, 40)), (8, 8));
    }

    #[test]
    fn points_snap_to_the_nearest_cell() {
        assert_eq!(GRID.snapped((60, 50)), (8, 8));
        assert_eq!(GRID.snapped((61, 55)), GRID.cell_point((1, 1)));
        assert_eq!(GRID.snapped((-500, 9999)), GRID.cell_point((0, 6)));
        // the live case: Papierkorb at "rechts 26, oben 27"
        assert_eq!(GRID.cell_of((1280 - 100 - 26, 27)), (11, 0));
        // a remembered place resolved on another screen lands on a cell
        let place = GRID.free_place(GRID.cell_point((11, 6)));
        let big = Geometry {
            width: 1920,
            height: 1016,
            ..GRID
        };
        assert_eq!(big.snapped(big.resolve(place)), big.cell_point((18, 10)));
    }

    #[test]
    fn snapped_drops_take_the_nearest_free_cell() {
        let taken = [GRID.cell_point((3, 2))];
        // a free cell: the drop near it snaps there
        assert_eq!(
            GRID.nearest_free((330, 200), &[]),
            Some(GRID.cell_point((3, 2)))
        );
        let free = GRID
            .nearest_free(GRID.cell_point((3, 2)), &taken)
            .expect("frei");
        assert_ne!(free, taken[0]);
        assert_eq!(GRID.snapped(free), free, "auf dem Raster");
        let c = GRID.cell_of(free);
        assert_eq!((c.0 - 3).abs() + (c.1 - 2).abs(), 1, "Nachbarzelle: {c:?}");
        // new icons fill the grid's columns from the top
        let first = GRID.arranged(9);
        assert_eq!(first[0], (8, 8));
        assert_eq!(first[7], GRID.cell_point((1, 0)));
        // full grid
        let small = Geometry {
            width: 216,
            height: 192,
            ..GRID
        };
        let full = small.arranged(4);
        assert_eq!(small.nearest_free((50, 50), &full), None);
    }

    #[test]
    fn display_snaps_remembered_pixels_without_overlap() {
        let wanted = [
            Some(GRID.free_place((1154, 27))),   // Papierkorb rechts 26 oben 27
            Some(GRID.top_left_place((8, 230))), // Terminal oben 230
            Some(GRID.top_left_place((8, 8))),
            Some(GRID.top_left_place((10, 12))), // fast dieselbe Zelle
        ];
        let shown = GRID.place_all(&wanted);
        assert_eq!(shown[0], GRID.cell_point((11, 0)));
        assert_eq!(shown[1], GRID.cell_point((0, 2)));
        assert_eq!(shown[2], (8, 8));
        assert_ne!(shown[3], (8, 8));
        for (i, a) in shown.iter().enumerate() {
            assert_eq!(GRID.snapped(*a), *a);
            for b in &shown[i + 1..] {
                assert!(!GRID.overlaps(*a, *b));
            }
        }
        // without snapping: exactly the remembered pixels (HMP-10)
        let free = Geometry {
            snap: false,
            ..GRID
        };
        assert_eq!(free.place_all(&wanted)[1], (8, 230));
    }

    #[test]
    fn a_group_moves_by_whole_cells_and_keeps_its_shape() {
        let group: Vec<Point> = [(0, 0), (1, 0), (0, 1)]
            .map(|c| GRID.cell_point(c))
            .to_vec();
        let others = vec![GRID.cell_point((6, 3))];
        // lead (0,0) dropped near cell (4,2)
        let want = (
            GRID.cell_point((4, 2)).0 + 20,
            GRID.cell_point((4, 2)).1 - 10,
        );
        let moved = GRID.place_group(&group, 0, want, &others).expect("Platz");
        assert_eq!(
            moved,
            [(4, 2), (5, 2), (4, 3)]
                .map(|c| GRID.cell_point(c))
                .to_vec()
        );
        // dropped where (5,2) would hit the other icon at (6,3)? shape kept, no overlap
        let want = GRID.cell_point((5, 2));
        let moved = GRID.place_group(&group, 0, want, &others).expect("Platz");
        assert!(
            moved.iter().all(|&p| !GRID.overlaps(p, others[0])),
            "{moved:?}"
        );
        let cells: Vec<Cell> = moved.iter().map(|&p| GRID.cell_of(p)).collect();
        assert_eq!((cells[1].0 - cells[0].0, cells[2].1 - cells[0].1), (1, 1));
        // past the right edge: the group stops at the edge
        let moved = GRID.place_group(&group, 0, (5000, 8), &[]).expect("Platz");
        assert_eq!(GRID.cell_of(moved[1]), (11, 0));
        assert_eq!(GRID.cell_of(moved[0]), (10, 0));
        // a group larger than the area cannot move
        let tiny = Geometry {
            width: 216,
            height: 104,
            ..GRID
        };
        assert_eq!(tiny.place_group(&group, 0, (8, 8), &[]), None);
    }

    #[test]
    fn a_free_group_keeps_its_pixels_and_avoids_others() {
        let group = vec![(100, 100), (210, 130)];
        let moved = AREA.place_group(&group, 0, (300, 300), &[]).expect("Platz");
        assert_eq!(moved, vec![(300, 300), (410, 330)]);
        let others = [(410, 330)];
        let moved = AREA
            .place_group(&group, 0, (300, 300), &others)
            .expect("Platz");
        assert!(moved.iter().all(|&p| !AREA.overlaps(p, others[0])));
        assert_eq!(moved[1].0 - moved[0].0, 110);
        assert_eq!(moved[1].1 - moved[0].1, 30);
        let moved = AREA
            .place_group(&group, 0, (-400, -400), &[])
            .expect("Platz");
        assert_eq!(moved[0], (8, 8));
    }

    #[test]
    fn the_rubber_band_takes_what_it_touches_in_any_direction() {
        let positions = [(8, 8), (108, 8), (8, 96), (600, 400)];
        let down_right = GRID.in_rectangle(&positions, (50.0, 50.0), (150.0, 120.0));
        assert_eq!(down_right, [0, 1, 2]);
        let up_left = GRID.in_rectangle(&positions, (150.0, 120.0), (50.0, 50.0));
        assert_eq!(up_left, down_right);
        assert!(
            GRID.in_rectangle(&positions, (300.0, 300.0), (310.0, 310.0))
                .is_empty()
        );
    }

    #[test]
    fn shift_click_selects_in_reading_order() {
        let positions = [(108, 8), (8, 96), (8, 8), (108, 96), (208, 8)];
        // reading order: (8,8)=2, (8,96)=1, (108,8)=0, (108,96)=3, (208,8)=4
        let mut range = range_between(&positions, 1, 3);
        range.sort_unstable();
        assert_eq!(range, [0, 1, 3]);
        let mut back = range_between(&positions, 3, 1);
        back.sort_unstable();
        assert_eq!(back, range);
        assert_eq!(range_between(&positions, 2, 2), [2]);
    }

    #[test]
    fn a_group_that_does_not_fit_is_refused() {
        // 400×236 between the panels: 3×2 cells, six of them
        let small = Geometry {
            width: 400,
            height: 236,
            ..GRID
        };
        assert_eq!(small.cells(), (3, 2));
        let cells: Vec<Point> = small.arranged(6);
        // more icons than cells: the other four lie on top (display only)
        let group = vec![cells[0], cells[1]];
        let others: Vec<Point> = cells[2..]
            .iter()
            .copied()
            .chain([cells[0], cells[3]])
            .collect();
        assert_eq!(
            small.move_group(&group, 0, (300, 150), &others),
            None,
            "kein Platz: abgelehnt"
        );
        // one free cell (their own) for two icons: refused too, nothing
        // half done
        let others: Vec<Point> = cells[1..6].to_vec();
        let group = vec![(8, 8), (10, 12)]; // two icons on one cell
        assert_eq!(small.place_group(&group, 0, cells[5], &others), None);
        assert_eq!(small.move_group(&group, 0, cells[5], &others), None);
        // enough room, but not for the arrangement: one by one, no overlap
        let wide = Geometry {
            width: 1280,
            height: 656,
            ..GRID
        };
        let group = vec![(8, 8), (10, 12)];
        let others = vec![wide.cell_point((5, 3))];
        let moved = wide
            .move_group(&group, 0, wide.cell_point((5, 3)), &others)
            .expect("Platz");
        assert_eq!(moved.len(), 2);
        for (i, a) in moved.iter().enumerate() {
            assert!(wide.is_free(*a, &others), "{a:?}");
            for b in &moved[i + 1..] {
                assert!(!wide.overlaps(*a, *b));
            }
        }
        // a whole full desktop: every drop is refused, the drop keeps the old cells
        let full = small.arranged(6);
        let others: Vec<Point> = full[2..].to_vec();
        let back = small
            .move_group(&full[..2], 0, (5000, 5000), &others)
            .expect("eigene Plätze");
        assert!(back.iter().all(|p| full[..2].contains(p)), "{back:?}");
    }

    #[test]
    fn volumes_appear_below_the_trash_and_move_nobody() {
        let trash = GRID.cell_point((11, 0));
        let remembered = [
            GRID.cell_point((0, 0)),
            GRID.cell_point((0, 1)),
            trash,
            GRID.cell_point((0, 6)),
        ];
        assert_eq!(
            GRID.place_transient(&remembered, &[None, None], Some(trash)),
            [GRID.cell_point((11, 1)), GRID.cell_point((11, 2))]
        );
        let mut taken = remembered.to_vec();
        taken.push(GRID.cell_point((11, 1)));
        assert_eq!(GRID.below(Some(trash), &taken), GRID.cell_point((11, 2)));
        // a full column: the next one towards the middle, from the top
        let column: Vec<Point> = (0..7).map(|r| GRID.cell_point((11, r))).collect();
        assert_eq!(GRID.below(Some(trash), &column), GRID.cell_point((10, 0)));
        let left = GRID.cell_point((0, 2));
        let first: Vec<Point> = (0..7).map(|r| GRID.cell_point((0, r))).collect();
        assert_eq!(GRID.below(Some(left), &first), GRID.cell_point((1, 0)));
        // without a trash icon: like any new icon
        assert_eq!(GRID.below(None, &remembered), GRID.cell_point((0, 2)));
        // a place from this session stays, new volumes still go below the trash
        let moved = GRID.free_place(GRID.cell_point((5, 3)));
        assert_eq!(
            GRID.place_transient(&remembered, &[Some(moved), None], Some(trash)),
            [GRID.cell_point((5, 3)), GRID.cell_point((11, 1))]
        );
        // a session place taken by a remembered icon: the volume gives way
        let on_home = GRID.free_place(GRID.cell_point((0, 1)));
        let given_way = GRID.place_transient(&remembered, &[Some(on_home)], Some(trash));
        assert!(GRID.is_free(given_way[0], &remembered), "{given_way:?}");
        // free placement: straight below, in tile steps
        assert_eq!(AREA.below(Some((1172, 8)), &[(1172, 8)]), (1172, 96));
        assert_eq!(
            AREA.below(Some((1172, 8)), &[(1172, 8), (1172, 96)]),
            (1172, 184)
        );
    }
}
