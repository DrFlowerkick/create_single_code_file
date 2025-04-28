pub mod my_map_point;

use self::my_map_point::*;
use my_array::*;
use my_map_point::my_compass::*;

// type definition for is_cell_free_fn,
// use is_cell_free_fn as follows (use "_" for unused variables):
// let is_cell_free_fn = Box::new(|current_point: MapPoint<X, Y>, current_cell_value: &T| current_point.use_it_somehow() || current_cell_value.use_it_somehow() );
pub type IsCellFreeFn<const X: usize, const Y: usize, T> = Box<dyn Fn(MapPoint<X, Y>, &T) -> bool>;

// use MyMap2D if compilation time is suffice, because it is more efficient and has cleaner interface
#[derive(Copy, Clone, PartialEq)]
pub struct MyMap2D<T, const X: usize, const Y: usize, const N: usize> {
    // X: number of columns, Y: number of rows, N: number of elements in map: X * Y
    items: [[T; X]; Y], //outer array rows, inner array columns -> first index chooses row (y), second index chooses column (x)
}

impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize>
    MyMap2D<T, X, Y, N>
{
    pub fn new() -> Self {
        if X == 0 {
            panic!("line {}, minimum one column", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum one row", line!());
        }
        Self {
            items: [[T::default(); X]; Y],
        }
    }
    pub fn init(init_element: T) -> Self {
        if X == 0 {
            panic!("line {}, minimum one column", line!());
        }
        if Y == 0 {
            panic!("line {}, minimum one row", line!());
        }
        Self {
            items: [[init_element; X]; Y],
        }
    }
    pub fn get(&self, coordinates: MapPoint<X, Y>) -> &T {
        &self.items[coordinates.y()][coordinates.x()]
    }
    pub fn get_mut(&mut self, coordinates: MapPoint<X, Y>) -> &mut T {
        &mut self.items[coordinates.y()][coordinates.x()]
    }
    pub fn set(&mut self, coordinates: MapPoint<X, Y>, value: T) -> &T {
        self.items[coordinates.y()][coordinates.x()] = value;
        &self.items[coordinates.y()][coordinates.x()]
    }
    pub fn is_cut_off_cell(
        &self,
        map_point: MapPoint<X, Y>,
        is_cell_free_fn: IsCellFreeFn<X, Y, T>,
    ) -> bool {
        let (mut last_free, initial_orientation) = match map_point.map_position() {
            Compass::NW | Compass::N => (false, Compass::E),
            Compass::NE | Compass::E => (false, Compass::S),
            Compass::SE | Compass::S => (false, Compass::W),
            Compass::SW | Compass::W => (false, Compass::N),
            Compass::Center => {
                let nw = map_point.neighbor(Compass::NW).unwrap();
                (is_cell_free_fn(nw, self.get(nw)), Compass::N)
            }
        };
        let mut free_zones = 0;
        for (is_free, is_side) in map_point
            .iter_neighbors(initial_orientation, true, false, true)
            .map(|(p, o)| (is_cell_free_fn(p, self.get(p)), o.is_cardinal()))
        {
            if !last_free && is_free && is_side {
                // new free zones start always at a side of map_point, since movement over corners is not allowed
                free_zones += 1;
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
    pub fn iter(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        self.items.iter().enumerate().flat_map(|(y, row)| {
            row.iter()
                .enumerate()
                .map(move |(x, column)| (MapPoint::<X, Y>::new(x, y), column))
        })
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (MapPoint<X, Y>, &mut T)> {
        self.items.iter_mut().enumerate().flat_map(|(y, row)| {
            row.iter_mut()
                .enumerate()
                .map(move |(x, column)| (MapPoint::<X, Y>::new(x, y), column))
        })
    }
    pub fn iter_row(&self, r: usize) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        if r >= Y {
            panic!("line {}, row index is out of range", line!());
        }
        self.items
            .iter()
            .enumerate()
            .filter(move |(y, _)| *y == r)
            .flat_map(|(y, row)| {
                row.iter()
                    .enumerate()
                    .map(move |(x, column)| (MapPoint::new(x, y), column))
            })
    }
    pub fn iter_column(&self, c: usize) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        if c >= X {
            panic!("line {}, column index is out of range", line!());
        }
        self.items.iter().enumerate().flat_map(move |(y, row)| {
            row.iter()
                .enumerate()
                .filter(move |(x, _)| *x == c)
                .map(move |(x, column)| (MapPoint::new(x, y), column))
        })
    }
    pub fn iter_neighbors(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        center_point
            .iter_neighbors(Compass::N, true, false, false)
            .map(move |(p, _)| (p, self.get(p)))
    }
    pub fn iter_neighbors_mut(
        &mut self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &mut T)> {
        center_point
            .iter_neighbors(Compass::N, true, false, false)
            .map(move |(p, _)| unsafe { (p, &mut *(self.get_mut(p) as *mut _)) })
    }
    pub fn iter_neighbors_with_center(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        center_point
            .iter_neighbors(Compass::N, true, true, false)
            .map(move |(p, _)| (p, self.get(p)))
    }
    pub fn iter_neighbors_with_corners(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T, bool)> {
        center_point
            .iter_neighbors(Compass::N, true, false, true)
            .map(move |(p, o)| (p, self.get(p), o.is_ordinal()))
    }
    pub fn iter_neighbors_with_center_and_corners(
        &self,
        center_point: MapPoint<X, Y>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T, bool)> {
        center_point
            .iter_neighbors(Compass::N, true, true, true)
            .map(move |(p, o)| (p, self.get(p), o.is_ordinal()))
    }
    pub fn iter_orientation(
        &self,
        start_point: MapPoint<X, Y>,
        orientation: Compass,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        start_point
            .iter_orientation(orientation)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_top_left(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(0, 0)
            .iter_orientation(Compass::SE)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_top_right(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(X - 1, 0)
            .iter_orientation(Compass::SW)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_bottom_left(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(0, Y - 1)
            .iter_orientation(Compass::NE)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_diagonal_bottom_right(&self) -> impl Iterator<Item = (MapPoint<X, Y>, &T)> {
        MapPoint::<X, Y>::new(X - 1, Y - 1)
            .iter_orientation(Compass::NW)
            .map(move |p| (p, self.get(p)))
    }
    pub fn iter_distance(
        &self,
        start_point: MapPoint<X, Y>,
        filter_fn: FilterFn<X, Y, T>,
    ) -> impl Iterator<Item = (MapPoint<X, Y>, &'_ T, usize)> {
        // use filter_fn as follows (use "_" for unused variables):
        // let filter_fn = Box::new(|point_of_next_cell: MapPoint<X, Y>, value_of_next_cell: &T, current_distance: usize| current_point.use_it_somehow() || current_cell_value.use_it_somehow() || current_distance.use_it_somehow());
        DistanceIter::new(self, start_point, filter_fn)
    }
}

impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Default
    for MyMap2D<T, X, Y, N>
{
    fn default() -> Self {
        Self::new()
    }
}

use std::fmt::Display;

impl<T: Copy + Clone + Default + Display, const X: usize, const Y: usize, const N: usize> Display
    for MyMap2D<T, X, Y, N>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut line = String::new();
        for (p, v) in self.iter() {
            line = format!("{}{}", line, v);
            if (p.x() + 1) % X == 0 && !line.is_empty() {
                writeln!(f, "{}", line)?;
                line = "".into();
            }
        }
        Ok(())
    }
}

// type definition for filter_fn
// input for filter_fn: next possible point, data from data_map of next possible point, distance of current point
pub type FilterFn<const X: usize, const Y: usize, T> =
    Box<dyn Fn(MapPoint<X, Y>, &T, usize) -> bool>;

struct DistanceIter<'a, T, const X: usize, const Y: usize, const N: usize> {
    data_map: &'a MyMap2D<T, X, Y, N>,
    filter_fn: FilterFn<X, Y, T>,
    next_cells: MyArray<(MapPoint<X, Y>, usize), N>,
    index: usize,
}

impl<'a, T: Copy + Clone, const X: usize, const Y: usize, const N: usize>
    DistanceIter<'a, T, X, Y, N>
{
    fn new(
        data_map: &'a MyMap2D<T, X, Y, N>,
        start_point: MapPoint<X, Y>,
        filter_fn: FilterFn<X, Y, T>,
    ) -> Self {
        DistanceIter {
            data_map,
            filter_fn,
            next_cells: MyArray::init((start_point, 0), 1),
            index: 0,
        }
    }
}

impl<'a, T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Iterator
    for DistanceIter<'a, T, X, Y, N>
{
    type Item = (MapPoint<X, Y>, &'a T, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.next_cells.len() {
            return None;
        }
        let (map_point, distance) = *self.next_cells.get(self.index).unwrap();
        let mut local_next_cells: MyArray<(MapPoint<X, Y>, usize), 4> = MyArray::new();
        for (next_cell, _) in self.data_map.iter_neighbors(map_point).filter(|(p, c)| {
            !self.next_cells.iter().any(|(n, _)| n == p) && (self.filter_fn)(*p, *c, distance)
        }) {
            local_next_cells.push((next_cell, distance + 1));
        }
        self.next_cells.append_slice(local_next_cells.as_slice());
        self.index += 1;
        Some((map_point, self.data_map.get(map_point), distance))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_cut_off() {
        const X: usize = 20;
        const Y: usize = 10;
        const N: usize = X * Y;

        let mut cut_off_map: MyMap2D<bool, X, Y, N> = MyMap2D::new();
        let mut game_map: MyMap2D<bool, X, Y, N> = MyMap2D::init(true);
        game_map.set(MapPoint::<X, Y>::new(1, 1), false);
        for (p, _) in game_map.iter().filter(|(_, c)| **c) {
            let is_cell_free_fn = Box::new(|_: MapPoint<X, Y>, c: &bool| *c);
            *cut_off_map.get_mut(p) = game_map.is_cut_off_cell(p, is_cell_free_fn);
        }
        assert_eq!(cut_off_map.iter().filter(|(_, c)| **c == true).count(), 5);

        game_map.set(MapPoint::<X, Y>::new(8, 2), false);
        for (p, _) in game_map.iter().filter(|(_, c)| **c) {
            let is_cell_free_fn = Box::new(|_: MapPoint<X, Y>, c: &bool| *c);
            *cut_off_map.get_mut(p) = game_map.is_cut_off_cell(p, is_cell_free_fn);
        }
        assert_eq!(cut_off_map.iter().filter(|(_, c)| **c == true).count(), 5);

        game_map.set(MapPoint::<X, Y>::new(7, 4), false);
        for (p, _) in game_map.iter().filter(|(_, c)| **c) {
            let is_cell_free_fn = Box::new(|_: MapPoint<X, Y>, c: &bool| *c);
            *cut_off_map.get_mut(p) = game_map.is_cut_off_cell(p, is_cell_free_fn);
        }
        assert_eq!(cut_off_map.iter().filter(|(_, c)| **c == true).count(), 7);

        game_map.set(MapPoint::<X, Y>::new(7, 6), false);
        for (p, _) in game_map.iter().filter(|(_, c)| **c) {
            let is_cell_free_fn = Box::new(|_: MapPoint<X, Y>, c: &bool| *c);
            *cut_off_map.get_mut(p) = game_map.is_cut_off_cell(p, is_cell_free_fn);
        }
        assert_eq!(cut_off_map.iter().filter(|(_, c)| **c == true).count(), 10);

        game_map.set(MapPoint::<X, Y>::new(9, 8), false);
        for (p, _) in game_map.iter().filter(|(_, c)| **c) {
            let is_cell_free_fn = Box::new(|_: MapPoint<X, Y>, c: &bool| *c);
            *cut_off_map.get_mut(p) = game_map.is_cut_off_cell(p, is_cell_free_fn);
        }
        assert_eq!(cut_off_map.iter().filter(|(_, c)| **c == true).count(), 14);
        assert!(*cut_off_map.get(MapPoint::<X, Y>::new(8, 7)));
    }
}
