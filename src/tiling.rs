use crate::workspace::{Monitor, Window};
use windows::Win32::Foundation::RECT;

#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub rect: RECT,
    pub windows: Vec<usize>, // Indices into workspace.windows
    pub split_direction: Option<SplitDirection>,
    pub children: Option<Box<(Tile, Tile)>>,
}

impl Tile {
    pub fn new(rect: RECT) -> Self {
        Tile {
            rect,
            windows: Vec::new(),
            split_direction: None,
            children: None,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }
}

pub struct DwindleTiler {
    gap: i32,
}

impl DwindleTiler {
    pub fn new(gap: i32) -> Self {
        DwindleTiler { gap }
    }

    pub fn tile_windows(&self, monitor: &Monitor, windows: &mut [Window]) {
        let active_workspace = monitor.get_active_workspace();
        println!(
            "DEBUG: Tiling active workspace: {} on monitor with rect {:?}",
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

        // Create initial tile covering entire work area
        let mut root_tile = Tile::new(work_rect);
        println!("DEBUG: Created initial root tile with rect {:?}", work_rect);

        // Distribute windows across tiles using Dwindle algorithm
        println!("DEBUG: Starting window distribution across tiles");
        self.distribute_windows(&mut root_tile, windows);

        println!("DEBUG: Window distribution completed");

        // Apply tile positions to windows
        println!("DEBUG: Applying calculated tile positions to windows");
        self.apply_tile_positions(&root_tile, windows);

        println!(
            "DEBUG: Tile positioning completed for {} windows",
            window_count
        );
    }

    fn get_work_area(&self, monitor: &Monitor) -> RECT {
        // For now, use full monitor rect
        // TODO: Consider taskbar and other reserved areas
        let mut rect = monitor.rect;
        // Add gap padding
        rect.left += self.gap;
        rect.top += self.gap;
        rect.right -= self.gap;
        rect.bottom -= self.gap;
        rect
    }

    fn distribute_windows(&self, tile: &mut Tile, windows: &[Window]) {
        // Count active windows and collect their indices
        let window_indices: Vec<usize> = (0..windows.len())
            .filter(|&i| windows[i].workspace > 0)
            .collect();

        println!(
            "DEBUG: Distributing {} windows across tiles",
            window_indices.len()
        );
        println!("DEBUG: Window indices to distribute: {:?}", window_indices);

        if window_indices.is_empty() {
            println!("DEBUG: No active windows to distribute");
            return;
        }

        // Assign all windows to root tile initially
        tile.windows.extend(window_indices.iter());
        println!(
            "DEBUG: Assigned all {} windows to root tile",
            tile.windows.len()
        );

        // Recursively split tiles
        println!(
            "DEBUG: Starting recursive tile splitting for {} windows",
            window_indices.len()
        );
        self.split_tile(tile, window_indices.len());
        println!("DEBUG: Recursive tile splitting completed");
    }

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
        let (left_rect, right_rect) = self.split_rect(&tile.rect, split_direction);
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

    fn split_rect(&self, rect: &RECT, direction: SplitDirection) -> (RECT, RECT) {
        let gap = self.gap;
        let mid_gap = gap / 2;
        println!(
            "DEBUG: Splitting rect {:?} in direction {:?} with gap {}",
            rect, direction, gap
        );

        match direction {
            SplitDirection::Horizontal => {
                let height = rect.bottom - rect.top;
                let split = rect.top + height / 2 - mid_gap;
                println!(
                    "DEBUG: Horizontal split: height={}, split_point={}",
                    height, split
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
                let split = rect.left + width / 2 - mid_gap;
                println!(
                    "DEBUG: Vertical split: width={}, split_point={}",
                    width, split
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

    fn apply_tile_positions(&self, tile: &Tile, windows: &mut [Window]) {
        if tile.is_leaf() {
            println!(
                "DEBUG: Applying positions to leaf tile with {} windows, rect {:?}",
                tile.windows.len(),
                tile.rect
            );
            // Apply tile rect to all windows in this tile
            for &window_idx in &tile.windows {
                if window_idx < windows.len() {
                    println!(
                        "DEBUG: Setting window at index {} (hwnd={:?}) to rect {:?}",
                        window_idx, windows[window_idx].hwnd, tile.rect
                    );
                    windows[window_idx].rect = tile.rect;
                } else {
                    println!(
                        "DEBUG: Warning: window index {} out of bounds (max {})",
                        window_idx,
                        windows.len()
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
        Self::new(8) // Default 8px gap
    }
}
