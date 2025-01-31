use super::*;
use crate::fmt;
use crate::fmt::Display;
use cg_fusion_lib_test::my_map_two_dim::my_map_point::MapPoint;

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
    pub fn set_black(cell: MapPoint<X, Y>) -> Self {
        Action {
            cell,
            value: Value::Black,
        }
    }
}
