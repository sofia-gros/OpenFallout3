//! アニメーションおよびスケルトン・ボーン姿勢管理モジュール。
//!
//! 参照元: Gamebryo 2.6 `NiControllerSequence::Update` → `NiSkinInstance::Update`
//! 参照元: `knowledge/actor_and_skin_mesh.md` (セクション 4.4: スケルトン分離とボーン名マッピング)

use std::collections::HashMap;
use std::io::Cursor;
use fo3_esm::FormId;
use fo3_nif::NifFile;
use fo3_render::{AnimationClip, AnimationPlayer, SkeletonPose};
use fo3_vfs::VfsManager;
use glam::Mat4;
use crate::types::{get_actor_part_paths, ViewerTarget};

/// アニメーション再生・スキニング更新用のステート。
pub struct AnimState {
    pub anim_player: Option<AnimationPlayer>,
    /// KF ファイルの NifFile（アニメーションデータ）
    pub anim_kf_nif: Option<NifFile>,
    /// パーツメッシュ NIF 群（スキニング変形対象）
    pub anim_parts: Vec<NifFile>,
    /// スケルトン NIF（真のボーン階層ツリー走査用）
    pub anim_skeleton_nif: Option<NifFile>,
    /// 現在の骨格姿勢（KF → SkeletonPose のオーバーライド）
    pub anim_pose: SkeletonPose,
    /// ボーンワールド行列マップ（block_index → Mat4）
    pub anim_bone_world_map: HashMap<i32, Mat4>,
    /// ボーン名ワールド行列マップ（bone_name → Mat4）
    pub anim_bone_name_world_map: HashMap<String, Mat4>,
}

