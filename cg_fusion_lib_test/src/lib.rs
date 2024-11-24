// some sample code just for fun

// reexport crates in workspace
pub use my_array;
pub use my_map_two_dim;

// the following code just exists to have some dependencies
// it will be purposely not included in fusion
use my_array::MyArray;
use my_map_two_dim::{my_map_point::MapPoint, MyMap2D};
use rand::prelude::*;

// N = X * Y

pub struct FunkyData<T, const X: usize, const Y: usize, const N: usize> {
    array: MyArray<T, N>,
    map: MyMap2D<T, X, Y, N>,
}

impl<T: Copy + Clone + Default, const X: usize, const Y: usize, const N: usize>
    FunkyData<T, X, Y, N>
{
    pub fn random_mix(&mut self) {
        let mut rng = thread_rng();
        let (index, item) = self.array.iter().enumerate().choose(&mut rng).unwrap();
        //let item = *item;
        let y = index / X;
        let x = index % X;
        self.map.set(MapPoint::new(x, y), *item);
    }
}
