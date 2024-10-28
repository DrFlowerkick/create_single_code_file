// main.rs - main test input file
use csf_cg_binary_test::{Go, X, Y};
use csf_cg_binary_test::action::Action;
use csf_cg_lib_test::my_map_two_dim::my_map_point::*;



fn main() {
    let mut my_go = Go::new();
    let action = Action::set_white(MapPoint::<X, Y>::new(0, 0));
    my_go.apply_action(action);
}