pub mod my_compass;

use crate::my_map_point::my_compass::*;
use std::cmp::Ordering;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct MapPoint<const X: usize, const Y: usize> {
    // X: size of dimension x
    // Y: size of dimension Y
    // x and y are not public, because changing them without the provided functions can result in unwanted panics!
    x: usize,
    y: usize,
}

impl<const X: usize, const Y: usize> MapPoint<X, Y> {
    pub fn new(x: usize, y: usize) -> Self {
        if X == 0 {
            panic!("line {}, minimum size of dimension X is 1", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum size of dimension Y is 1", line!());
        }
        let result = MapPoint { x, y };
        if !result.is_in_map() {
            panic!("line {}, coordinates are out of range", line!());
        }
        result
    }
    pub fn x(&self) -> usize {
        self.x
    }
    pub fn y(&self) -> usize {
        self.y
    }
    pub fn distance_x(&self, target: MapPoint<X, Y>) -> usize {
        match self.x.cmp(&target.x) {
            Ordering::Equal => 0,
            Ordering::Greater => self.x - target.x,
            Ordering::Less => target.x - self.x,
        }
    }
    pub fn distance_y(&self, target: MapPoint<X, Y>) -> usize {
        match self.y.cmp(&target.y) {
            Ordering::Equal => 0,
            Ordering::Greater => self.y - target.y,
            Ordering::Less => target.y - self.y,
        }
    }
    pub fn distance(&self, target: MapPoint<X, Y>) -> usize {
        self.distance_x(target) + self.distance_y(target)
    }
    pub fn delta_xy(&self, target: MapPoint<X, Y>) -> usize {
        let dist_x = self.distance_x(target);
        let dist_y = self.distance_y(target);
        match dist_x.cmp(&dist_y) {
            Ordering::Equal => 0,
            Ordering::Greater => dist_x - dist_y,
            Ordering::Less => dist_y - dist_x,
        }
    }
    pub fn is_in_map(&self) -> bool {
        self.x < X && self.y < Y
    }
    pub fn map_position(&self) -> Compass {
        match (self.x, self.y) {
            (0, 0) => Compass::NW,
            (x, 0) if x == X - 1 => Compass::NE,
            (0, y) if y == Y - 1 => Compass::SW,
            (x, y) if x == X - 1 && y == Y - 1 => Compass::SE,
            (x, 0) if x < X - 1 => Compass::N,
            (0, y) if y < Y - 1 => Compass::W,
            (x, y) if y == Y - 1 && x < X - 1 => Compass::S,
            (x, y) if x == X - 1 && y < Y - 1 => Compass::E,
            _ => Compass::Center,
        }
    }
    pub fn forward_x(&self) -> Option<MapPoint<X, Y>> {
        // increments x, if x reaches row end, move to start of next row; if x reaches end of map, return None
        let mut result = *self;
        result.x += 1;
        if result.x == X {
            result.y += 1;
            if result.y == Y {
                return None;
            }
            result.x = 0;
        }
        Some(result)
    }
    pub fn backward_x(&self) -> Option<MapPoint<X, Y>> {
        // decrements x, if x reaches row start, move to end of previous row; if x reaches start of map, return None
        let mut result = *self;
        if result.x == 0 {
            if result.y == 0 {
                return None;
            }
            result.y -= 1;
            result.x = X - 1;
        } else {
            result.x -= 1;
        }
        Some(result)
    }
    pub fn forward_y(&self) -> Option<MapPoint<X, Y>> {
        // increments y, if y reaches column end, move to end of next column; if y reaches end of map, return None
        let mut result = *self;
        result.y += 1;
        if result.y == Y {
            result.x += 1;
            if result.x == X {
                return None;
            }
            result.y = 0;
        }
        Some(result)
    }
    pub fn backward_y(&self) -> Option<MapPoint<X, Y>> {
        // decrements y, if y reaches column start, move to end of previous column; if y reaches start of map, return None
        let mut result = *self;
        if result.y == 0 {
            if result.x == 0 {
                return None;
            }
            result.x -= 1;
            result.y = Y - 1;
        } else {
            result.y -= 1;
        }
        Some(result)
    }
    pub fn offset_pp(&self, offset: (usize, usize)) -> Option<MapPoint<X, Y>> {
        let result = MapPoint {
            x: self.x + offset.0,
            y: self.y + offset.1,
        };
        if result.is_in_map() {
            Some(result)
        } else {
            None
        }
    }
    pub fn offset_mm(&self, offset: (usize, usize)) -> Option<MapPoint<X, Y>> {
        if offset.0 > self.x || offset.1 > self.y {
            return None;
        }
        let result = MapPoint {
            x: self.x - offset.0,
            y: self.y - offset.1,
        };
        if result.is_in_map() {
            Some(result)
        } else {
            None
        }
    }
    pub fn neighbor(&self, orientation: Compass) -> Option<MapPoint<X, Y>> {
        match orientation {
            Compass::Center => Some(*self),
            Compass::N => self.offset_mm((0, 1)),
            Compass::NE => self.offset_mm((0, 1)).and_then(|n| n.offset_pp((1, 0))),
            Compass::E => self.offset_pp((1, 0)),
            Compass::SE => self.offset_pp((1, 1)),
            Compass::S => self.offset_pp((0, 1)),
            Compass::SW => self.offset_pp((0, 1)).and_then(|s| s.offset_mm((1, 0))),
            Compass::W => self.offset_mm((1, 0)),
            Compass::NW => self.offset_mm((1, 1)),
        }
    }
    pub fn orientation_of_neighbor(&self, neighbor: MapPoint<X, Y>) -> Option<Compass> {
        self.iter_neighbors(Compass::N, true, false, true)
            .find(|(p, _)| *p == neighbor)
            .map(|(_, o)| o)
    }
    pub fn iter_neighbors(
        &self,
        initial_orientation: Compass,
        rotation_direction: bool,
        include_center: bool,
        include_corners: bool,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, Compass)> + use<X, Y> {
        NeighborIter::new(
            *self,
            initial_orientation,
            rotation_direction,
            include_center,
            include_corners,
        )
    }
    pub fn iter_orientation(
        &self,
        orientation: Compass,
    ) -> impl Iterator<Item = MapPoint<X, Y>> + use<X, Y> {
        OrientationIter::new(*self, orientation)
    }
}

struct NeighborIter<const X: usize, const Y: usize> {
    include_center: bool,
    include_corners: bool,
    center_point: MapPoint<X, Y>,
    initial_orientation: Compass,
    current_orientation: Compass,
    rotation_direction: bool,
    finished: bool,
}

impl<const X: usize, const Y: usize> NeighborIter<X, Y> {
    fn new(
        center_point: MapPoint<X, Y>,
        initial_orientation: Compass,
        rotation_direction: bool,
        include_center: bool,
        include_corners: bool,
    ) -> Self {
        if initial_orientation.is_center() {
            panic!("line {}, need direction", line!());
        }
        if !include_corners && initial_orientation.is_ordinal() {
            panic!("line {}, need side direction", line!());
        }
        NeighborIter {
            include_center,
            include_corners,
            center_point,
            initial_orientation,
            current_orientation: initial_orientation,
            rotation_direction,
            finished: false,
        }
    }
    fn rotate_orientation(&mut self) {
        if self.include_center {
            self.include_center = false;
        } else if self.rotation_direction {
            // rotate clockwise
            self.current_orientation = if self.include_corners {
                self.current_orientation.clockwise()
            } else {
                self.current_orientation.clockwise().clockwise()
            };
            self.finished = self.current_orientation == self.initial_orientation;
        } else {
            // rotate counterclockwise
            self.current_orientation = if self.include_corners {
                self.current_orientation.counterclockwise()
            } else {
                self.current_orientation
                    .counterclockwise()
                    .counterclockwise()
            };
            self.finished = self.current_orientation == self.initial_orientation;
        }
    }
}

impl<const X: usize, const Y: usize> Iterator for NeighborIter<X, Y> {
    type Item = (MapPoint<X, Y>, Compass);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.finished {
            let result = if self.include_center {
                Some((self.center_point, Compass::Center))
            } else {
                self.center_point
                    .neighbor(self.current_orientation)
                    .map(|n| (n, self.current_orientation))
            };
            match result {
                Some(map_point) => {
                    self.rotate_orientation();
                    return Some(map_point);
                }
                None => self.rotate_orientation(),
            }
        }
        None
    }
}

