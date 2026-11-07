//! Contains CellOccupancyMatrix used to track occupied cells during grid placement
use super::TrackCounts;
use crate::compute::grid::OriginZeroLine;
use crate::geometry::AbsoluteAxis;
use crate::geometry::Line;
use crate::util::sys::{new_vec_with_capacity, Vec};
use core::cmp::{max, min};
use core::fmt::Debug;
use core::ops::Range;
use smallvec::SmallVec;

/// The occupancy state of a single grid cell
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub(crate) enum CellOccupancyState {
    #[default]
    /// Indicates that a grid cell is unoccupied
    Unoccupied,
    /// Indicates that a grid cell is occupied by a definitely placed item
    DefinitelyPlaced,
    /// Indicates that a grid cell is occupied by an item that was placed by the auto placement algorithm
    AutoPlaced,
}

/// A run of occupied cells within a single track. The range is in OriginZero line coordinates
/// along the track (so for a row track, the range spans column lines and vice versa).
#[derive(Debug, Clone, PartialEq, Eq)]
struct OccupiedInterval {
    /// The range of cells covered by this interval (start line..end line in OriginZero coordinates)
    range: Range<i16>,
    /// The occupancy state of every cell within this interval
    state: CellOccupancyState,
}

impl OccupiedInterval {
    /// Whether this interval overlaps the given range
    fn overlaps(&self, range: &Range<i16>) -> bool {
        self.range.start < range.end && self.range.end > range.start
    }
}

/// The occupied cells of a single track, stored as a sorted list of disjoint intervals in
/// OriginZero line coordinates. Gaps between intervals are unoccupied. Touching intervals
/// with the same state are merged.
#[derive(Debug, Clone, Default)]
struct TrackIntervals {
    /// The sorted, disjoint list of occupied intervals within the track
    intervals: SmallVec<[OccupiedInterval; 2]>,
    /// The furthest end line of an auto-placed item starting in this secondary-axis track
    auto_placement_cursor: Option<OriginZeroLine>,
}

impl TrackIntervals {
    /// Whether the track contains any occupied cells
    fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    /// The occupancy state of the cell whose start line is at `coordinate`
    fn state_at(&self, coordinate: i16) -> CellOccupancyState {
        self.intervals
            .iter()
            .find(|interval| interval.range.start <= coordinate && coordinate < interval.range.end)
            .map(|interval| interval.state)
            .unwrap_or(CellOccupancyState::Unoccupied)
    }

    /// Set the cells covered by `range` to `state`, overwriting the state of any already-occupied
    /// cells within the range (matching the overwrite semantics of a dense matrix of cells)
    fn paint(&mut self, range: Range<i16>, state: CellOccupancyState) {
        if range.start >= range.end {
            return;
        }

        // Fast path: the painted range lies entirely after all existing intervals
        // (the common case, as placement queries and inserts mostly advance forwards)
        match self.intervals.last_mut() {
            None => {
                self.intervals.push(OccupiedInterval { range, state });
                return;
            }
            Some(last) if last.range.end <= range.start => {
                if last.state == state && last.range.end == range.start {
                    last.range.end = range.end;
                } else {
                    self.intervals.push(OccupiedInterval { range, state });
                }
                return;
            }
            _ => {}
        }

        let mut result: SmallVec<[OccupiedInterval; 2]> = SmallVec::new();

        // Intervals (or partial intervals) entirely before the painted range
        for interval in &self.intervals {
            if interval.range.end <= range.start {
                result.push(interval.clone());
            } else if interval.range.start < range.start {
                result.push(OccupiedInterval { range: interval.range.start..range.start, state: interval.state });
            }
        }

        // The painted range, merged with the preceding interval if it touches and has the same state
        match result.last_mut() {
            Some(last) if last.state == state && last.range.end == range.start => last.range.end = range.end,
            _ => result.push(OccupiedInterval { range: range.clone(), state }),
        }

        // Intervals (or partial intervals) entirely after the painted range
        for interval in &self.intervals {
            let trimmed = if interval.range.start >= range.end {
                interval.clone()
            } else if interval.range.end > range.end {
                OccupiedInterval { range: range.end..interval.range.end, state: interval.state }
            } else {
                continue;
            };
            match result.last_mut() {
                Some(last) if last.state == trimmed.state && last.range.end == trimmed.range.start => {
                    last.range.end = trimmed.range.end
                }
                _ => result.push(trimmed),
            }
        }

        self.intervals = result;
    }

