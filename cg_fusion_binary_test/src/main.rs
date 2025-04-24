// main.rs - main test input file
use cg_fusion_binary_test::action::Action;
use cg_fusion_binary_test::{Go, X, Y};
use cg_fusion_lib_test::my_map_two_dim::my_map_point::*;

fn main() {
    let mut my_go = Go::default();
    let action = Action::set_white(MapPoint::<X, Y>::new(0, 0));
    my_go.apply_action(action);
}
