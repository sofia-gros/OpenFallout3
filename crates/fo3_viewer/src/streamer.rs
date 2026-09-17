use fo3_esm::FormId;
use glam::Vec3;
use std::collections::HashSet;

pub struct WorldStreamer {
    pub current_grid: Option<(i32, i32)>,
    pub loaded_cells: HashSet<u32>,
    pub world_edid: String,
    pub radius: i32,
}

impl WorldStreamer {
    pub fn new(world_edid: String, radius: i32) -> Self {
        Self {
            current_grid: None,
            loaded_cells: HashSet::new(),
            world_edid,
            radius,
        }
    }

    /// プレイヤー座標から現在のグリッドを計算
    pub fn calculate_grid(pos: Vec3) -> (i32, i32) {
        let x = (pos.x / fo3_esm::LAND_REAL_SIZE).floor() as i32;
        let y = (pos.y / fo3_esm::LAND_REAL_SIZE).floor() as i32;
        (x, y)
    }

    /// グリッドが変化したかどうかを確認し、更新が必要なら周辺グリッドの座標リストを返す
    pub fn check_update(&mut self, pos: Vec3) -> Option<Vec<(i32, i32)>> {
        let grid = Self::calculate_grid(pos);
        if Some(grid) == self.current_grid {
            return None;
        }

        self.current_grid = Some(grid);
        let mut target_grids = Vec::new();
        for x in (grid.0 - self.radius)..=(grid.0 + self.radius) {
            for y in (grid.1 - self.radius)..=(grid.1 + self.radius) {
                target_grids.push((x, y));
            }
        }
        Some(target_grids)
    }
}
