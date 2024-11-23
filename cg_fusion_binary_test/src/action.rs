use super::*;
use cg_fusion_lib_test::my_map_two_dim::my_map_point::*;

pub struct Action {
    pub cell: MapPoint<X, Y>,
    pub value: Value,
}

impl Action {
    pub fn set_white(cell: MapPoint<X, Y>) -> Self {
        Action {
            cell,
            value: Value::White,
        }
    }
    pub fn set_black(cell: MapPoint<X, Y>) -> Self {
        Action {
            cell,
            value: Value::Black,
        }
    }
}
