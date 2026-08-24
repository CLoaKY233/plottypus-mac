#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Surface {
    #[default]
    Work,
    Glance,
}

impl Surface {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Work => Self::Glance,
            Self::Glance => Self::Work,
        }
    }
}

/// Work needs room for both columns: 24 process + 36 metrics.
pub const WORK_MIN_COLS: u16 = 60;
pub const WORK_MIN_ROWS: u16 = 16;

#[must_use]
pub const fn auto_surface(cols: u16, rows: u16) -> Surface {
    if cols < WORK_MIN_COLS || rows < WORK_MIN_ROWS {
        Surface::Glance
    } else {
        Surface::Work
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_terminal_is_glance() {
        assert_eq!(auto_surface(30, 40), Surface::Glance);
        assert_eq!(auto_surface(80, 10), Surface::Glance);
        assert_eq!(auto_surface(80, 15), Surface::Glance);
    }

    #[test]
    fn typical_terminal_is_work() {
        assert_eq!(auto_surface(80, 24), Surface::Work);
        assert_eq!(auto_surface(120, 30), Surface::Work);
        assert_eq!(auto_surface(60, 16), Surface::Work);
    }

    #[test]
    fn between_old_gate_and_two_columns_is_glance() {
        // 40..59 used to pick Work and panic in the layout clamp.
        for cols in 40..60 {
            assert_eq!(auto_surface(cols, 24), Surface::Glance, "cols {cols}");
        }
    }

    #[test]
    fn other_flips() {
        assert_eq!(Surface::Work.other(), Surface::Glance);
        assert_eq!(Surface::Glance.other(), Surface::Work);
    }
}
