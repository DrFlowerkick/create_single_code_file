use crate::cg_fusion_binary_test::Go;
use crate::cg_fusion_binary_test::X;
use crate::cg_fusion_binary_test::Y;
use crate::cg_fusion_binary_test::action::Action;
use crate::my_map_two_dim::my_map_point::MapPoint;
fn main() {
    let mut my_go = Go::default();
    let action = Action::set_white(MapPoint::<X, Y>::new(0, 0));
    my_go.apply_action(action);
}
pub mod cg_fusion_binary_test {
    pub mod action {
        use super::Value;
        use super::X;
        use super::Y;
        use super::fmt;
        use super::fmt::Display;
        use crate::my_map_two_dim::my_map_point::MapPoint;
        pub struct Action {
            pub cell: MapPoint<X, Y>,
            pub value: Value,
        }
        impl Display for Action {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    f,
                    "Action: cell: '({},{}), value: {}",
                    self.cell.x(),
                    self.cell.y(),
                    self.value
                )
            }
        }
        impl Action {
            pub fn set_white(cell: MapPoint<X, Y>) -> Self {
                Action {
                    cell,
                    value: Value::White,
                }
            }
        }
    }
    use crate::my_map_two_dim::MyMap2D;
    use action::Action;
    use std::fmt;
    pub const X: usize = 19;
    pub const Y: usize = X;
    const N: usize = X * Y;
    #[derive(Copy, Clone, PartialEq, Default)]
    pub enum Value {
        #[default]
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
    pub struct Go {
        board: MyMap2D<Value, X, Y, N>,
    }
    impl Default for Go {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Go {
        pub fn new() -> Self {
            Go {
                board: MyMap2D::<Value, X, Y, N>::default(),
            }
        }
        pub fn apply_action(&mut self, action: Action) {
            self.board.set(action.cell, action.value);
        }
    }
}
pub mod my_map_two_dim {
    pub mod my_map_point {
        #[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
        pub struct MapPoint<const X: usize, const Y: usize> {
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
            pub fn is_in_map(&self) -> bool {
                self.x < X && self.y < Y
            }
        }
    }
    use my_map_point::MapPoint;
    #[derive(Copy, Clone, PartialEq)]
    pub struct MyMap2D<T, const X: usize, const Y: usize, const N: usize> {
        items: [[T; X]; Y],
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
        pub fn set(&mut self, coordinates: MapPoint<X, Y>, value: T) -> &T {
            self.items[coordinates.y()][coordinates.x()] = value;
            &self.items[coordinates.y()][coordinates.x()]
        }
    }
    impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize> Default
        for MyMap2D<T, X, Y, N>
    {
        fn default() -> Self {
            Self::new()
        }
    }
}