struct OrientationIter<const X: usize, const Y: usize> {
    current_point: MapPoint<X, Y>,
    orientation: Compass,
    finished: bool,
}

impl<const X: usize, const Y: usize> OrientationIter<X, Y> {
    fn new(start_point: MapPoint<X, Y>, orientation: Compass) -> Self {
        if orientation.is_center() {
            panic!("line {}, need direction", line!());
        }
        OrientationIter {
            current_point: start_point,
            orientation,
            finished: false,
        }
    }
}

impl<const X: usize, const Y: usize> Iterator for OrientationIter<X, Y> {
    type Item = MapPoint<X, Y>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let result = self.current_point;
        match self.current_point.neighbor(self.orientation) {
            Some(map_point) => self.current_point = map_point,
            None => self.finished = true,
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn iter_map_test() {
        const X: usize = 3;
        const Y: usize = 3;
        let mut point = MapPoint::<X, Y>::new(0, 0);
        assert_eq!(point, MapPoint::<X, Y>::new(0, 0));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(1, 0));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(2, 0));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(0, 1));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(1, 1));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(2, 1));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(0, 2));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(1, 2));
        point = point.forward_x().unwrap();
        assert_eq!(point, MapPoint::<X, Y>::new(2, 2));
        assert_eq!(point.forward_x(), None);
    }

    #[test]
    fn side_and_corner_test() {
        const X: usize = 20;
        const Y: usize = 10;
        let a = MapPoint::<X, Y>::new(0, 0);
        assert!(a.map_position().is_ordinal());
        let a = MapPoint::<X, Y>::new(19, 0);
        assert!(a.map_position().is_ordinal());
        let a = MapPoint::<X, Y>::new(0, 9);
        assert!(a.map_position().is_ordinal());
        let a = MapPoint::<X, Y>::new(19, 9);
        assert!(a.map_position().is_ordinal());
        let a = MapPoint::<X, Y>::new(0, 5);
        assert!(a.map_position().is_cardinal());
        let a = MapPoint::<X, Y>::new(19, 3);
        assert!(a.map_position().is_cardinal());
        let a = MapPoint::<X, Y>::new(7, 0);
        assert!(a.map_position().is_cardinal());
        let a = MapPoint::<X, Y>::new(18, 9);
        assert!(a.map_position().is_cardinal());
        let a = MapPoint::<X, Y>::new(18, 8);
        assert!(a.map_position().is_center());
    }
}
