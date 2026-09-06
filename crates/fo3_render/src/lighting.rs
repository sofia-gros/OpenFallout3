//! # Gamebryo 2.6 セル環境光・点光源ライティングモジュール
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadcell.hpp:L87` (`Lighting`)
//! - `references/openmw/components/esm4/loadligh.hpp:L58` (`Light::Data`)
//! - `references/openmw/files/shaders/lib/light/util.glsl:L45-97` (`calcPointLighting`)
//! - Gamebryo 2.6 `NiPointLight`, `NiAmbientLight`, `NiDirectionalLight`

use bytemuck::{Pod, Zeroable};
use fo3_esm::CellLighting;
use glam::Vec3;

/// シェーダーに渡す単一点光源データ (32 バイト)。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuPointLight {
    /// XYZ: ワールド座標, W: 最大到達半径 (radius)
    pub pos_radius: [f32; 4],
    /// RGB: 光源色 (0.0..1.0), W: 減衰指数 (falloff)
    pub color_falloff: [f32; 4],
}

impl Default for GpuPointLight {
    fn default() -> Self {
        Self {
            pos_radius: [0.0; 4],
            color_falloff: [0.0; 4],
        }
    }
}

/// シェーダーに渡すセル全体のライティング Uniform 構造体 (592 バイト)。
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LightingUniform {
    /// アンビエント環境光色 (RGB, W=1.0)
    pub ambient_color: [f32; 4],
    /// 指向性平行光源色 (RGB, W=0.0)
    pub dir_light_color: [f32; 4],
    /// 指向性平行光の照射方向 (正規化 XYZ, W=0.0)
    pub dir_light_dir: [f32; 4],
    /// RGB: フォグ色, W: フォグ開始距離 (near)
    pub fog_color_near: [f32; 4],
    /// X: フォグ最大距離 (far), Y: フォグ濃度指数 (power), Z: 有効点光源数, W: 未使用
    pub fog_far_power: [f32; 4],
    /// 最大 16 個の配置点光源配列
    pub point_lights: [GpuPointLight; 16],
}

impl Default for LightingUniform {
    fn default() -> Self {
        Self {
            ambient_color: [0.35, 0.35, 0.35, 1.0],
            dir_light_color: [0.65, 0.65, 0.65, 0.0],
            dir_light_dir: [0.5, 0.5, 0.8, 0.0],
            fog_color_near: [0.0, 0.0, 0.0, 0.0],
            fog_far_power: [0.0, 1.0, 0.0, 0.0],
            point_lights: [GpuPointLight::default(); 16],
        }
    }
}

/// 配置された点光源情報。
#[derive(Clone, Copy, Debug)]
pub struct PlacedPointLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: [f32; 3],
    pub falloff: f32,
}

impl LightingUniform {
    /// セル環境光 (XCLL) および配置点光源群から LightingUniform を構築する。
    pub fn from_cell_lighting(
        cell_lighting: Option<&CellLighting>,
        lights: &[PlacedPointLight],
        camera_pos: Vec3,
    ) -> Self {
        let mut uniform = Self::default();

        if let Some(lgt) = cell_lighting {
            // RGBA 各要素を 0..255 から 0.0..1.0 に正規化
            uniform.ambient_color = [
                lgt.ambient[0] as f32 / 255.0,
                lgt.ambient[1] as f32 / 255.0,
                lgt.ambient[2] as f32 / 255.0,
                1.0,
            ];

            let dir_r = lgt.directional[0] as f32 / 255.0;
            let dir_g = lgt.directional[1] as f32 / 255.0;
            let dir_b = lgt.directional[2] as f32 / 255.0;
            uniform.dir_light_color = [dir_r, dir_g, dir_b, 0.0];

            // 指向性光の角度計算 (Gamebryo 回転)
            let rot_xy = (lgt.rotation_xy as f32).to_radians();
            let rot_z = (lgt.rotation_z as f32).to_radians();
            let dir_x = rot_z.cos() * rot_xy.cos();
            let dir_y = rot_z.cos() * rot_xy.sin();
            let dir_z = rot_z.sin();
            let dir = Vec3::new(dir_x, dir_y, dir_z).normalize_or_zero();
            uniform.dir_light_dir = [dir.x, dir.y, dir.z, 0.0];

            uniform.fog_color_near = [
                lgt.fog_color[0] as f32 / 255.0,
                lgt.fog_color[1] as f32 / 255.0,
                lgt.fog_color[2] as f32 / 255.0,
                lgt.fog_near,
            ];

            uniform.fog_far_power = [
                lgt.fog_far,
                if lgt.fog_power > 0.01 { lgt.fog_power } else { 1.0 },
                0.0,
                0.0,
            ];
        }

        // カメラに近い順に最大 16 個の点光源を選択
        let mut sorted_lights: Vec<&PlacedPointLight> = lights.iter().collect();
        sorted_lights.sort_by(|a, b| {
            let dist_a = a.position.distance_squared(camera_pos);
            let dist_b = b.position.distance_squared(camera_pos);
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let num_lights = sorted_lights.len().min(16);
        for (i, light) in sorted_lights.iter().take(num_lights).enumerate() {
            uniform.point_lights[i] = GpuPointLight {
                pos_radius: [light.position.x, light.position.y, light.position.z, light.radius],
                color_falloff: [light.color[0], light.color[1], light.color[2], light.falloff],
            };
        }
        uniform.fog_far_power[2] = num_lights as f32;

        uniform
    }
}
