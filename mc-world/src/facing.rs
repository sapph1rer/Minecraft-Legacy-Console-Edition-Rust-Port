//! Port of `Facing` — the six block faces.

pub const DOWN: i32 = 0;
pub const UP: i32 = 1;
pub const NORTH: i32 = 2;
pub const SOUTH: i32 = 3;
pub const WEST: i32 = 4;
pub const EAST: i32 = 5;

pub const OPPOSITE_FACING: [i32; 6] = [UP, DOWN, SOUTH, NORTH, EAST, WEST];
pub const STEP_X: [i32; 6] = [0, 0, 0, 0, -1, 1];
pub const STEP_Y: [i32; 6] = [-1, 1, 0, 0, 0, 0];
pub const STEP_Z: [i32; 6] = [0, 0, -1, 1, 0, 0];
pub const NAMES: [&str; 6] = ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_is_involution_and_steps_cancel() {
        for f in 0..6usize {
            let o = OPPOSITE_FACING[f] as usize;
            assert_eq!(OPPOSITE_FACING[o] as usize, f);
            assert_eq!(STEP_X[f] + STEP_X[o], 0);
            assert_eq!(STEP_Y[f] + STEP_Y[o], 0);
            assert_eq!(STEP_Z[f] + STEP_Z[o], 0);
        }
    }
}