    /// Find the extent of the occupied interval which an auto-placement search along the track
    /// would collide with last: the end of the last overlapping interval. Returns the extremal
    /// occupied cell of that interval (which may lie outside `range`: every search position
    /// before the returned extent also collides with the interval), or `None` if the range is
    /// entirely unoccupied.
    fn collision_extent(&self, range: &Range<i16>) -> Option<i16> {
        let interval = self
            .intervals
            .iter()
            .rev()
            .take_while(|interval| interval.range.end > range.start)
            .find(|interval| interval.overlaps(range))?;
        Some(interval.range.end - 1)
    }
}

/// A dynamically sized matrix (2d grid) which tracks the occupancy of each grid cell during auto-placement.
/// It also keeps tabs on how many tracks there are and which tracks are implicit and which are explicit.
///
/// Occupancy is stored sparsely as per-track interval lists (in both orientations), so memory usage
/// is proportional to the number of placed items rather than the total number of grid cells.
pub(crate) struct CellOccupancyMatrix {
    /// The counts of implicit and explicit columns
    columns: TrackCounts,
    /// The counts of implicit and explicit rows
    rows: TrackCounts,
    /// For each row track: the occupied intervals within that row (in column coordinates)
    row_intervals: Vec<TrackIntervals>,
    /// For each column track: the occupied intervals within that column (in row coordinates)
    column_intervals: Vec<TrackIntervals>,
}

