//! What a manager screen shows, and the cursors kept between visits.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Manager,
    Transfer,
    Gym,
}

/// What the left and right keys go through, and the gym buys for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyMode {
    Player,
    Group,
    Team,
    Market,
}

impl BuyMode {
    pub fn last_index(self) -> usize {
        match self {
            BuyMode::Player => 11,
            BuyMode::Group => 3,
            BuyMode::Team => 0,
            BuyMode::Market => 17,
        }
    }

    /// How many players an upgrade is bought for.
    pub fn cost_multiplier(self) -> u16 {
        match self {
            BuyMode::Player | BuyMode::Market => 1,
            BuyMode::Group => 3,
            BuyMode::Team => 12,
        }
    }
}

/// Shown instead of the upgrade's name after a purchase fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    NotEnoughCash,
    MaximumBoost,
}

/// The screens' cursors and choices. The original kept them in globals,
/// so they carry over from one visit to the next.
#[derive(Clone, Debug)]
pub struct Cursors {
    pub keypad: (usize, usize),
    pub grid: (usize, usize),
    /// 0, or 3 when the table has scrolled to show the substitutes.
    pub table_offset: usize,
    pub upgrade: (usize, usize),
    pub selected_upgrade: usize,
    /// The upgrade fire last chose: choosing it again buys it.
    pub(super) previous_selection: Option<usize>,
}

impl Default for Cursors {
    fn default() -> Self {
        Self {
            keypad: (1, 0),
            grid: (0, 0),
            table_offset: 0,
            upgrade: (0, 0),
            selected_upgrade: 0,
            previous_selection: None,
        }
    }
}

/// Everything one frame of a manager screen shows.
pub struct View {
    pub screen: Screen,
    pub mode: BuyMode,
    /// Which player, group or market player is shown.
    pub index: usize,
    pub key_highlight: Option<(usize, usize)>,
    pub key_pressed: bool,
    pub grid: (usize, usize),
    pub table_offset: usize,
    /// The lit upgrade button, while the joystick is on them.
    pub upgrade_cursor: Option<usize>,
    pub selected_upgrade: usize,
    pub message: Option<Message>,
}

impl View {
    /// A fresh view of `screen`, with the cursors where they were left.
    pub fn new(cursors: &Cursors, screen: Screen, mode: BuyMode) -> Self {
        Self {
            screen,
            mode,
            index: 0,
            key_highlight: None,
            key_pressed: false,
            grid: cursors.grid,
            table_offset: cursors.table_offset,
            upgrade_cursor: None,
            selected_upgrade: cursors.selected_upgrade,
            message: None,
        }
    }

    /// The chosen squad place, counting across the rows.
    pub fn grid_slot(&self) -> usize {
        self.grid.1 * 3 + self.grid.0
    }

    /// The squad table's position: it moves up to show the substitutes.
    pub fn table_y(&self) -> i32 {
        if self.table_offset == 0 {
            0x24
        } else {
            -0xC
        }
    }

    /// Shows another of the current mode's players or groups, wrapping
    /// round (the "previous" and "next" keys).
    pub fn step_index(&mut self, forward: bool) {
        let last = self.mode.last_index();
        self.index = if forward {
            if self.index >= last {
                0
            } else {
                self.index + 1
            }
        } else {
            self.index.checked_sub(1).unwrap_or(last)
        };
        self.message = None;
    }

    /// Switches what the left and right keys go through, back to the first.
    pub fn switch_mode(&mut self, mode: BuyMode) {
        self.mode = mode;
        self.index = 0;
        self.message = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_wraps_round_the_mode() {
        let mut view = View::new(&Cursors::default(), Screen::Gym, BuyMode::Group);
        view.step_index(false);
        assert_eq!(view.index, 3);
        view.step_index(true);
        assert_eq!(view.index, 0);
        view.switch_mode(BuyMode::Team);
        view.step_index(true);
        assert_eq!(view.index, 0, "one team");
    }
}
