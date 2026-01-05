//! Tiling layout algorithms and data structures.
//!
//! This module implements a dwindle-style tiling algorithm where windows
//! are recursively split into halves, alternating between horizontal
//! and vertical splits based on the available space aspect ratio.

use crate::statusbar::STATUSBAR_VERTICAL_RESERVE;
use crate::workspace::{Monitor, Window};
use windows::Win32::Foundation::RECT;

/// Direction of a tile split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    /// Split into top and bottom regions.
    Horizontal,
    /// Split into left and right regions.
    Vertical,
}

/// A node in the tiling layout tree.
///
/// Tiles form a binary tree structure where each non-leaf tile is split
/// into two children. Leaf tiles contain the actual windows.
#[derive(Debug, Clone)]
pub struct Tile {
    pub rect: RECT,
    pub windows: Vec<isize>, // HWnds of windows in this tile
    pub split_direction: Option<SplitDirection>,
    pub children: Option<Box<(Tile, Tile)>>,
    pub split_ratio: f32, // Ratio for split (0.0-1.0, default 0.5)
}

impl Tile {
    /// Creates a new tile with the given bounds.
    pub fn new(rect: RECT) -> Self {
        Tile {
            rect,
            windows: Vec::new(),
            split_direction: None,
            children: None,
            split_ratio: 0.5, // Default 50/50 split
        }
    }

    /// Returns true if this tile has no children (is a leaf node).
    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }
}

/// Implements the dwindle tiling algorithm.
///
/// Windows are placed by recursively splitting the available space,
/// alternating between horizontal and vertical splits based on aspect ratio.
pub struct DwindleTiler {
    /// Gap in pixels between tiled windows.
    gap: i32,
}

impl DwindleTiler {
    /// Creates a new tiler with the specified gap between windows.
    pub fn new(gap: i32) -> Self {
        DwindleTiler { gap }
    }

    /// Calculates and applies tiling layout to windows on a monitor.
    ///
    /// Reuses the existing layout tree if possible, otherwise creates a new one.
    pub fn tile_windows(
        &self,
        monitor: &Monitor,
        layout_tree: &mut Option<crate::tiling::Tile>,
        windows: &mut [Window],
    ) {
        let active_workspace = monitor.get_active_workspace();
        println!(
            "DEBUG: Tiling active workspace: {:?} on monitor with rect {:?}",
            active_workspace, monitor.rect
        );
        let window_count = active_workspace.window_count();
        println!("DEBUG: Window count in workspace: {}", window_count);

        if window_count == 0 {
            println!("DEBUG: No windows to tile, returning");
            return;
        }

        // Get monitor work area (usable space)
        let work_rect = self.get_work_area(monitor);
        println!("DEBUG: Work area rect: {:?}", work_rect);

        // Check if we can reuse existing layout_tree
        let tiled_windows: Vec<_> = windows
            .iter()
            .filter(|w| w.workspace > 0 && w.is_tiled)
            .map(|w| w.hwnd)
            .collect();

        if let Some(existing_tree) = layout_tree.as_ref()
            && self.can_reuse_layout(existing_tree, &tiled_windows)
        {
            println!("DEBUG: Reusing existing layout tree");
            let mut updated_tree = existing_tree.clone();
            updated_tree.rect = work_rect;
            self.update_tree_rects(&mut updated_tree);
            self.apply_tile_positions(&updated_tree, windows);
            println!("DEBUG: Applied positions from existing layout");
            return;
        }

        // Create new layout tree
        println!("DEBUG: Creating new layout tree");
        let mut root_tile = Tile::new(work_rect);
        println!("DEBUG: Created initial root tile with rect {:?}", work_rect);

        // Distribute windows across tiles using Dwindle algorithm
        println!("DEBUG: Starting window distribution across tiles");
        self.distribute_windows(&mut root_tile, windows);

        println!("DEBUG: Window distribution completed");

        // Store the layout tree for future reuse
        *layout_tree = Some(root_tile.clone());

        // Apply tile positions to windows
        println!("DEBUG: Applying calculated tile positions to windows");
        self.apply_tile_positions(&root_tile, windows);

        println!(
            "DEBUG: Tile positioning completed for {} windows",
            window_count
        );
    }

