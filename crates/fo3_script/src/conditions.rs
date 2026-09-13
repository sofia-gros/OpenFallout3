//! # 会話・スクリプト条件式 (`CTDA`) 評価エンジン
//!
//! 参照元:
//! - `references/openmw/components/esm4/loadinfo.cpp:81-105`
//! - `references/openmw/components/esm4/script.hpp:330-345`

use std::collections::{HashMap, HashSet};
use glam::Vec3;
use fo3_esm::{FormId, TargetCondition};
use crate::opcodes::functions::*;

/// 条件式評価に必要なゲーム実行コンテキスト。
#[derive(Clone, Debug, Default)]
pub struct ConditionContext {
    /// 会話の話者 (NPC) FormID
    pub speaker: Option<FormId>,
    /// 会話の対象 (プレイヤー) FormID
    pub target: Option<FormId>,
    /// 話者の空間座標
    pub speaker_pos: Vec3,
    /// プレイヤーの空間座標
    pub player_pos: Vec3,
    /// クエストごとの現在ステージマップ (`QuestFormID -> Stage`)
    pub quest_stages: HashMap<FormId, u32>,
    /// クエストごとの到達済みステージ履歴 (`QuestFormID -> Set<Stage>`)
    pub quest_stage_history: HashMap<FormId, HashSet<u32>>,
    /// プレイヤーのインベントリ所持数 (`ItemFormID -> Count`)
    pub inventory: HashMap<FormId, u32>,
}

/// 単一の `TargetCondition` を評価する。
pub fn evaluate_single_condition(cond: &TargetCondition, ctx: &ConditionContext) -> bool {
    let actual_value = match cond.function_index {
        FN_GET_IS_ID => {
            // 話者の Base FormID と比較
            if let Some(speaker_id) = ctx.speaker {
                if speaker_id.0 == cond.param1 { 1.0 } else { 0.0 }
            } else {
                0.0
            }
        }
        FN_GET_STAGE => {
            // クエストステージ取得
            let q_id = FormId(cond.param1);
            let stage = ctx.quest_stages.get(&q_id).copied().unwrap_or(0);
            stage as f32
        }
        FN_GET_STAGE_DONE => {
            // クエストステージ到達判定 (param1: Quest, param2 または comparison_value: Stage)
            let q_id = FormId(cond.param1);
            let target_stage = if cond.param2 != 0 {
                cond.param2
            } else {
                cond.comparison_value as u32
            };
            let done = ctx.quest_stage_history
                .get(&q_id)
                .map(|h| h.contains(&target_stage))
                .unwrap_or(false);
            if done { 1.0 } else { 0.0 }
        }
        FN_GET_QUEST_RUNNING => {
            // クエスト実行中判定 (ステージ > 0)
            let q_id = FormId(cond.param1);
            let running = ctx.quest_stages.get(&q_id).map(|&s| s > 0).unwrap_or(false);
            if running { 1.0 } else { 0.0 }
        }
        FN_GET_ITEM_COUNT => {
            // アイテム所持数
            let item_id = FormId(cond.param1);
            let count = ctx.inventory.get(&item_id).copied().unwrap_or(0);
            count as f32
        }
        FN_GET_DISTANCE => {
            // 距離算出
            ctx.speaker_pos.distance(ctx.player_pos)
        }
        _ => {
            // 未知または未実装の関数は真とみなす（Fallout 3 フォールバック）
            return true;
        }
    };

    let op_type = cond.operator & 0x7F; // 下位ビットが演算子
    match op_type {
        0 => (actual_value - cond.comparison_value).abs() < 1e-4, // ==
        1 => (actual_value - cond.comparison_value).abs() >= 1e-4, // !=
        2 => actual_value > cond.comparison_value,                 // >
        3 => actual_value >= cond.comparison_value,                // >=
        4 => actual_value < cond.comparison_value,                 // <
        5 => actual_value <= cond.comparison_value,                // <=
        _ => true,
    }
}

/// 複数条件式リスト（AND / OR 結合）を順次評価する。
///
/// 参照元: `references/openmw/components/esm4/loadinfo.cpp:81-105`
/// `operator` の bit 7 (0x80) が立っている場合は OR 結合。
pub fn evaluate_conditions(conditions: &[TargetCondition], ctx: &ConditionContext) -> bool {
    if conditions.is_empty() {
        return true;
    }

    let mut current_or_group = false;
    let mut in_or_chain = false;

    for cond in conditions {
        let is_or = (cond.operator & 0x80) != 0;
        let result = evaluate_single_condition(cond, ctx);

        if is_or {
            current_or_group = current_or_group || result;
            in_or_chain = true;
        } else if in_or_chain {
            current_or_group = current_or_group || result;
            if !current_or_group {
                return false;
            }
            in_or_chain = false;
            current_or_group = false;
        } else if !result {
            return false;
        }
    }

    if in_or_chain && !current_or_group {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_conditions_speaker_and_stage() {
        let mut ctx = ConditionContext::default();
        let npc_id = FormId(0x00012345);
        let quest_id = FormId(0x000abcde);

        ctx.speaker = Some(npc_id);
        ctx.quest_stages.insert(quest_id, 20);

        // 条件 1: GetIsID == 0x00012345 (一致)
        let cond1 = TargetCondition {
            operator: 0, // ==
            comparison_value: 1.0,
            function_index: FN_GET_IS_ID,
            param1: npc_id.0,
            param2: 0,
            run_on: 0,
            reference: FormId(0),
        };

        // 条件 2: GetStage == 20 (一致)
        let cond2 = TargetCondition {
            operator: 0, // ==
            comparison_value: 20.0,
            function_index: FN_GET_STAGE,
            param1: quest_id.0,
            param2: 0,
            run_on: 0,
            reference: FormId(0),
        };

        assert!(evaluate_conditions(&[cond1.clone(), cond2.clone()], &ctx));

        // ステージが異なる場合は不一致
        ctx.quest_stages.insert(quest_id, 30);
        assert!(!evaluate_conditions(&[cond1, cond2], &ctx));
    }
}
