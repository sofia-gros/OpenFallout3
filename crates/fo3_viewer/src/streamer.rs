use fo3_esm::FormId;
use glam::Vec3;
use std::collections::HashSet;

pub struct WorldStreamer {
    pub current_grid: Option<(i32, i32)>,
    pub loaded_grids: HashSet<(i32, i32)>,
    pub loaded_cells_by_grid: std::collections::HashMap<(i32, i32), Vec<u32>>,
    pub world_edid: String,
    pub radius: i32,
}

impl WorldStreamer {
    pub fn new(world_edid: String, radius: i32) -> Self {
        Self {
            current_grid: None,
            loaded_grids: HashSet::new(),
            loaded_cells_by_grid: std::collections::HashMap::new(),
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

    /// グリッドが変化したかどうかを確認し、更新が必要なら (to_load, to_unload) のリストを返す
    pub fn check_update(&mut self, pos: Vec3) -> Option<(Vec<(i32, i32)>, Vec<(i32, i32)>)> {
        let grid = Self::calculate_grid(pos);
        if Some(grid) == self.current_grid {
            return None;
        }

        self.current_grid = Some(grid);
        let mut target_grids = HashSet::new();
        for x in (grid.0 - self.radius)..=(grid.0 + self.radius) {
            for y in (grid.1 - self.radius)..=(grid.1 + self.radius) {
                target_grids.insert((x, y));
            }
        }

        let to_load: Vec<(i32, i32)> = target_grids
            .difference(&self.loaded_grids)
            .copied()
            .collect();
        let to_unload: Vec<(i32, i32)> = self
            .loaded_grids
            .difference(&target_grids)
            .copied()
            .collect();

        if to_load.is_empty() && to_unload.is_empty() {
            return None;
        }

        for g in &to_unload {
            self.loaded_grids.remove(g);
        }
        for g in &to_load {
            self.loaded_grids.insert(*g);
        }

        Some((to_load, to_unload))
    }
}
