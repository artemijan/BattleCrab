//! Ports of `util/LinePointIterator` and `util/LinePointIterator3D` — the
//! Bresenham-style cell walkers the geo engine drives its LOS / move checks
//! with.
//!
//! These are **not** `Iterator`s, and the stepping method is deliberately not
//! called `next`. Java's is, but the two contracts differ in a way that would
//! mislead here: `Iterator::next` *returns* the item and ends with `None`,
//! whereas these return `bool` for "did I move?" and leave the caller to read
//! `x()`/`y()`/`z()` afterwards. A method called `next` returning `bool` invites
//! exactly the wrong reading. Hence [`advance`](LinePointIterator::advance),
//! the same name on both types.
//!
//! (The struct names keep Java's `…Iterator` spelling because they are the
//! citation — the doc line above is how you find the original.)

/// `util/LinePointIterator`: walks geo cells from src to dst in 2D. The first
/// [`advance`](Self::advance) yields the source point itself.
pub struct LinePointIterator {
    src_x: i32,
    src_y: i32,
    dst_x: i32,
    dst_y: i32,
    dx: i64,
    dy: i64,
    sx: i64,
    sy: i64,
    error: i64,
    first: bool,
}

impl LinePointIterator {
    pub fn new(src_x: i32, src_y: i32, dst_x: i32, dst_y: i32) -> Self {
        let dx = (dst_x as i64 - src_x as i64).abs();
        let dy = (dst_y as i64 - src_y as i64).abs();
        Self {
            src_x,
            src_y,
            dst_x,
            dst_y,
            dx,
            dy,
            sx: if src_x < dst_x { 1 } else { -1 },
            sy: if src_y < dst_y { 1 } else { -1 },
            error: if dx >= dy { dx / 2 } else { dy / 2 },
            first: true,
        }
    }

    /// Step to the next cell along the line, or report that the destination
    /// has been reached. Read the position off `x()`/`y()` after each `true`.
    pub fn advance(&mut self) -> bool {
        if self.first {
            self.first = false;
            return true;
        }
        if self.dx >= self.dy {
            if self.src_x != self.dst_x {
                self.src_x += self.sx as i32;
                self.error += self.dy;
                if self.error >= self.dx {
                    self.src_y += self.sy as i32;
                    self.error -= self.dx;
                }
                return true;
            }
        } else if self.src_y != self.dst_y {
            self.src_y += self.sy as i32;
            self.error += self.dx;
            if self.error >= self.dy {
                self.src_x += self.sx as i32;
                self.error -= self.dy;
            }
            return true;
        }
        false
    }

    pub fn x(&self) -> i32 {
        self.src_x
    }

    pub fn y(&self) -> i32 {
        self.src_y
    }
}

/// `util/LinePointIterator3D`: 3D variant used by the LOS check; z steps along
/// the "bee line" between the two heights. Consecutive points may share the
/// same x/y (z-major lines) — callers skip those.
pub struct LinePointIterator3D {
    src_x: i32,
    src_y: i32,
    src_z: i32,
    dst_x: i32,
    dst_y: i32,
    dst_z: i32,
    dx: i64,
    dy: i64,
    dz: i64,
    sx: i64,
    sy: i64,
    sz: i64,
    error: i64,
    error2: i64,
    first: bool,
}

impl LinePointIterator3D {
    pub fn new(src_x: i32, src_y: i32, src_z: i32, dst_x: i32, dst_y: i32, dst_z: i32) -> Self {
        let dx = (dst_x as i64 - src_x as i64).abs();
        let dy = (dst_y as i64 - src_y as i64).abs();
        let dz = (dst_z as i64 - src_z as i64).abs();
        let error = if dx >= dy && dx >= dz {
            dx / 2
        } else if dy >= dx && dy >= dz {
            dy / 2
        } else {
            dz / 2
        };
        Self {
            src_x,
            src_y,
            src_z,
            dst_x,
            dst_y,
            dst_z,
            dx,
            dy,
            dz,
            sx: if src_x < dst_x { 1 } else { -1 },
            sy: if src_y < dst_y { 1 } else { -1 },
            sz: if src_z < dst_z { 1 } else { -1 },
            error,
            error2: error,
            first: true,
        }
    }

