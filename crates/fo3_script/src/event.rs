//! # イベントディスパッチャー & スクリプトインスタンス状態管理 (`event`)
//!
//! Fallout 3 のゲーム内イベント (`OnActivate`, `OnAdd`, `GameMode` 等) をキューイングし、
//! アタッチされたスクリプトの対応ブロックへディスパッチする。
//!
//! 参照元:
//! - GECK Wiki "Begin"
//! - `references/openmw/apps/openmw/mwworld/refdata.cpp:19-21` (`Flag_SuppressActivate`, `Flag_OnActivate`)
//! - `references/openmw/apps/openmw/mwlua/engineevents.hpp`

use std::collections::{HashMap, VecDeque};
use fo3_esm::types::FormId;
use fo3_esm::records::scpt::{ScptRecord, ScriptBlock, ScriptEventType};
use crate::vm::{ScriptError, ScriptVm};

/// ゲーム内スクリプトイベント。
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    /// オブジェクトのアクティベート (Eキー操作等)
    /// 参照元: `references/openmw/apps/openmw/mwlua/engineevents.hpp:37`
    OnActivate {
        /// 対象オブジェクトの FormID
        target: FormId,
        /// アクティベートを実行したアクターの FormID (通常はプレイヤー)
        actor: FormId,
    },
    /// コンテナ・インベントリへのアイテム追加
    OnAdd {
        item: FormId,
        container: FormId,
    },
    /// アイテムのドロップ
    OnDrop {
        item: FormId,
        dropper: FormId,
    },
    /// アイテムの装備
    OnEquip {
        item: FormId,
        actor: FormId,
    },
    /// アイテムの装備解除
    OnUnequip {
        item: FormId,
        actor: FormId,
    },
    /// 通常フレーム更新ループ
    GameMode,
    /// メニューUI開放中
    MenuMode {
        menu_type: u32,
    },
}

/// 個々のオブジェクトインスタンスに紐づくスクリプト実行コンテキスト。
/// ローカル変数の永続化値やアクティベート抑制状態を保持する。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptInstanceContext {
    /// アタッチされたスクリプトの FormID
    pub script_form_id: FormId,
    /// インスタンス固有のローカル変数ストレージ (変数名小文字 -> 値)
    pub local_vars: HashMap<String, f32>,
    /// アクティベート抑制フラグ (`Flag_SuppressActivate`)
    /// `OnActivate` ブロックが実行されると true になり、スクリプト内で `Activate` が呼ばれると false に戻る。
    pub suppress_activate: bool,
    /// 通常のアクティベート処理要求（ドア開放やコンテナ表示など）がスクリプトから発行されたか
    pub trigger_default_activate: bool,
}

impl ScriptInstanceContext {
    /// 新しいインスタンスコンテキストを作成する。
    pub fn new(script_form_id: FormId) -> Self {
        Self {
            script_form_id,
            local_vars: HashMap::new(),
            suppress_activate: false,
            trigger_default_activate: false,
        }
    }
}

/// ゲーム内イベントを収集・順次実行するイベントディスパッチャー。
#[derive(Default)]
pub struct EventDispatcher {
    /// 未処理イベントキュー
    event_queue: VecDeque<GameEvent>,
    /// 各配置オブジェクト FormID ごとのスクリプトインスタンス状態
    pub instances: HashMap<FormId, ScriptInstanceContext>,
    /// キャッシュされたスクリプトレコードマップ (FormID -> ScptRecord)
    pub scripts: HashMap<FormId, ScptRecord>,
    /// パース済みスクリプトブロックキャッシュ (FormID -> Vec<ScriptBlock>)
    pub parsed_blocks: HashMap<FormId, Vec<ScriptBlock>>,
}

impl EventDispatcher {
    /// 新しいディスパッチャーを作成する。
    pub fn new() -> Self {
        Self::default()
    }

    /// スクリプトレコードを登録・ブロック構文解析を事前キャッシュする。
    pub fn register_script(&mut self, scpt: ScptRecord) {
        let blocks = scpt.parse_blocks();
        let form_id = scpt.form_id;
        self.parsed_blocks.insert(form_id, blocks);
        self.scripts.insert(form_id, scpt);
    }

    /// オブジェクトにスクリプトをアタッチする。
    pub fn attach_script(&mut self, object_id: FormId, script_id: FormId) {
        let entry = self.instances.entry(object_id).or_insert_with(|| ScriptInstanceContext::new(script_id));
        entry.script_form_id = script_id;
    }

