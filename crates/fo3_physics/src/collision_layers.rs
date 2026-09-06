//! # コリジョンレイヤーおよびグループフィルタリング
//!
//! Fallout 3 (Gamebryo 2.6 / Havok) のコリジョンレイヤーを物理エンジンの接触フィルタにマッピングします。
//!
//! 参照元: `references/nifxml/nif.xml:L2287` (`Fallout3Layer`), `knowledge/havok_collision_blocks.md`

use fo3_nif::Fallout3Layer;
use rapier3d::geometry::{Group, InteractionGroups};

/// Fallout 3 レイヤーを物理エンジンの InteractionGroups に変換する。
///
/// レイヤーごとに衝突すべき相手のマスクグループを設定します。
pub fn layer_to_interaction_groups(layer: Fallout3Layer) -> InteractionGroups {
    let (membership, filter) = match layer {
        Fallout3Layer::Static | Fallout3Layer::AnimStatic => {
            // 静的ジオメトリ: キャラクター、動的オブジェクト、弾丸等と衝突
            (
                Group::GROUP_1,
                Group::GROUP_2 | Group::GROUP_3 | Group::GROUP_4 | Group::GROUP_5,
            )
        }
        Fallout3Layer::CharController => {
            // プレイヤー/NPC キャラクタコントローラー: 静的、動的、地面等と衝突
            (
                Group::GROUP_2,
                Group::GROUP_1 | Group::GROUP_3 | Group::GROUP_4 | Group::GROUP_5,
            )
        }
        Fallout3Layer::Biped | Fallout3Layer::DeadBip => {
            // ラグドール・生体ボーン
            (
                Group::GROUP_3,
                Group::GROUP_1 | Group::GROUP_2 | Group::GROUP_3 | Group::GROUP_4,
            )
        }
        Fallout3Layer::DebrisSmall | Fallout3Layer::DebrisLarge => {
            // 破片・小物物理アイテム
            (
                Group::GROUP_4,
                Group::GROUP_1 | Group::GROUP_2 | Group::GROUP_3 | Group::GROUP_4,
            )
        }
        Fallout3Layer::Trigger | Fallout3Layer::AcousticSpace => {
            // トリガー・センサーゾーン (キャラクタのみ検知)
            (
                Group::GROUP_5,
                Group::GROUP_2,
            )
        }
        Fallout3Layer::NonCollidable => {
            // 衝突判定なし
            (Group::NONE, Group::NONE)
        }
        _ => {
            // デフォルト: 静的オブジェクト扱い
            (
                Group::GROUP_1,
                Group::GROUP_2 | Group::GROUP_3 | Group::GROUP_4 | Group::GROUP_5,
            )
        }
    };

    InteractionGroups::new(membership, filter)
}
