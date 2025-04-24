//⏬my_map_two_dim.rs
mod my_map_point;

use my_map_point::my_compass::*;

// use MyMap2D if compilation time is suffice, because it is more efficient and has cleaner interface
#[derive(Copy, Clone, PartialEq)]
struct MyMap2D<T, const X: usize, const Y: usize, const N: usize> { // X: number of columns, Y: number of rows, N: number of elements in map: X * Y
    items: [[T; X] ; Y], //outer array rows, inner array columns -> first index chooses row (y), second index chooses column (x)
}

impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> MyMap2D<T, X, Y, N> {
    fn new() -> Self {
        if X == 0 {
            panic!("line {}, minimum one column", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum one row", line!());
        }
        Self { items: [[T::default(); X] ; Y], }
    }
    fn init(init_element: T) -> Self {
        if X == 0 {
            panic!("line {}, minimum one column", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum one row", line!());
        }
        Self { items: [[init_element; X] ; Y], }
    }
    fn get(&self, coordinates: MapPoint<X, Y>) -> &T {
        &self.items[coordinates.y()][coordinates.x()]
    }
    fn get_mut(&mut self, coordinates: MapPoint<X, Y>) -> &mut T {
        &mut self.items[coordinates.y()][coordinates.x()]
    }
    fn set(&mut self, coordinates: MapPoint<X, Y>, value: T) -> &T {
        self.items[coordinates.y()][coordinates.x()] = value;
        &self.items[coordinates.y()][coordinates.x()]
    }
    fn is_cut_off_cell(&self, map_point: MapPoint<X, Y>, is_cell_free_fn: Box<dyn Fn(MapPoint<X, Y>, &T) -> bool>) -> bool {
        // use is_cell_free_fn as follows (use "_" for unused variables):
        // let is_cell_free_fn = Box::new(|current_point: MapPoint<X, Y>, current_cell_value: &T| current_point.use_it_somehow() || current_cell_value.use_it_somehow() );
        let (mut last_free, initial_orientation) = match map_point.map_position() {
            Compass::NW | Compass::N => (false, Compass::E),
            Compass::NE | Compass::E => (false, Compass::S),
            Compass::SE | Compass::S => (false, Compass::W),
            Compass::SW | Compass::W => (false, Compass::N),
            Compass::Center => {
                let nw = map_point.neighbor(Compass::NW).unwrap();
                (is_cell_free_fn(nw, self.get(nw)), Compass::N)
            },
        };
        let mut free_zones = 0;
        for (is_free, is_side) in map_point.iter_neighbors(initial_orientation, true, false, true).map(|(p, o)| (is_cell_free_fn(p, self.get(p)), o.is_cardinal())) {
            if !last_free {
                if is_free && is_side {
                    // new free zones start always at a side of map_point, since movement over cornes is not allowed
                    free_zones += 1;
                }
            }
            last_free = if is_side || !is_free {
                // side or blocked corner -> apply is_free to last_free
                is_free
            } else {
                // free corner -> keep old value of last_free
                last_free
            };
        }
        free_zones > 1
    }
    fn iter(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        self.items
            .iter()
            .enumerate()
            .flat_map(|(y, row)| row.iter().enumerate().map(move |(x, column)| (MapPoint::<X, Y>::new(x, y), column)))
    }
    fn iter_mut(&mut self) -> impl Iterator<Item = (MapPoint<X, Y>, &mut T)> {
        self.items
            .iter_mut()
            .enumerate()
            .flat_map(|(y, row)| row.iter_mut().enumerate().map(move |(x, column)| (MapPoint::<X, Y>::new(x, y), column)))
    }
    fn iter_row(&self, r: usize) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        if r >= Y {
            panic!("line {}, row index is out of range", line!());
        }
        self.items
            .iter()
            .enumerate()
            .filter(move |(y, _)| *y == r)
            .flat_map(|(y, row)| row.iter().enumerate().map(move |(x, column)| (MapPoint::new(x, y), column)))
    }
    fn iter_column(&self, c: usize) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        if c >= X {
            panic!("line {}, column index is out of range", line!());
        }
        self.items
            .iter()
            .enumerate()
            .flat_map(move |(y, row)| row.iter().enumerate().filter(move |(x, _)| *x == c).map(move |(x, column)| (MapPoint::new(x, y), column)))
    }
    fn iter_neighbors(&self, center_point: MapPoint<X, Y>) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        center_point.iter_neighbors(Compass::N, true, false, false).map(move |(p, _)| (p, self.get(p)))
    }
    fn iter_neighbors_mut(&mut self, center_point: MapPoint<X, Y>) -> impl Iterator<Item = (MapPoint<X, Y>, &mut T)> {
        center_point.iter_neighbors(Compass::N, true, false, false).map(move |(p, _)| unsafe { (p, &mut *(self.get_mut(p) as *mut _ )) } )
    }
    fn iter_neighbors_with_center(&self, center_point: MapPoint<X, Y>) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        center_point.iter_neighbors(Compass::N, true, true, false).map(move |(p, _)| (p, self.get(p)))
    }
    fn iter_neighbors_with_corners(&self, center_point: MapPoint<X, Y>) -> impl Iterator<Item = (MapPoint<X, Y>, &T, bool)> {
        center_point.iter_neighbors(Compass::N, true, false, true).map(move |(p, o)| (p, self.get(p), o.is_ordinal()))
    }
    fn iter_neighbors_with_center_and_corners(&self, center_point: MapPoint<X, Y>) -> impl Iterator<Item = (MapPoint<X, Y>, &T, bool)> {
        center_point.iter_neighbors(Compass::N, true, true, true).map(move |(p, o)| (p, self.get(p), o.is_ordinal()))
    }
    fn iter_orientation(&self, start_point: MapPoint<X, Y>, orientation: Compass) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        start_point.iter_orientation(orientation).map(move |p| (p, self.get(p)))
    }
    fn iter_diagonale_top_left(&self)  -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(0, 0).iter_orientation(Compass::SE).map(move |p| (p, self.get(p)))
    }
    fn iter_diagonale_top_right(&self)  -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(X - 1, 0).iter_orientation(Compass::SW).map(move |p| (p, self.get(p)))
    }
    fn iter_diagonale_bottom_left(&self)  -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(0, Y - 1).iter_orientation(Compass::NE).map(move |p| (p, self.get(p)))
    }
    fn iter_diagonale_bottom_right(&self)  -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(X - 1, Y - 1).iter_orientation(Compass::NW).map(move |p| (p, self.get(p)))
    }
    fn iter_distance<'a>(&'a self, start_point: MapPoint<X, Y>, filter_fn: Box<dyn Fn(MapPoint<X, Y>, &T, usize) -> bool>) -> impl Iterator<Item = (MapPoint<X, Y>, &'a T, usize)> {
        // use filter_fn as follows (use "_" for unused variables):
        // let filter_fn = Box::new(|point_of_next_cell: MapPoint<X, Y>, value_of_next_cell: &T, current_distance: usize| current_point.use_it_somehow() || current_cell_value.use_it_somehow() || current_distance.use_it_somehow());
        DistanceIter::new(self, start_point, filter_fn)
    }
}

impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Default for MyMap2D<T, X, Y, N> {
    fn default() -> Self {
        Self::new()
    }
}


struct DistanceIter<'a, T, const X: usize, const Y: usize, const N: usize> {
    data_map: &'a MyMap2D<T, X, Y, N>,
    filter_fn: Box<dyn Fn(MapPoint<X, Y>, &T, usize) -> bool>, // input for filter_fn: next possible point, data from data_map of next possible point, distance of current point
    next_cells: MyArray<(MapPoint<X, Y>, usize), N>,
    index: usize,
}

impl<'a, T: Copy + Clone, const X: usize, const Y: usize, const N: usize> DistanceIter<'a, T, X, Y, N> {
    fn new(data_map: &'a MyMap2D<T, X, Y, N>, start_point: MapPoint<X, Y>, filter_fn: Box<dyn Fn(MapPoint<X, Y>, &T, usize) -> bool>) -> Self {
        DistanceIter {
            data_map,
            filter_fn,
            next_cells: MyArray::init((start_point, 0), 1),
            index: 0,
        }
    }
}

impl<'a, T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Iterator for DistanceIter<'a, T, X, Y, N> {
    type Item = (MapPoint<X, Y>, &'a T, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.next_cells.len() {
            return None
        }
        let (map_point, distance) = *self.next_cells.get(self.index).unwrap();
        let mut local_next_cells: MyArray<(MapPoint<X, Y>, usize), 4> = MyArray::new();
        for (next_cell, _) in self.data_map.iter_neighbors(map_point).filter(|(p, c)| self.next_cells.iter().find(|(n, _)| n == p).is_none() && (self.filter_fn)(*p, *c, distance)) {
            local_next_cells.push((next_cell, distance + 1));
        }
        self.next_cells.append_slice(local_next_cells.as_slice());
        self.index += 1;
        Some((map_point, self.data_map.get(map_point), distance))
    }
}
//⏫my_map_two_dim.rs
//⏬my_map_point.rs
mod my_compass;

use std::cmp::Ordering;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
struct MapPoint<const X: usize, const Y: usize> {
    // X: size of dimension x
    // Y: size of dimension Y
    // x and y are not public, because changing them without the provided functions can result in unwanted panics!
    x: usize,
    y: usize,
}