impl AnimState {
    /// ターゲットに応じたアニメーション初期状態を構築する。
    pub fn new(target: &ViewerTarget, vfs: &mut VfsManager) -> Self {
        if let ViewerTarget::Actor { outfit_or_naked, kf_path } = target {
            use fo3_render::recompute_bone_world_maps_with_pose;

            // 1. KF ファイル読み込み
            let kf_bytes = vfs
                .read(kf_path)
                .expect("KF ファイルの読み込みに失敗しました");
            let mut kf_cursor = Cursor::new(kf_bytes);
            let kf_nif =
                NifFile::read(&mut kf_cursor).expect("KF ファイルのパースに失敗しました");
            println!("KF パース成功 (ブロック数: {})", kf_nif.blocks.len());

            // 2. スケルトン NIF 読み込み
            let skel_path = "meshes\\characters\\_male\\skeleton.nif";
            let skel_bytes = vfs
                .read(skel_path)
                .expect("スケルトン NIF の読み込みに失敗しました");
            let mut skel_cursor = Cursor::new(skel_bytes);
            let skel_nif =
                NifFile::read(&mut skel_cursor).expect("スケルトン NIF のパースに失敗しました");

            // 3. 全パーツ NIF 群の読み込み
            let body_path = if outfit_or_naked.eq_ignore_ascii_case("naked") {
                "meshes\\characters\\_male\\upperbody.nif".to_string()
            } else {
                outfit_or_naked.clone()
            };
            let is_female = outfit_or_naked.to_ascii_lowercase().contains("female")
                || outfit_or_naked.to_ascii_lowercase().contains("outfitf");
            let part_paths = get_actor_part_paths(is_female, FormId(0), &body_path, None, None, None, None, false);
            let mut anim_parts = Vec::new();
            for path in &part_paths {
                if let Ok(bytes) = vfs.read(path) {
                    let mut cursor = Cursor::new(bytes);
                    if let Ok(nif) = NifFile::read(&mut cursor) {
                        anim_parts.push(nif);
                    }
                }
            }

            // 4. 初期ポーズでボーンワールド行列およびボーン名マップを計算
            let mut bone_world_map = HashMap::new();
            let mut bone_name_world_map = HashMap::new();
            let initial_pose = SkeletonPose::default();
            recompute_bone_world_maps_with_pose(
                &skel_nif,
                &initial_pose,
                &mut bone_world_map,
                &mut bone_name_world_map,
            );

            // AnimationClip を構築して AnimationPlayer を作成
            let player = AnimationClip::from_kf(&kf_nif).map(|clip| {
                println!(
                    "アクターアニメーションクリップ \"{}\" ロード完了: {:.2}s〜{:.2}s, チャンネル数: {}",
                    clip.name, clip.start_time, clip.stop_time, clip.channels.len()
                );
                AnimationPlayer::new(clip)
            });

            Self {
                anim_player: player,
                anim_kf_nif: Some(kf_nif),
                anim_parts,
                anim_skeleton_nif: Some(skel_nif),
                anim_pose: initial_pose,
                anim_bone_world_map: bone_world_map,
                anim_bone_name_world_map: bone_name_world_map,
            }
        } else if let ViewerTarget::Anim { nif_path, kf_path } = target {
            use fo3_render::recompute_bone_world_maps_with_pose;

            // 1. KF ファイル読み込み
            let kf_bytes = vfs
                .read(kf_path)
                .expect("KF ファイルの読み込みに失敗しました");
            let mut kf_cursor = Cursor::new(kf_bytes);
            let kf_nif =
                NifFile::read(&mut kf_cursor).expect("KF ファイルのパースに失敗しました");
            println!("KF パース成功 (ブロック数: {})", kf_nif.blocks.len());

            // 2. スキンメッシュ NIF 読み込み
            let nif_bytes = vfs
                .read(nif_path)
                .expect("メッシュ NIF の読み込みに失敗しました");
            let mut nif_cursor = Cursor::new(nif_bytes);
            let skin_nif =
                NifFile::read(&mut nif_cursor).expect("メッシュ NIF のパースに失敗しました");

            // 3. 人型アクター等で外部スケルトン NIF が必要な場合の検出
            let has_true_skeleton = skin_nif.blocks.iter().any(|b| {
                if let fo3_nif::NifBlock::NiNode(ref node) = b {
                    let name = skin_nif.get_string(node.av.net.name_index);
                    name == Some("Bip01") || name == Some("Bip01 Pelvis")
                } else {
                    false
                }
            });

            let skeleton_nif = if !has_true_skeleton && (nif_path.to_ascii_lowercase().contains("characters") || nif_path.to_ascii_lowercase().contains("armor")) {
                let skel_path = "meshes\\characters\\_male\\skeleton.nif";
                println!("メッシュ単体に全身骨格がないため、共有スケルトン NIF をロード中: {}", skel_path);
                if let Ok(skel_bytes) = vfs.read(skel_path) {
                    let mut skel_cursor = Cursor::new(skel_bytes);
                    NifFile::read(&mut skel_cursor).ok()
                } else {
                    None
                }
            } else {
                None
            };

            let eval_nif = skeleton_nif.as_ref().unwrap_or(&skin_nif);

            // 4. 初期ポーズでボーンワールド行列およびボーン名マップを計算
            let mut bone_world_map = HashMap::new();
            let mut bone_name_world_map = HashMap::new();
            let initial_pose = SkeletonPose::default();
            recompute_bone_world_maps_with_pose(
                eval_nif,
                &initial_pose,
                &mut bone_world_map,
                &mut bone_name_world_map,
            );

            // AnimationClip を構築して AnimationPlayer を作成
            let player = AnimationClip::from_kf(&kf_nif).map(|clip| {
                println!(
                    "アニメーションクリップ \"{}\" ロード完了: {:.2}s〜{:.2}s, チャンネル数: {}",
                    clip.name, clip.start_time, clip.stop_time, clip.channels.len()
                );
                AnimationPlayer::new(clip)
            });

            Self {
                anim_player: player,
                anim_kf_nif: Some(kf_nif),
                anim_parts: vec![skin_nif],
                anim_skeleton_nif: skeleton_nif,
                anim_pose: initial_pose,
                anim_bone_world_map: bone_world_map,
                anim_bone_name_world_map: bone_name_world_map,
            }
        } else {
            Self {
                anim_player: None,
                anim_kf_nif: None,
                anim_parts: Vec::new(),
                anim_skeleton_nif: None,
                anim_pose: SkeletonPose::default(),
                anim_bone_world_map: HashMap::new(),
                anim_bone_name_world_map: HashMap::new(),
            }
        }
    }

    /// フレーム更新処理（単体 Anim / Actor モード用）。
    pub fn update(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &mut fo3_render::RenderScene,
    ) {
        if let (Some(player), Some(kf_nif), Some(skel_nif)) = (
            self.anim_player.as_mut(),
            self.anim_kf_nif.as_ref(),
            self.anim_skeleton_nif.as_ref(),
        ) {
            // 1. アニメーション時刻を進めてボーン姿勢を更新
            player.update(kf_nif, dt, &mut self.anim_pose);

            // 2. スケルトン NIF の FK を再計算（アニメーション姿勢適用後のボーンワールド行列 & ボーン名マップ）
            fo3_render::recompute_bone_world_maps_with_pose(
                skel_nif,
                &self.anim_pose,
                &mut self.anim_bone_world_map,
                &mut self.anim_bone_name_world_map,
            );

            // 3. 全パーツスキンメッシュの頂点バッファをスケルトンのボーン名ワールド行列で更新
            let part_refs: Vec<&NifFile> = self.anim_parts.iter().collect();
            scene.update_animated_skins_multi_parts(device, queue, &part_refs, &self.anim_bone_name_world_map);

            // 4. 全剛体アタッチメントパーツ (目・歯・舌など) のモデル行列をボーン追従更新
            scene.update_animated_rigid_meshes(queue, &self.anim_bone_name_world_map);
        }
    }
}