    /// Calculates the usable work area for tiling on a monitor.
    fn get_work_area(&self, monitor: &Monitor) -> RECT {
        // For now, use full monitor rect
        // TODO: Consider taskbar and other reserved areas
        let mut rect = monitor.rect;
        // Add minimal gap padding - use smaller gaps at edges for tighter layout
        let edge_gap = 2; // Minimal edge gap
        rect.left += edge_gap;
        rect.top += STATUSBAR_VERTICAL_RESERVE; // No extra gap, status bar reserve is enough
        rect.right -= edge_gap;
        rect.bottom -= edge_gap; // Minimal gap at bottom
        if rect.top > rect.bottom {
            rect.top = rect.bottom;
        }
        rect
    }

    /// Assigns windows to the tile tree and triggers recursive splitting.
    fn distribute_windows(&self, tile: &mut Tile, windows: &[Window]) {
        // Count active windows and collect their hwnds
        let window_hwnds: Vec<isize> = windows
            .iter()
            .filter(|w| w.workspace > 0 && w.is_tiled)
            .map(|w| w.hwnd)
            .collect();

        println!(
            "DEBUG: Distributing {} windows across tiles",
            window_hwnds.len()
        );
        println!("DEBUG: Window hwnds to distribute: {:?}", window_hwnds);

        if window_hwnds.is_empty() {
            println!("DEBUG: No active windows to distribute");
            return;
        }

        // Assign all windows to root tile initially
        tile.windows = window_hwnds;
        println!(
            "DEBUG: Assigned all {} windows to root tile",
            tile.windows.len()
        );

        // Recursively split tiles
        println!(
            "DEBUG: Starting recursive tile splitting for {} windows",
            tile.windows.len()
        );
        self.split_tile(tile, tile.windows.len());
        println!("DEBUG: Recursive tile splitting completed");
    }

    /// Recursively splits a tile based on window count and aspect ratio.
    fn split_tile(&self, tile: &mut Tile, window_count: usize) {
        println!(
            "DEBUG: Splitting tile with {} windows, rect {:?}",
            window_count, tile.rect
        );

        if window_count <= 1 {
            println!(
                "DEBUG: Tile has {} windows, no splitting needed",
                window_count
            );
            return;
        }

        // Determine split direction
        let tile_width = tile.rect.right - tile.rect.left;
        let tile_height = tile.rect.bottom - tile.rect.top;
        println!("DEBUG: Tile dimensions: {}x{}", tile_width, tile_height);

        let split_direction = if tile_width > tile_height {
            println!("DEBUG: Splitting vertically (width > height)");
            SplitDirection::Vertical
        } else {
            println!("DEBUG: Splitting horizontally (height >= width)");
            SplitDirection::Horizontal
        };

        tile.split_direction = Some(split_direction);

        // Split windows between children
        let split_point = window_count / 2;
        let left_windows = tile.windows[..split_point].to_vec();
        let right_windows = tile.windows[split_point..].to_vec();

        println!(
            "DEBUG: Splitting {} windows at point {}: left gets {}, right gets {}",
            window_count,
            split_point,
            left_windows.len(),
            right_windows.len()
        );

        // Create child tiles
        let (left_rect, right_rect) =
            self.split_rect(&tile.rect, split_direction, tile.split_ratio);
        println!(
            "DEBUG: Split rects: left={:?}, right={:?}",
            left_rect, right_rect
        );

        let mut left_tile = Tile::new(left_rect);
        left_tile.windows = left_windows;

        let mut right_tile = Tile::new(right_rect);
        right_tile.windows = right_windows;

        // Recursively split children
        let left_count = left_tile.windows.len();
        let right_count = right_tile.windows.len();

        println!(
            "DEBUG: Processing left child tile with {} windows",
            left_count
        );
        if left_count > 0 {
            self.split_tile(&mut left_tile, left_count);
        }

        println!(
            "DEBUG: Processing right child tile with {} windows",
            right_count
        );
        if right_count > 0 {
            self.split_tile(&mut right_tile, right_count);
        }

        tile.children = Some(Box::new((left_tile, right_tile)));
        println!("DEBUG: Tile splitting completed for this level");
    }

