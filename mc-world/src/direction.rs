//! Port of `Direction` — the four horizontal directions (note: a different numbering than `Facing`).

use crate::facing;

pub const UNDEFINED: i32 = -1;
pub const SOUTH: i32 = 0;
pub const WEST: i32 = 1;
pub const NORTH: i32 = 2;
pub const EAST: i32 = 3;

pub const STEP_X: [i32; 4] = [0, -1, 0, 1];
pub const STEP_Z: [i32; 4] = [1, 0, -1, 0];
pub const NAMES: [&str; 4] = ["SOUTH", "WEST", "NORTH", "EAST"];

/// for \[direction] gives \[tile face]
pub const DIRECTION_FACING: [i32; 4] = [facing::SOUTH, facing::WEST, facing::NORTH, facing::EAST];
/// for \[facing] gives \[direction]
pub const FACING_DIRECTION: [i32; 6] = [UNDEFINED, UNDEFINED, NORTH, SOUTH, WEST, EAST];
pub const DIRECTION_OPPOSITE: [i32; 4] = [NORTH, EAST, SOUTH, WEST];
pub const DIRECTION_CLOCKWISE: [i32; 4] = [WEST, NORTH, EAST, SOUTH];
pub const DIRECTION_COUNTER_CLOCKWISE: [i32; 4] = [EAST, SOUTH, WEST, NORTH];

/// for \[direction]\[world facing] gives \[tile facing]
pub const RELATIVE_DIRECTION_FACING: [[i32; 6]; 4] = [
    [facing::UP, facing::DOWN, facing::SOUTH, facing::NORTH, facing::EAST, facing::WEST],
    [facing::UP, facing::DOWN, facing::EAST, facing::WEST, facing::NORTH, facing::SOUTH],
    [facing::UP, facing::DOWN, facing::NORTH, facing::SOUTH, facing::WEST, facing::EAST],
    [facing::UP, facing::DOWN, facing::WEST, facing::EAST, facing::SOUTH, facing::NORTH],
];

/// `Direction::getDirection(double xd, double zd)`
pub fn get_direction(xd: f64, zd: f64) -> i32 {
    if (xd as f32).abs() > (zd as f32).abs() {
        if xd > 0.0 { WEST } else { EAST }
    } else if zd > 0.0 {
        NORTH
    } else {
        SOUTH
    }
}

/// `Direction::getDirection(int x0, int z0, int x1, int z1)`
pub fn get_direction_between(x0: i32, z0: i32, x1: i32, z1: i32) -> i32 {
    get_direction((x0 - x1) as f64, (z0 - z1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_consistent() {
        for d in 0..4usize {
            assert_eq!(DIRECTION_OPPOSITE[DIRECTION_OPPOSITE[d] as usize], d as i32);
            assert_eq!(DIRECTION_CLOCKWISE[DIRECTION_COUNTER_CLOCKWISE[d] as usize], d as i32);
            // facing <-> direction round trip
            assert_eq!(FACING_DIRECTION[DIRECTION_FACING[d] as usize], d as i32);
            // the step vectors agree with the facing step vectors
            let f = DIRECTION_FACING[d] as usize;
            assert_eq!(STEP_X[d], facing::STEP_X[f]);
            assert_eq!(STEP_Z[d], facing::STEP_Z[f]);
        }
    }

    #[test]
    fn get_direction_quadrants() {
        assert_eq!(get_direction(5.0, 1.0), WEST);
        assert_eq!(get_direction(-5.0, 1.0), EAST);
        assert_eq!(get_direction(1.0, 5.0), NORTH);
        assert_eq!(get_direction(1.0, -5.0), SOUTH);
        // |xd| == |zd| falls to the z branch
        assert_eq!(get_direction(3.0, 3.0), NORTH);
        assert_eq!(get_direction_between(0, 0, 10, 0), EAST);
    }
}
