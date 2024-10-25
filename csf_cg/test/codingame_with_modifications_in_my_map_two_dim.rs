//⏬my_map_two_dim.rs
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
    // A CHANGE WHICH WILL BE REMOVED
    fn set(&mut self, coordinates: MapPoint<X, Y>, value: T) -> &T {
        self.items[coordinates.y()][coordinates.x()] = value;
        &self.items[coordinates.y()][coordinates.x()]
    }
}
impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Default for MyMap2D<T, X, Y, N> {
    fn default() -> Self {
        Self::new()
    }
}
//⏫my_map_two_dim.rs
//⏬my_map_point.rs
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
    fn is_in_map(&self) -> bool {
        self.x < X && self.y < Y
    }
}
//⏫my_map_point.rs
//⏬lib.rs
use std::fmt;
const X: usize = 19;
const Y: usize = X;
const N: usize = X * Y;
#[derive(Copy, Clone, PartialEq)]
enum Value {
    Free,
    White,
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Free => write!(f, "Free"),
            Value::White => write!(f, "White"),
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
}
//⏫action.rs
//⏬main.rs
fn main() {
    let mut my_go = Go::new();
    let action = Action::set_white(MapPoint::<X, Y>::new(0, 0));
    my_go.apply_action(action);
}
// A CHANGE WHICH WILL BE REMOVED
//⏫main.rs