    /// Splits a rectangle into two parts based on direction and ratio.
    fn split_rect(&self, rect: &RECT, direction: SplitDirection, ratio: f32) -> (RECT, RECT) {
        let gap = self.gap;
        let mid_gap = gap / 2;
        println!(
            "DEBUG: Splitting rect {:?} in direction {:?} with gap {}",
            rect, direction, gap
        );

        match direction {
            SplitDirection::Horizontal => {
                let height = rect.bottom - rect.top;
                let split = rect.top + (height as f32 * ratio) as i32 - mid_gap;
                println!(
                    "DEBUG: Horizontal split: height={}, ratio={}, split_point={}",
                    height, ratio, split
                );

                let mut left = *rect;
                left.bottom = split;

                let mut right = *rect;
                right.top = split + gap;

                println!(
                    "DEBUG: Horizontal split results: left={:?}, right={:?}",
                    left, right
                );
                (left, right)
            }
            SplitDirection::Vertical => {
                let width = rect.right - rect.left;
                let split = rect.left + (width as f32 * ratio) as i32 - mid_gap;
                println!(
                    "DEBUG: Vertical split: width={}, ratio={}, split_point={}",
                    width, ratio, split
                );

                let mut left = *rect;
                left.right = split;

                let mut right = *rect;
                right.left = split + gap;

                println!(
                    "DEBUG: Vertical split results: left={:?}, right={:?}",
                    left, right
                );
                (left, right)
            }
        }
    }

    /// Checks if an existing layout tree can be reused for the current windows.
    fn can_reuse_layout(&self, tile: &Tile, tiled_windows: &[isize]) -> bool {
        let tile_windows: Vec<_> = self
            .collect_tile_windows(tile)
            .into_iter()
            .flatten()
            .collect();
        tile_windows.len() == tiled_windows.len()
            && tile_windows.iter().all(|hwnd| tiled_windows.contains(hwnd))
    }

    /// Collects all window handles from a tile tree.
    fn collect_tile_windows(&self, tile: &Tile) -> Vec<Vec<isize>> {
        let mut result = Vec::new();
        self.collect_tile_windows_recursive(tile, &mut result);
        result
    }

    fn collect_tile_windows_recursive(&self, tile: &Tile, result: &mut Vec<Vec<isize>>) {
        if tile.is_leaf() {
            result.push(tile.windows.clone());
        } else if let Some(ref children) = tile.children {
            self.collect_tile_windows_recursive(&children.0, result);
            self.collect_tile_windows_recursive(&children.1, result);
        }
    }

    /// Updates rectangle positions throughout the tile tree.
    fn update_tree_rects(&self, tile: &mut Tile) {
        if let Some(ref mut children) = tile.children {
            let (left_rect, right_rect) =
                self.split_rect(&tile.rect, tile.split_direction.unwrap(), tile.split_ratio);
            children.0.rect = left_rect;
            children.1.rect = right_rect;
            self.update_tree_rects(&mut children.0);
            self.update_tree_rects(&mut children.1);
        }
    }

    /// Applies tile rectangles to window positions.
    fn apply_tile_positions(&self, tile: &Tile, windows: &mut [Window]) {
        if tile.is_leaf() {
            println!(
                "DEBUG: Applying positions to leaf tile with {} windows, rect {:?}",
                tile.windows.len(),
                tile.rect
            );
            // Apply tile rect to all windows in this tile
            for &window_hwnd in &tile.windows {
                if let Some(window) = windows.iter_mut().find(|w| w.hwnd == window_hwnd) {
                    println!(
                        "DEBUG: Setting window hwnd={:?} to rect {:?}",
                        window_hwnd, tile.rect
                    );
                    window.rect = tile.rect;
                } else {
                    println!(
                        "DEBUG: Warning: window hwnd {:?} not found in windows list",
                        window_hwnd
                    );
                }
            }
        } else if let Some(ref children) = tile.children {
            println!("DEBUG: Recursing into child tiles");
            self.apply_tile_positions(&children.0, windows);
            self.apply_tile_positions(&children.1, windows);
        } else {
            println!("DEBUG: Warning: Non-leaf tile with no children");
        }
    }
}

impl Default for DwindleTiler {
    fn default() -> Self {
        Self::new(4) // Default 4px gap for minimal spacing
    }
}
