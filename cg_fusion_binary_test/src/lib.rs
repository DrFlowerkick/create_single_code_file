// lib.rs - sample lib file for local crate
pub mod action;

use crate::action::*;
use cg_fusion_lib_test::my_map_two_dim::*;

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

impl Default for crate::Go {
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