impl<const X: usize, const Y: usize> MapPoint<X, Y> {
    fn new(x: usize, y: usize) -> Self {
        if X == 0 {
            panic!("line {}, minimum size of dimension X is 1", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum size of dimension Y is 1", line!());
        }
        let result = MapPoint { x, y, };
        if !result.is_in_map() {
            panic!("line {}, coordinates are out of range", line!());
        }
        result
    }
    fn x(&self) -> usize {
        self.x
    }
    fn y(&self) -> usize {
        self.y
    }
    fn distance_x(&self, target: MapPoint<X, Y>) -> usize {
        match self.x.cmp(&target.x) {
            Ordering::Equal => 0,
            Ordering::Greater => self.x - target.x,
            Ordering::Less => target.x - self.x,
        }
    }
    fn distance_y(&self, target: MapPoint<X, Y>) -> usize {
        match self.y.cmp(&target.y) {
            Ordering::Equal => 0,
            Ordering::Greater => self.y - target.y,
            Ordering::Less => target.y - self.y,
        }
    }
    fn distance(&self, target: MapPoint<X, Y>) -> usize {
        self.distance_x(target) + self.distance_y(target)
    }
    fn delta_xy(&self, target: MapPoint<X, Y>) -> usize {
        let dist_x = self.distance_x(target);
        let dist_y = self.distance_y(target);
        match dist_x.cmp(&dist_y) {
            Ordering::Equal => 0,
            Ordering::Greater => dist_x - dist_y,
            Ordering::Less => dist_y - dist_x,
        }
    }
    fn is_in_map(&self) -> bool {
        self.x < X && self.y < Y
    }
    fn map_position(&self) -> Compass {
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
    fn forward_x(&self) -> Option<MapPoint<X, Y>> {
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
    fn backward_x(&self) -> Option<MapPoint<X, Y>> {
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
    fn forward_y(&self) -> Option<MapPoint<X, Y>> {
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
    fn backward_y(&self) -> Option<MapPoint<X, Y>> {
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
    fn offset_pp(&self, offset: (usize, usize)) -> Option<MapPoint<X, Y>> {
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
    fn offset_mm(&self, offset: (usize, usize)) -> Option<MapPoint<X, Y>> {
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
    fn neighbor(&self, orientation: Compass) -> Option<MapPoint<X, Y>> {
        match orientation {
            Compass::Center => Some(*self),
            Compass::N => self.offset_mm((0, 1)),
            Compass::NE => self.offset_mm((0, 1)).map_or(None, |n| n.offset_pp((1, 0))),
            Compass::E => self.offset_pp((1, 0)),
            Compass::SE => self.offset_pp((1, 1)),
            Compass::S => self.offset_pp((0, 1)),
            Compass::SW => self.offset_pp((0, 1)).map_or(None, |s| s.offset_mm((1, 0))),
            Compass::W => self.offset_mm((1, 0)),
            Compass::NW => self.offset_mm((1, 1)),
        }
    }
    fn orientation_of_neighbor(&self, neighbor: MapPoint<X, Y>) -> Option<Compass> {
        self.iter_neighbors(Compass::N, true, false, true).find(|(p, _)| *p == neighbor).map_or(None, |(_, o)| Some(o))
    }
    fn iter_neighbors(&self, initial_orientation: Compass, rotation_direction: bool, include_center: bool, include_corners: bool) -> impl Iterator<Item = (MapPoint<X, Y>, Compass)> {
        NeighborIter::new(*self, initial_orientation, rotation_direction, include_center, include_corners)
    }
    fn iter_orientation(&self, orientation: Compass) -> impl Iterator<Item = MapPoint<X, Y>> {
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

impl<const X: usize, const Y: usize>NeighborIter<X, Y> {
    fn new(center_point: MapPoint<X, Y>, initial_orientation: Compass, rotation_direction: bool, include_center: bool, include_corners: bool) -> Self {
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
                self.current_orientation.counterclockwise().counterclockwise()
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
                self.center_point.neighbor(self.current_orientation).map_or(None, |n| Some((n, self.current_orientation)))
            };
            match result {
                Some(map_point) => {
                    self.rotate_orientation();
                    return Some(map_point);
                },
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

impl <const X: usize, const Y: usize>OrientationIter<X, Y> {
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
            return None
        }
        let result = self.current_point;
        match self.current_point.neighbor(self.orientation) {
            Some(map_point) => self.current_point = map_point,
            None => self.finished = true,
        }
        Some(result)
    }
}
//⏫my_map_point.rs
//⏬lib.rs
// lib.rs - sample lib file for local crate
mod action;


use std::fmt;

const X: usize = 19;
const Y: usize = X;
const N: usize = X * Y;

#[derive(Copy, Clone, PartialEq)]
enum Value {
    Free,
    White,
    Black,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Free => write!(f, "Free"),
            Value::White => write!(f, "White"),
            Value::Black => write!(f, "Black"),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Free
    }
}

struct Go {
    board: MyMap2D<Value, X, Y, N>,
}

impl Go {
    fn new() -> Self {
        Go {
            board: MyMap2D::<Value, X, Y, N>::default(),
        }
    }
    fn apply_action(&mut self, action: Action) {
        self.board.set(action.cell, action.value);
    }
}
//⏫lib.rs
//⏬action.rs
use super::*;

struct Action {
    cell: MapPoint<X, Y>,
    value: Value,
}

impl Action {
    fn set_white(cell: MapPoint<X, Y>) -> Self {
        Action {
            cell,
            value: Value::White,
        }
    }
    fn set_black(cell: MapPoint<X, Y>) -> Self {
        Action {
            cell,
            value: Value::Black,
        }
    }
}
//⏫action.rs
//⏬main.rs
// main.rs - main test input file



fn main() {
    let mut my_go = Go::new();
    let action = Action::set_white(MapPoint::<X, Y>::new(0, 0));
    my_go.apply_action(action);
}
//⏫main.rs