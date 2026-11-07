//! Implements placing items in the grid and resolving the implicit grid.
//! <https://www.w3.org/TR/css-grid-1/#placement>
use super::types::{CellOccupancyMatrix, CellOccupancyState, GridItem};
use super::{OriginZeroLine, MAX_OZ_LINE, MIN_OZ_LINE};
use crate::geometry::Line;
use crate::geometry::{AbsoluteAxis, InBothAbsAxis};
use crate::style::{GridAutoFlow, OriginZeroGridPlacement};

#[inline]
/// Advances the cursor by one track.
fn advance_position(position: OriginZeroLine) -> OriginZeroLine {
    OriginZeroLine(position.0.saturating_add(1))
}

#[inline]
/// Resolves an indefinite span starting at `position`.
fn resolve_indefinite_grid_span(position: OriginZeroLine, span: u16) -> Line<OriginZeroLine> {
    let position = position.0 as i32;
    let span = span as i32;
    let line = |value: i32| OriginZeroLine(value.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    Line { start: line(position), end: line(position + span) }
}

/// A grid item's `grid-row`/`grid-column` styles with named lines resolved, in origin-zero coordinates
pub(super) type ItemPlacement = InBothAbsAxis<Line<OriginZeroGridPlacement>>;

/// 8.5. Grid Item Placement Algorithm
/// Place items into the grid, generating new rows/column into the implicit grid as required
///
/// `items` and `placements` are parallel slices in document order: `placements[i]` holds the `grid-row`/`grid-column`
/// styles of `items[i]`, with named lines already resolved. The placement passes run over these (small) values rather
/// than re-reading the children's styles.
///
/// [Specification](https://www.w3.org/TR/css-grid-2/#auto-placement-algo)
pub(super) fn place_grid_items(
    cell_occupancy_matrix: &mut CellOccupancyMatrix,
    items: &mut [GridItem],
    placements: &[ItemPlacement],
    grid_auto_flow: GridAutoFlow,
) {
    debug_assert_eq!(items.len(), placements.len());
    let primary_axis = grid_auto_flow.primary_axis();
    let secondary_axis = primary_axis.other_axis();

    // 1. Place children with definite positions
    for (item, &placement) in items.iter_mut().zip(placements.iter()) {
        if !(placement.horizontal.is_definite() && placement.vertical.is_definite()) {
            continue;
        }

        let (row_span, col_span) = place_definite_grid_item(placement, primary_axis);
        record_grid_placement(
            cell_occupancy_matrix,
            item,
            primary_axis,
            row_span,
            col_span,
            CellOccupancyState::DefinitelyPlaced,
        );
    }

    // 2. Place remaining children with definite secondary axis positions
    for (item, &placement) in items.iter_mut().zip(placements.iter()) {
        if !(placement.get(secondary_axis).is_definite() && !placement.get(primary_axis).is_definite()) {
            continue;
        }

        let (primary_span, secondary_span) =
            place_definite_secondary_axis_item(&*cell_occupancy_matrix, placement, grid_auto_flow);

        record_grid_placement(
            cell_occupancy_matrix,
            item,
            primary_axis,
            primary_span,
            secondary_span,
            CellOccupancyState::AutoPlaced,
        );
    }

    // 3. Determine the number of columns in the implicit grid
    // By the time we get to this point in the execution, this is actually already accounted for:
    //
    // 3.1 Start with the columns from the explicit grid
    //        => Handled by grid size estimate which is used to pre-size the GridOccupancyMatrix
    //
    // 3.2 Among all the items with a definite column position (explicitly positioned items, items positioned in the previous step,
    //     and items not yet positioned but with a definite column) add columns to the beginning and end of the implicit grid as necessary
    //     to accommodate those items.
    //        => Handled by expand_to_fit_range which expands the GridOccupancyMatrix as necessary
    //            -> Called by mark_area_as
    //            -> Called by record_grid_placement
    //
    // 3.3 If the largest column span among all the items without a definite column position is larger than the width of
    //     the implicit grid, add columns to the end of the implicit grid to accommodate that column span.
    //        => Handled by grid size estimate which is used to pre-size the GridOccupancyMatrix

    // 4. Position the remaining grid items
    // (which either have definite position only in the secondary axis or indefinite positions in both axis)
    let primary_axis = grid_auto_flow.primary_axis();
    let secondary_axis = primary_axis.other_axis();
    let primary_axis_grid_start_line = cell_occupancy_matrix.track_counts(primary_axis).implicit_start_line();
    let secondary_axis_grid_start_line = cell_occupancy_matrix.track_counts(secondary_axis).implicit_start_line();
    let grid_start_position = (primary_axis_grid_start_line, secondary_axis_grid_start_line);
    let mut grid_position = grid_start_position;
    for (item, &placement) in items.iter_mut().zip(placements.iter()) {
        if placement.get(secondary_axis).is_definite() {
            continue;
        }

        // Compute placement
        let (primary_span, secondary_span) =
            place_indefinitely_positioned_item(&*cell_occupancy_matrix, placement, grid_auto_flow, grid_position);

        // Record item
        record_grid_placement(
            cell_occupancy_matrix,
            item,
            primary_axis,
            primary_span,
            secondary_span,
            CellOccupancyState::AutoPlaced,
        );

        // If using the "dense" placement algorithm then reset the grid position back to grid_start_position ready for the next item
        // Otherwise set it to the position of the current item so that the next item it placed after it.
        grid_position = match grid_auto_flow.is_dense() {
            true => grid_start_position,
            false => (primary_span.end, secondary_span.start),
        };
    }
}

/// 8.5. Grid Item Placement Algorithm
/// Place a single definitely placed item into the grid
fn place_definite_grid_item(
    placement: ItemPlacement,
    primary_axis: AbsoluteAxis,
) -> (Line<OriginZeroLine>, Line<OriginZeroLine>) {
    // Resolve spans to tracks
    let primary_span = placement.get(primary_axis).resolve_definite_grid_lines();
    let secondary_span = placement.get(primary_axis.other_axis()).resolve_definite_grid_lines();

    (primary_span, secondary_span)
}

/// 8.5. Grid Item Placement Algorithm
/// Step 2. Place remaining children with definite secondary axis positions
fn place_definite_secondary_axis_item(
    cell_occupancy_matrix: &CellOccupancyMatrix,
    placement: ItemPlacement,
    auto_flow: GridAutoFlow,
) -> (Line<OriginZeroLine>, Line<OriginZeroLine>) {
    let primary_axis = auto_flow.primary_axis();
    let secondary_axis = primary_axis.other_axis();
    let primary_axis_grid_start_line = cell_occupancy_matrix.track_counts(primary_axis).implicit_start_line();

    let secondary_axis_placement = placement.get(secondary_axis).resolve_definite_grid_lines();
    let starting_position = match auto_flow.is_dense() {
        true => primary_axis_grid_start_line,
        false => cell_occupancy_matrix
            .auto_placement_cursor(primary_axis, secondary_axis_placement.start)
            .unwrap_or(primary_axis_grid_start_line),
    };
    let primary_axis_span = placement.get(primary_axis).indefinite_span();

    let mut position: OriginZeroLine = starting_position;
    loop {
        let primary_axis_placement = resolve_indefinite_grid_span(position, primary_axis_span);

        let collision = cell_occupancy_matrix.line_area_collision_jump(
            primary_axis,
            primary_axis_placement,
            secondary_axis_placement,
        );

        match collision {
            None => return (primary_axis_placement, secondary_axis_placement),
            Some(next_position) => position = next_position,
        }
    }
}

/// 8.5. Grid Item Placement Algorithm
/// Step 4. Position the remaining grid items.
fn place_indefinitely_positioned_item(
    cell_occupancy_matrix: &CellOccupancyMatrix,
    placement: ItemPlacement,
    auto_flow: GridAutoFlow,
    grid_position: (OriginZeroLine, OriginZeroLine),
) -> (Line<OriginZeroLine>, Line<OriginZeroLine>) {
    let primary_axis = auto_flow.primary_axis();
    let secondary_axis = primary_axis.other_axis();

    let primary_placement_style = placement.get(primary_axis);
    let secondary_placement_style = placement.get(secondary_axis);

    let secondary_span = secondary_placement_style.indefinite_span();
    let has_definite_primary_axis_position = primary_placement_style.is_definite();
    let primary_axis_grid_start_line = cell_occupancy_matrix.track_counts(primary_axis).implicit_start_line();
    let primary_axis_grid_end_line = cell_occupancy_matrix.track_counts(primary_axis).implicit_end_line();
    let secondary_axis_grid_start_line = cell_occupancy_matrix.track_counts(secondary_axis).implicit_start_line();

    let (mut primary_idx, mut secondary_idx) = grid_position;

    if has_definite_primary_axis_position {
        let primary_span = primary_placement_style.resolve_definite_grid_lines();

        // Compute secondary axis starting position for search
        secondary_idx = match auto_flow.is_dense() {
            // If auto-flow is dense then we always search from the first track
            true => secondary_axis_grid_start_line,
            false => {
                if primary_span.start < primary_idx {
                    advance_position(secondary_idx)
                } else {
                    secondary_idx
                }
            }
        };

        // Item has fixed primary axis position: so we simply increment the secondary axis position
        // until we find a space that the item fits in
        loop {
            let secondary_span = resolve_indefinite_grid_span(secondary_idx, secondary_span);

            // If area is occupied, jump the index past the collision and try again
            let collision =
                cell_occupancy_matrix.line_area_collision_jump(secondary_axis, secondary_span, primary_span);
            if let Some(next_position) = collision {
                secondary_idx = next_position;
                continue;
            }

            // Once we find a free space, return that position
            return (primary_span, secondary_span);
        }
    } else {
        let primary_span = primary_placement_style.indefinite_span();

        // Whether the item spans every track in the primary axis. Such an item can only be
        // placed at the primary axis grid start, in a stripe of entirely unoccupied tracks.
        let spans_all_primary_tracks = primary_span as usize >= cell_occupancy_matrix.track_counts(primary_axis).len();

        // Item does not have any fixed axis, so we search along the primary axis until we hit the end of the already
        // existent tracks, and then we reset the primary axis back to zero and increment the secondary axis index.
        // We continue in this vein until we find a space that the item fits in.
        loop {
            let primary_span = resolve_indefinite_grid_span(primary_idx, primary_span);
            let secondary_span = resolve_indefinite_grid_span(secondary_idx, secondary_span);

            // If the primary index is out of bounds, then increment the secondary index and reset the primary
            // index back to the start of the grid
            let primary_out_of_bounds = primary_span.end > primary_axis_grid_end_line;
            if primary_out_of_bounds {
                // If the span is out of bounds even at the search start position then it can never fit,
                // as searching only ever moves the span further away from the start of the grid. Bail out
                // and let `record_grid_placement` clamp the placement into the limited grid
                if primary_idx == primary_axis_grid_start_line {
                    return (primary_span, secondary_span);
                }
                secondary_idx = advance_position(secondary_idx);
                primary_idx = primary_axis_grid_start_line;
                continue;
            }

            // If the item spans every primary axis track, it fits if and only if all of the
            // secondary axis tracks it spans are entirely unoccupied. Jump the secondary index
            // past any non-empty tracks in the spanned stripe.
            if spans_all_primary_tracks {
                match cell_occupancy_matrix.occupied_track_jump(secondary_axis, secondary_span) {
                    Some(next_position) => {
                        secondary_idx = next_position;
                        primary_idx = primary_axis_grid_start_line;
                        continue;
                    }
                    None => return (primary_span, secondary_span),
                }
            }

            // If area is occupied, jump the primary index past the collision and try again
            let collision = cell_occupancy_matrix.line_area_collision_jump(primary_axis, primary_span, secondary_span);
            if let Some(next_position) = collision {
                primary_idx = next_position;
                continue;
            }

            // Once we find a free space that's in bounds, return that position
            return (primary_span, secondary_span);
        }
    }
}

/// Clamp a placement into the limited grid, preserving a span of at least 1 track.
/// Items placed outside of the limited grid are clamped into it.
///
/// See: <https://www.w3.org/TR/css-grid-1/#overlarge-grids>
fn clamp_span_to_limited_grid(span: Line<OriginZeroLine>) -> Line<OriginZeroLine> {
    let start = span.start.0.clamp(MIN_OZ_LINE, MAX_OZ_LINE - 1);
    let end = span.end.0.clamp(start + 1, MAX_OZ_LINE);
    Line { start: OriginZeroLine(start), end: OriginZeroLine(end) }
}

/// Record the grid item's placement in both the CellOccupancyMatrix and the item itself
/// once a definite placement has been determined
fn record_grid_placement(
    cell_occupancy_matrix: &mut CellOccupancyMatrix,
    item: &mut GridItem,
    primary_axis: AbsoluteAxis,
    primary_span: Line<OriginZeroLine>,
    secondary_span: Line<OriginZeroLine>,
    placement_type: CellOccupancyState,
) {

    // Clamp placements into the limited grid to prevent arithmetic overflow when growing the
    // implicit grid (https://www.w3.org/TR/css-grid-1/#overlarge-grids)
    let primary_span = clamp_span_to_limited_grid(primary_span);
    let secondary_span = clamp_span_to_limited_grid(secondary_span);

    // Mark area of grid as occupied
    cell_occupancy_matrix.mark_area_as(primary_axis, primary_span, secondary_span, placement_type);

    // Record placement on the item
    let (col_span, row_span) = match primary_axis {
        AbsoluteAxis::Horizontal => (primary_span, secondary_span),
        AbsoluteAxis::Vertical => (secondary_span, primary_span),
    };
    item.column = col_span;
    item.row = row_span;

}

