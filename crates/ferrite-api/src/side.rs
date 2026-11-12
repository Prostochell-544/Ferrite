use mod_api::Direction;
use serde::{Deserialize, Serialize};

/// The six sides in the engine's `Direction` order, the index [`side_index`] gives.
pub const SIDES: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub const fn side_index(side: Direction) -> usize {
    match side {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    }
}

/// The block next to `pos` through `side`.
pub const fn offset(pos: [i32; 3], side: Direction) -> [i32; 3] {
    let n = side.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

/// What a machine's side lets through, set per machine by the player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Io {
    #[default]
    None,
    In,
    Out,
    Both,
}

impl Io {
    pub const fn accepts(self) -> bool {
        matches!(self, Io::In | Io::Both)
    }

    pub const fn gives(self) -> bool {
        matches!(self, Io::Out | Io::Both)
    }

    const fn bits(self) -> i32 {
        match self {
            Io::None => 0,
            Io::In => 1,
            Io::Out => 2,
            Io::Both => 3,
        }
    }

    const fn from_bits(bits: i32) -> Io {
        match bits & 3 {
            1 => Io::In,
            2 => Io::Out,
            3 => Io::Both,
            _ => Io::None,
        }
    }
}

/// A machine's [`Io`] per side, kept in its block entity: as one int (`to_bits`) in its saved
/// data or a menu data slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SideConfig(pub [Io; 6]);

impl SideConfig {
    pub const fn all(io: Io) -> SideConfig {
        SideConfig([io; 6])
    }

    pub const fn get(&self, side: Direction) -> Io {
        self.0[side_index(side)]
    }

    pub fn set(&mut self, side: Direction, io: Io) {
        self.0[side_index(side)] = io;
    }

    /// Through `side`, or from no particular side (the machine itself, a menu): everything.
    pub const fn through(&self, side: Option<Direction>) -> Io {
        match side {
            Some(side) => self.get(side),
            None => Io::Both,
        }
    }

    /// Two bits a side, `Down` lowest.
    pub const fn to_bits(self) -> i32 {
        let mut bits = 0;
        let mut i = 0;
        while i < 6 {
            bits |= self.0[i].bits() << (2 * i);
            i += 1;
        }
        bits
    }

    pub const fn from_bits(bits: i32) -> SideConfig {
        let mut sides = [Io::None; 6];
        let mut i = 0;
        while i < 6 {
            sides[i] = Io::from_bits(bits >> (2 * i));
            i += 1;
        }
        SideConfig(sides)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip() {
        let mut sides = SideConfig::all(Io::In);
        sides.set(Direction::East, Io::Out);
        sides.set(Direction::Up, Io::Both);
        sides.set(Direction::North, Io::None);
        assert_eq!(SideConfig::from_bits(sides.to_bits()), sides);
        assert_eq!(sides.get(Direction::East), Io::Out);
        assert_eq!(sides.through(None), Io::Both);
    }

    #[test]
    fn indices_follow_sides() {
        for (i, side) in SIDES.into_iter().enumerate() {
            assert_eq!(side_index(side), i);
        }
        assert_eq!(offset([0, 64, 0], Direction::North), [0, 64, -1]);
    }
}