    /// Step to the next cell along the line, or report that the destination
    /// has been reached. Read the position off `x()`/`y()`/`z()` after each
    /// `true`.
    pub fn advance(&mut self) -> bool {
        if self.first {
            self.first = false;
            return true;
        }
        if self.dx >= self.dy && self.dx >= self.dz {
            if self.src_x != self.dst_x {
                self.src_x += self.sx as i32;
                self.error += self.dy;
                if self.error >= self.dx {
                    self.src_y += self.sy as i32;
                    self.error -= self.dx;
                }
                self.error2 += self.dz;
                if self.error2 >= self.dx {
                    self.src_z += self.sz as i32;
                    self.error2 -= self.dx;
                }
                return true;
            }
        } else if self.dy >= self.dx && self.dy >= self.dz {
            if self.src_y != self.dst_y {
                self.src_y += self.sy as i32;
                self.error += self.dx;
                if self.error >= self.dy {
                    self.src_x += self.sx as i32;
                    self.error -= self.dy;
                }
                self.error2 += self.dz;
                if self.error2 >= self.dy {
                    self.src_z += self.sz as i32;
                    self.error2 -= self.dy;
                }
                return true;
            }
        } else if self.src_z != self.dst_z {
            self.src_z += self.sz as i32;
            self.error += self.dx;
            if self.error >= self.dz {
                self.src_x += self.sx as i32;
                self.error -= self.dz;
            }
            self.error2 += self.dy;
            if self.error2 >= self.dz {
                self.src_y += self.sy as i32;
                self.error2 -= self.dz;
            }
            return true;
        }
        false
    }

    pub fn x(&self) -> i32 {
        self.src_x
    }

    pub fn y(&self) -> i32 {
        self.src_y
    }

    pub fn z(&self) -> i32 {
        self.src_z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 2D iterator must start at the source, end exactly on the
    /// destination, and step one cell per `advance()` along the driving axis.
    #[test]
    fn line_2d_walks_from_src_to_dst() {
        let mut it = LinePointIterator::new(0, 0, 5, 2);
        let mut points = Vec::new();
        while it.advance() {
            points.push((it.x(), it.y()));
        }
        assert_eq!(points.first(), Some(&(0, 0)));
        assert_eq!(points.last(), Some(&(5, 2)));
        assert_eq!(points.len(), 6, "x-major line yields dx+1 points");
        for w in points.windows(2) {
            assert_eq!((w[1].0 - w[0].0).abs(), 1, "x advances every step");
            assert!((w[1].1 - w[0].1).abs() <= 1);
        }
    }

    /// Degenerate line: a single point, yielded exactly once.
    #[test]
    fn line_2d_single_point() {
        let mut it = LinePointIterator::new(3, 4, 3, 4);
        assert!(it.advance());
        assert_eq!((it.x(), it.y()), (3, 4));
        assert!(!it.advance());
    }

    /// The 3D iterator interpolates z monotonically between the endpoints and
    /// lands exactly on the destination.
    #[test]
    fn line_3d_interpolates_z() {
        let mut it = LinePointIterator3D::new(0, 0, 0, 10, 0, 5);
        let mut points = Vec::new();
        while it.advance() {
            points.push((it.x(), it.y(), it.z()));
        }
        assert_eq!(points.first(), Some(&(0, 0, 0)));
        assert_eq!(points.last(), Some(&(10, 0, 5)));
        for w in points.windows(2) {
            assert!(w[1].2 >= w[0].2, "z never steps backwards on a rising line");
        }
    }
}