    /// イベントをキューに追加する。
    pub fn push_event(&mut self, event: GameEvent) {
        self.event_queue.push_back(event);
    }

    /// オブジェクトのアクティベートイベントを即時ディスパッチする。
    /// 戻り値:
    /// - `true`: スクリプトの `OnActivate` ブロックが実行され、デフォルト動作が抑制された（または遅延実行フラグが立った）。
    /// - `false`: スクリプトが存在しないか `OnActivate` がないため、直ちにデフォルト動作を実行してよい。
    pub fn dispatch_activate(
        &mut self,
        target: FormId,
        actor: FormId,
        vm: &mut ScriptVm,
    ) -> Result<bool, ScriptError> {
        let Some(instance) = self.instances.get_mut(&target) else {
            return Ok(false);
        };

        let script_id = instance.script_form_id;
        let Some(blocks) = self.parsed_blocks.get(&script_id) else {
            return Ok(false);
        };

        // OnActivate ブロックの存在確認
        let on_activate_blocks: Vec<ScriptBlock> = blocks
            .iter()
            .filter(|b| b.event_type == ScriptEventType::OnActivate)
            .cloned()
            .collect();

        if on_activate_blocks.is_empty() {
            return Ok(false);
        }

        // アクティベート抑制フラグをセット
        instance.suppress_activate = true;
        instance.trigger_default_activate = false;

        // ローカル変数を VM に同期
        vm.locals = instance.local_vars.clone();

        // 各 OnActivate ブロックを実行
        for block in on_activate_blocks {
            // ブロック引数 (例: "player") の一致判定
            if !block.args.is_empty() {
                let target_arg = block.args[0].to_ascii_lowercase();
                // "player" 指定で発火者がプレイヤー (FormId 0x14) でない場合はスキップ
                if target_arg == "player" && actor != FormId(0x00000014) {
                    continue;
                }
            }

            // VM でブロックを実行 (If/Else/Return/Activate 判定含む)
            let activated = vm.execute_block(&block.lines, Some(target))?;
            if activated {
                instance.trigger_default_activate = true;
                instance.suppress_activate = false;
            }
        }

        // 実行後のローカル変数をインスタンスストレージに書き戻し
        instance.local_vars = vm.locals.clone();

        Ok(instance.suppress_activate || !instance.trigger_default_activate)
    }

    /// キュー内のイベントをすべて順次ディスパッチする。
    pub fn process_queue(&mut self, vm: &mut ScriptVm) -> Result<(), ScriptError> {
        while let Some(event) = self.event_queue.pop_front() {
            match event {
                GameEvent::OnActivate { target, actor } => {
                    self.dispatch_activate(target, actor, vm)?;
                }
                _ => {
                    // その他のイベント種別（OnAdd, GameMode 等）の処理
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fo3_esm::records::scpt::ScriptHeader;

    #[test]
    fn test_event_dispatcher_on_activate_suppression() {
        let mut dispatcher = EventDispatcher::new();
        let mut vm = ScriptVm::default();

        let script_text = r#"
scn MoriartyDoorScript
short bLocked

Begin OnActivate player
    if ( bLocked == 1 )
        Return
    endif
    Activate
End
"#;

        let scpt = ScptRecord {
            form_id: FormId(0x100),
            edid: "MoriartyDoorScript".to_string(),
            header: ScriptHeader {
                ref_count: 0,
                compiled_size: 0,
                variable_count: 1,
                script_type: fo3_esm::records::scpt::ScriptType::Object,
                flags: 1,
            },
            bytecode: Vec::new(),
            source_text: Some(script_text.to_string()),
            local_vars: Vec::new(),
            ref_objects: Vec::new(),
        };

        dispatcher.register_script(scpt);

        let door_id = FormId(0x200);
        let player_id = FormId(0x14);

        dispatcher.attach_script(door_id, FormId(0x100));

        // 初期状態: Activate が実行され、suppress されない (false = 通常通り開く)
        let suppressed = dispatcher.dispatch_activate(door_id, player_id, &mut vm).unwrap();
        assert!(!suppressed, "Activate が呼ばれたため抑制は解除されるべき");

        // bLocked = 1 に設定
        dispatcher.instances.get_mut(&door_id).unwrap().local_vars.insert("blocked".to_string(), 1.0);
        // 再実行: Activate に到達せず Return するため抑制される (true = 通常開閉をブロック)
        let suppressed_locked = dispatcher.dispatch_activate(door_id, player_id, &mut vm).unwrap();
        assert!(suppressed_locked, "bLocked == 1 のため Activate が呼ばれず抑制されるべき");
    }
}
