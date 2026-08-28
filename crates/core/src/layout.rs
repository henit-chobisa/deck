//! How many panes fit beside each other.
//!
//! One sentence covers all of it:
//!
//! > Deck fills panes in the direction you asked for, then wraps — never
//! > narrower than `min_pane_width`, never more than `max_columns`.
//!
//! The two guards are not preferences, they are the reason the rule can be
//! stated so simply. Without a width floor, a fourth column on a laptop slices
//! code into unreadable ribbons; without a column cap, a wide monitor spreads
//! two refs so far apart that the pairing they were meant to show is lost.

use serde::{Deserialize, Serialize};

/// Which direction panes fill before they wrap.
///
/// The names describe the *result*, not the mechanism. "Prefer rows" cannot be
/// read off the page unambiguously, and "vertical split" means opposite things
/// in vim and tmux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arrange {
    /// One full-width pane per row. The default: a single readable column of
    /// code is worth more than two cramped ones.
    #[default]
    Stacked,
    /// Side by side, as far as the width allows.
    Columns,
    /// Squared off — the shape that keeps four refs on screen at once.
    Grid,
}

/// The layout settings, as they appear under `[layout]` in the config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// Which direction panes fill before wrapping.
    pub arrange: Arrange,
    /// The most columns to use, however wide the window gets.
    pub max_columns: u32,
    /// The narrowest a pane may be. A column is dropped rather than go under
    /// this.
    pub min_pane_width: u32,
    /// The most panes on one page. Refs past this become the next page.
    pub max_panes: u32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            arrange: Arrange::Stacked,
            max_columns: 3,
            min_pane_width: 60,
            max_panes: 4,
        }
    }
}

/// How the panes on one page are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    /// Panes across. At least one.
    pub cols: u32,
    /// Rows needed to hold them all. At least one.
    pub rows: u32,
}

impl Layout {
    /// The grid for `panes` panes in a window `width` wide.
    ///
    /// `width` is in whatever unit `min_pane_width` is given in — columns for a
    /// terminal, pixels for a window. The two only ever have to agree with each
    /// other.
    #[must_use]
    pub fn grid(&self, panes: u32, width: u32) -> Grid {
        let panes = panes.max(1);

        let wanted = match self.arrange {
            Arrange::Stacked => 1,
            Arrange::Columns => panes,
            // Biased toward rows: nine refs go 3x3, but six go 2x3 rather than
            // 3x2, so a tall narrow window keeps its panes readable.
            Arrange::Grid => panes.isqrt(),
        };

        // Whichever guard bites hardest wins, and one column always survives —
        // a pane too narrow to read still beats no pane at all.
        let by_width = width / self.min_pane_width.max(1);
        let cols = wanted
            .min(self.max_columns.max(1))
            .min(by_width)
            .min(panes)
            .max(1);

        Grid {
            cols,
            rows: panes.div_ceil(cols),
        }
    }

    /// The same settings, for a page that has a picture on it.
    ///
    /// A diagram is wide and short where a file is narrow and tall. Stacked,
    /// the two fight: the picture takes a full row's height to draw two rows of
    /// boxes and leaves the rest of it empty, and the code beneath it is given
    /// half a window to show a file in. Side by side each one gets the shape it
    /// wants, and the pairing the group was making is on screen at once —
    /// which is the whole reason a diagram was drawn beside code rather than
    /// put in the prose.
    ///
    /// Only [`Arrange::Stacked`] moves, and only because it is the default
    /// rather than an answer. An arrangement the reader asked for outright is
    /// left alone: `max_columns` and `min_pane_width` still decide how many
    /// columns there actually are, so a narrow window drops back to one and
    /// nothing is squeezed.
    #[must_use]
    pub fn beside_pictures(self) -> Self {
        Self {
            arrange: match self.arrange {
                Arrange::Stacked => Arrange::Columns,
                asked_for => asked_for,
            },
            ..self
        }
    }

