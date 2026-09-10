#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub const fn moved(current: usize, direction: Direction, columns: usize, count: usize) -> usize {
    let next = match direction {
        Direction::Left => current.checked_sub(1),
        Direction::Right => current.checked_add(1),
        Direction::Up => current.checked_sub(columns),
        Direction::Down => current.checked_add(columns),
    };
    match next {
        Some(next) if next < count => next,
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn movement_matches_linear_reference_without_edge_wrap() {
        assert_eq!(moved(2, Direction::Right, 3, 6), 3);
        assert_eq!(moved(3, Direction::Left, 3, 6), 2);
        assert_eq!(moved(0, Direction::Up, 3, 6), 0);
        assert_eq!(moved(1, Direction::Down, 3, 5), 4);
        assert_eq!(moved(2, Direction::Down, 3, 5), 2);
        assert_eq!(moved(5, Direction::Right, 3, 6), 5);
    }
}
