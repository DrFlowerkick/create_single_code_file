#[derive(Clone, Copy, PartialEq, Default)]
pub enum Compass {
    #[default]
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
    Center,
}

impl Compass {
    pub fn flip(&self) -> Self {
        match self {
            Compass::N => Compass::S,
            Compass::NE => Compass::SW,
            Compass::E => Compass::W,
            Compass::SE => Compass::NW,
            Compass::S => Compass::N,
            Compass::SW => Compass::NE,
            Compass::W => Compass::E,
            Compass::NW => Compass::SE,
            Compass::Center => Compass::Center,
        }
    }
    pub fn clockwise(&self) -> Self {
        match self {
            Compass::N => Compass::NE,
            Compass::NE => Compass::E,
            Compass::E => Compass::SE,
            Compass::SE => Compass::S,
            Compass::S => Compass::SW,
            Compass::SW => Compass::W,
            Compass::W => Compass::NW,
            Compass::NW => Compass::N,
            Compass::Center => Compass::Center,
        }
    }
    pub fn counterclockwise(&self) -> Self {
        match self {
            Compass::N => Compass::NW,
            Compass::NW => Compass::W,
            Compass::W => Compass::SW,
            Compass::SW => Compass::S,
            Compass::S => Compass::SE,
            Compass::SE => Compass::E,
            Compass::E => Compass::NE,
            Compass::NE => Compass::N,
            Compass::Center => Compass::Center,
        }
    }
    pub fn is_cardinal(&self) -> bool {
        matches!(self, Compass::N | Compass::E | Compass::S | Compass::W)
    }
    pub fn is_ordinal(&self) -> bool {
        matches!(self, Compass::NE | Compass::SE | Compass::SW | Compass::NW)
    }
    pub fn is_center(&self) -> bool {
        *self == Compass::Center
    }
}
