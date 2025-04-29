// test linking of traits over file borders

use super::{Go, Value, fmt};

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Free => write!(f, "Free"),
            Value::White => write!(f, "White"),
            Value::Black => write!(f, "Black"),
        }
    }
}

impl fmt::Display for Go {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Go {{ board: {} }}", self.board)
    }
}
