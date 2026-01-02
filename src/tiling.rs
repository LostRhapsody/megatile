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
        let window_count = active_workspace.window_count();

        if window_count == 0 {
            return;
        }

        // Get monitor work area (usable space)
        let work_rect = self.get_work_area(monitor);

        // Create initial tile covering entire work area
        let mut root_tile = Tile::new(work_rect);

        // Distribute windows across tiles using Dwindle algorithm
        self.distribute_windows(&mut root_tile, windows);

        // Apply tile positions to windows
        self.apply_tile_positions(&root_tile, windows);
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

        if window_indices.is_empty() {
            return;
        }

        // Assign all windows to root tile initially
        tile.windows.extend(window_indices.iter());

        // Recursively split tiles
        self.split_tile(tile, window_indices.len());
    }

    fn split_tile(&self, tile: &mut Tile, window_count: usize) {
        if window_count <= 1 {
            return;
        }

        // Determine split direction
        let tile_width = tile.rect.right - tile.rect.left;
        let tile_height = tile.rect.bottom - tile.rect.top;

        let split_direction = if tile_width > tile_height {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };

        tile.split_direction = Some(split_direction);

        // Split windows between children
        let split_point = window_count / 2;
        let left_windows = tile.windows[..split_point].to_vec();
        let right_windows = tile.windows[split_point..].to_vec();

        // Create child tiles
        let (left_rect, right_rect) = self.split_rect(&tile.rect, split_direction);

        let mut left_tile = Tile::new(left_rect);
        left_tile.windows = left_windows;

        let mut right_tile = Tile::new(right_rect);
        right_tile.windows = right_windows;

        // Recursively split children
        let left_count = left_tile.windows.len();
        let right_count = right_tile.windows.len();

        if left_count > 0 {
            self.split_tile(&mut left_tile, left_count);
        }
        if right_count > 0 {
            self.split_tile(&mut right_tile, right_count);
        }

        tile.children = Some(Box::new((left_tile, right_tile)));
    }

    fn split_rect(&self, rect: &RECT, direction: SplitDirection) -> (RECT, RECT) {
        let gap = self.gap;
        let mid_gap = gap / 2;

        match direction {
            SplitDirection::Horizontal => {
                let height = rect.bottom - rect.top;
                let split = rect.top + height / 2 - mid_gap;

                let mut left = *rect;
                left.bottom = split;

                let mut right = *rect;
                right.top = split + gap;

                (left, right)
            }
            SplitDirection::Vertical => {
                let width = rect.right - rect.left;
                let split = rect.left + width / 2 - mid_gap;

                let mut left = *rect;
                left.right = split;

                let mut right = *rect;
                right.left = split + gap;

                (left, right)
            }
        }
    }

    fn apply_tile_positions(&self, tile: &Tile, windows: &mut [Window]) {
        if tile.is_leaf() {
            // Apply tile rect to all windows in this tile
            for &window_idx in &tile.windows {
                if window_idx < windows.len() {
                    windows[window_idx].rect = tile.rect;
                }
            }
        } else if let Some(ref children) = tile.children {
            self.apply_tile_positions(&children.0, windows);
            self.apply_tile_positions(&children.1, windows);
        }
    }
}

impl Default for DwindleTiler {
    fn default() -> Self {
        Self::new(8) // Default 8px gap
    }
}