/// Debug impl that represents the matrix in a compact 2d text format
impl Debug for CellOccupancyMatrix {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "Rows: neg_implicit={} explicit={} pos_implicit={}",
            self.rows.negative_implicit, self.rows.explicit, self.rows.positive_implicit
        )?;
        writeln!(
            f,
            "Cols: neg_implicit={} explicit={} pos_implicit={}",
            self.columns.negative_implicit, self.columns.explicit, self.columns.positive_implicit
        )?;
        if self.rows.len() > 100 || self.columns.len() > 100 {
            writeln!(f, "State: (not printed: more than 100 tracks)")?;
            return Ok(());
        }
        writeln!(f, "State:")?;

        for row in &self.row_intervals {
            for column_index in 0..self.columns.len() {
                let coordinate = self.columns.track_to_prev_oz_line(column_index as u16);
                let letter = match row.state_at(coordinate.0) {
                    CellOccupancyState::Unoccupied => '_',
                    CellOccupancyState::DefinitelyPlaced => 'D',
                    CellOccupancyState::AutoPlaced => 'A',
                };
                write!(f, "{letter}")?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

impl CellOccupancyMatrix {
    /// Create a CellOccupancyMatrix given a set of provisional track counts. The grid can expand as needed to fit more tracks,
    /// the provisional track counts represent a best effort attempt to avoid the extra allocations this requires.
    pub fn with_track_counts(columns: TrackCounts, rows: TrackCounts) -> Self {
        let mut row_intervals = new_vec_with_capacity(rows.len());
        row_intervals.resize(rows.len(), TrackIntervals::default());
        let mut column_intervals = new_vec_with_capacity(columns.len());
        column_intervals.resize(columns.len(), TrackIntervals::default());
        Self { rows, columns, row_intervals, column_intervals }
    }

    /// The per-track interval lists for tracks in the specified axis. Each row track's intervals
    /// are in column coordinates and vice versa.
    fn track_lists(&self, track_axis: AbsoluteAxis) -> &[TrackIntervals] {
        match track_axis {
            AbsoluteAxis::Horizontal => &self.column_intervals,
            AbsoluteAxis::Vertical => &self.row_intervals,
        }
    }

    /// Expands the grid (potentially in all 4 directions) in order to ensure that the specified
    /// spans (in OriginZero coordinates) fit within the tracked tracks
    fn expand_to_fit_range(&mut self, row_span: Line<OriginZeroLine>, col_span: Line<OriginZeroLine>) {
        // Calculate number of rows and columns missing to accommodate ranges (if any)
        let req_negative_rows = max(-(self.rows.negative_implicit as i16) - row_span.start.0, 0);
        let req_positive_rows = max(row_span.end.0 - self.rows.implicit_end_line().0, 0);
        let req_negative_cols = max(-(self.columns.negative_implicit as i16) - col_span.start.0, 0);
        let req_positive_cols = max(col_span.end.0 - self.columns.implicit_end_line().0, 0);

        // Add empty tracks to the front and/or back of the per-track interval lists.
        // Interval contents are stored in OriginZero coordinates, so they do not need to shift.
        if req_negative_rows > 0 {
            self.row_intervals
                .splice(0..0, core::iter::repeat_with(TrackIntervals::default).take(req_negative_rows as usize));
        }
        if req_positive_rows > 0 {
            let new_len = self.row_intervals.len() + req_positive_rows as usize;
            self.row_intervals.resize(new_len, TrackIntervals::default());
        }
        if req_negative_cols > 0 {
            self.column_intervals
                .splice(0..0, core::iter::repeat_with(TrackIntervals::default).take(req_negative_cols as usize));
        }
        if req_positive_cols > 0 {
            let new_len = self.column_intervals.len() + req_positive_cols as usize;
            self.column_intervals.resize(new_len, TrackIntervals::default());
        }

        self.rows.negative_implicit += req_negative_rows as u16;
        self.rows.positive_implicit += req_positive_rows as u16;
        self.columns.negative_implicit += req_negative_cols as u16;
        self.columns.positive_implicit += req_positive_cols as u16;
    }

    /// Mark an area of the matrix as occupied, expanding the allocated space as necessary to accommodate the passed area.
    pub fn mark_area_as(
        &mut self,
        primary_axis: AbsoluteAxis,
        primary_span: Line<OriginZeroLine>,
        secondary_span: Line<OriginZeroLine>,
        value: CellOccupancyState,
    ) {
        let (row_span, column_span) = match primary_axis {
            AbsoluteAxis::Horizontal => (secondary_span, primary_span),
            AbsoluteAxis::Vertical => (primary_span, secondary_span),
        };

        self.expand_to_fit_range(row_span, column_span);

        if value == CellOccupancyState::AutoPlaced
            && primary_span.start < primary_span.end
            && secondary_span.start < secondary_span.end
        {
            self.advance_auto_placement_cursor(primary_axis, secondary_span.start, primary_span.end);
        }

        let row_range = self.rows.oz_line_range_to_track_range(row_span);
        let col_range = self.columns.oz_line_range_to_track_range(column_span);
        for row_index in row_range {
            self.row_intervals[row_index as usize].paint(column_span.start.0..column_span.end.0, value);
        }
        for column_index in col_range {
            self.column_intervals[column_index as usize].paint(row_span.start.0..row_span.end.0, value);
        }
    }


    /// Checks the specified area for occupied cells (`primary_span` and `secondary_span` are
    /// bounding grid lines in OriginZero coordinates). Returns `None` if the area is entirely
    /// unoccupied. Otherwise returns the next search position (in OriginZero coordinates, along
    /// `primary_axis`) that is not guaranteed to collide with the occupied cells found in the
    /// area. This allows the auto-placement search cursor to jump past collisions rather than
    /// advancing one track at a time.
    pub fn line_area_collision_jump(
        &self,
        primary_axis: AbsoluteAxis,
        primary_span: Line<OriginZeroLine>,
        secondary_span: Line<OriginZeroLine>,
    ) -> Option<OriginZeroLine> {
        let track_lists = self.track_lists(primary_axis.other_axis());
        let secondary_counts = self.track_counts(primary_axis.other_axis());
        let secondary_range = secondary_counts.oz_line_range_to_track_range(secondary_span);

        // Out of bounds cells are considered unoccupied, so clamp the secondary range to the
        // tracks which actually exist
        let secondary_start = max(secondary_range.start, 0);
        let secondary_end = min(secondary_range.end, track_lists.len() as i16);

        let primary_range = primary_span.start.0..primary_span.end.0;

        let mut extent: Option<i16> = None;
        for secondary_index in secondary_start..secondary_end {
            let Some(cell) = track_lists[secondary_index as usize].collision_extent(&primary_range) else {
                continue;
            };
            extent = Some(match extent {
                None => cell,
                Some(best) => max(best, cell),
            });
        }

        extent.map(|cell| OriginZeroLine(cell + 1))
    }

    /// Given a span of tracks in `axis` (in OriginZero coordinates), returns the next search
    /// position past all non-empty tracks within the span, or `None` if all tracks within the
    /// span are entirely unoccupied. Used to place items which span every track in the other
    /// axis (such items can only fit in a stripe of entirely unoccupied tracks).
    pub fn occupied_track_jump(&self, axis: AbsoluteAxis, span: Line<OriginZeroLine>) -> Option<OriginZeroLine> {
        let counts = self.track_counts(axis);
        let track_lists = self.track_lists(axis);
        let range = counts.oz_line_range_to_track_range(span);
        let start = max(range.start, 0);
        let end = min(range.end, track_lists.len() as i16);
        let found = (start..end).rev().find(|&index| !track_lists[index as usize].is_empty());
        found.map(|track_index| {
            let line = counts.track_to_prev_oz_line(track_index as u16);
            line + 1
        })
    }

    /// Determines whether the specified row contains any items
    pub fn row_is_occupied(&self, row_index: usize) -> bool {
        self.track_lists(AbsoluteAxis::Vertical).get(row_index).is_some_and(|track| !track.is_empty())
    }

    /// Determines whether the specified column contains any items
    pub fn column_is_occupied(&self, column_index: usize) -> bool {
        self.track_lists(AbsoluteAxis::Horizontal).get(column_index).is_some_and(|track| !track.is_empty())
    }

    /// Returns the track counts of this CellOccunpancyMatrix in the relevant axis
    pub fn track_counts(&self, track_type: AbsoluteAxis) -> &TrackCounts {
        match track_type {
            AbsoluteAxis::Horizontal => &self.columns,
            AbsoluteAxis::Vertical => &self.rows,
        }
    }

    /// The sparse placement cursor for items starting in the given secondary-axis track.
    pub fn auto_placement_cursor(&self, track_type: AbsoluteAxis, start_at: OriginZeroLine) -> Option<OriginZeroLine> {
        let track_counts = self.track_counts(track_type.other_axis());
        let track_computed_index = track_counts.oz_line_to_next_track(start_at);
        let track_lists = self.track_lists(track_type.other_axis());
        if track_computed_index < 0 || track_computed_index >= track_lists.len() as i16 {
            // Index out of bounds: no tracks to search
            return None;
        }
        track_lists[track_computed_index as usize].auto_placement_cursor
    }

    /// Advances the sparse placement cursor to at least `primary_end`.
    /// The secondary-axis track must already exist in the matrix.
    fn advance_auto_placement_cursor(
        &mut self,
        primary_axis: AbsoluteAxis,
        secondary_start: OriginZeroLine,
        primary_end: OriginZeroLine,
    ) {
        let track_index = self.track_counts(primary_axis.other_axis()).oz_line_to_next_track(secondary_start) as usize;
        let tracks = match primary_axis {
            AbsoluteAxis::Horizontal => &mut self.row_intervals,
            AbsoluteAxis::Vertical => &mut self.column_intervals,
        };
        let cursor = &mut tracks[track_index].auto_placement_cursor;
        *cursor = max(*cursor, Some(primary_end));
    }
}