    /// The refs for each page, in order.
    ///
    /// A group with more refs than `max_panes` becomes several pages rather
    /// than one unreadable one. The reader walks pages; the group is still one
    /// claim.
    pub fn pages<'a, T>(&self, refs: &'a [T]) -> std::slice::Chunks<'a, T> {
        refs.chunks(self.max_panes.max(1) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window wide enough that `min_pane_width` never bites.
    const WIDE: u32 = 4000;

    #[test]
    fn stacked_gives_one_pane_per_row() {
        let layout = Layout::default();
        assert_eq!(layout.grid(1, WIDE), Grid { cols: 1, rows: 1 });
        assert_eq!(layout.grid(4, WIDE), Grid { cols: 1, rows: 4 });
    }

    #[test]
    fn columns_fills_sideways_until_a_guard_stops_it() {
        let layout = Layout {
            arrange: Arrange::Columns,
            max_columns: 3,
            ..Layout::default()
        };
        assert_eq!(layout.grid(2, WIDE), Grid { cols: 2, rows: 1 });
        // Four panes, capped at three columns, so the fourth wraps.
        assert_eq!(layout.grid(4, WIDE), Grid { cols: 3, rows: 2 });
    }

    #[test]
    fn a_grid_is_biased_toward_rows() {
        let layout = Layout {
            arrange: Arrange::Grid,
            max_columns: 9,
            ..Layout::default()
        };
        assert_eq!(layout.grid(4, WIDE), Grid { cols: 2, rows: 2 });
        assert_eq!(layout.grid(9, WIDE), Grid { cols: 3, rows: 3 });
        // Six panes go 2x3, not 3x2: rows are cheaper than narrow columns.
        assert_eq!(layout.grid(6, WIDE), Grid { cols: 2, rows: 3 });
    }

    #[test]
    fn a_narrow_window_drops_columns_rather_than_squeeze_them() {
        let layout = Layout {
            arrange: Arrange::Grid,
            max_columns: 9,
            min_pane_width: 60,
            ..Layout::default()
        };
        // 200 wide holds three 60-wide panes; 130 holds two; 100 holds one.
        assert_eq!(layout.grid(9, 200).cols, 3);
        assert_eq!(layout.grid(9, 130).cols, 2);
        assert_eq!(layout.grid(9, 100).cols, 1);
    }

    #[test]
    fn a_window_too_narrow_for_even_one_pane_still_gets_one() {
        let layout = Layout {
            arrange: Arrange::Columns,
            ..Layout::default()
        };
        assert_eq!(layout.grid(4, 10), Grid { cols: 1, rows: 4 });
    }

    #[test]
    fn there_are_never_more_columns_than_panes_to_put_in_them() {
        let layout = Layout {
            arrange: Arrange::Columns,
            max_columns: 8,
            ..Layout::default()
        };
        assert_eq!(layout.grid(2, WIDE), Grid { cols: 2, rows: 1 });
    }

    #[test]
    fn a_picture_on_the_page_turns_the_default_sideways() {
        let stacked = Layout::default();
        assert_eq!(stacked.grid(2, WIDE), Grid { cols: 1, rows: 2 });
        assert_eq!(
            stacked.beside_pictures().grid(2, WIDE),
            Grid { cols: 2, rows: 1 }
        );
    }

    #[test]
    fn an_arrangement_that_was_asked_for_is_left_alone() {
        // Stacked is a default, so it gives way. A grid is an answer, and a
        // picture is not a reason to overrule the reader.
        let grid = Layout {
            arrange: Arrange::Grid,
            ..Layout::default()
        };
        assert_eq!(grid.beside_pictures().arrange, Arrange::Grid);
    }

    #[test]
    fn a_narrow_window_beside_a_picture_still_drops_to_one_column() {
        // The guards outrank the preference: two cramped panes are worse than
        // one readable one, whatever is drawn on them.
        let layout = Layout::default().beside_pictures();
        assert_eq!(layout.grid(2, 80), Grid { cols: 1, rows: 2 });
    }

    #[test]
    fn a_group_with_too_many_refs_becomes_several_pages() {
        let layout = Layout {
            max_panes: 4,
            ..Layout::default()
        };
        let refs = [1, 2, 3, 4, 5, 6];
        let pages: Vec<&[i32]> = layout.pages(&refs).collect();

        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0], &[1, 2, 3, 4]);
        assert_eq!(pages[1], &[5, 6]);
    }

    #[test]
    fn a_config_that_says_zero_is_corrected_rather_than_dividing_by_it() {
        let layout = Layout {
            max_columns: 0,
            min_pane_width: 0,
            max_panes: 0,
            arrange: Arrange::Grid,
        };
        assert_eq!(layout.grid(4, WIDE), Grid { cols: 1, rows: 4 });
        assert_eq!(layout.pages(&[1, 2]).count(), 2);
    }

    #[test]
    fn the_defaults_are_what_the_config_file_documents() {
        let layout: Layout = serde_json::from_str("{}").unwrap();
        assert_eq!(layout.arrange, Arrange::Stacked);
        assert_eq!(layout.max_columns, 3);
        assert_eq!(layout.min_pane_width, 60);
        assert_eq!(layout.max_panes, 4);
    }
}
