//! # スクリプトオペコードおよび関数テーブル
//!
//! 参照元:
//! - `references/openmw/components/esm4/script.hpp`
//! - `references/bevyout/docs/plans/M7_SCRIPTING_ARCHITECTURE_ROADMAP.md`
//! - `references/openmw/components/esm4/loadinfo.cpp:81-105`

/// スクリプト基本オペコード (SCDA バイトコード)
pub mod opcodes {
    /// スクリプトイベントブロック開始 (`Begin [Event]`)
    pub const OP_BEGIN: u16 = 0x0010;
    /// ブロック終了 (`End`)
    pub const OP_END: u16 = 0x0011;
    /// 短整数ローカル変数宣言 (`short`)
    pub const OP_SHORT: u16 = 0x0012;
    /// 長整数ローカル変数宣言 (`long`)
    pub const OP_LONG: u16 = 0x0013;
    /// 浮動小数点ローカル変数宣言 (`float`)
    pub const OP_FLOAT: u16 = 0x0014;
    /// 変数代入 (`set [var] to [expr]`)
    pub const OP_SET: u16 = 0x0015;
    /// 条件分岐開始 (`if [condition]`)
    pub const OP_IF: u16 = 0x0016;
    /// 条件分岐代替 (`else`)
    pub const OP_ELSE: u16 = 0x0017;
    /// 条件分岐継続 (`elseif`)
    pub const OP_ELSEIF: u16 = 0x0018;
    /// 条件分岐終了 (`endif`)
    pub const OP_ENDIF: u16 = 0x0019;
    /// 早期脱出 (`return`)
    pub const OP_RETURN: u16 = 0x001E;
}

/// スクリプトおよび CTDA 条件式で使用される組み込み関数 ID
pub mod functions {
    /// オブジェクト間距離取得 (`GetDistance [Target]`)
    pub const FN_GET_DISTANCE: u16 = 0x0035;
    /// 同一セル存在判定 (`GetInSameCell [Target]`)
    pub const FN_GET_IN_SAME_CELL: u16 = 0x0038;
    /// 対象 Base FormID 判定 (`GetIsID [BaseFormID]`)
    /// 参照元: `references/openmw/components/esm4/script.hpp:114` (FUN_GetIsID = 72 = 0x0048)
    pub const FN_GET_IS_ID: u16 = 0x0048;
    /// クエスト実行中判定 (`GetQuestRunning [QuestFormID]`)
    /// 参照元: `references/openmw/components/esm4/script.hpp:99` (FUN_GetQuestRunning = 56 = 0x0038)
    pub const FN_GET_QUEST_RUNNING: u16 = 0x0038;
    /// クエストステージ取得 (`GetStage <QuestID>`)
    /// 参照元: `references/openmw/components/esm4/script.hpp:100` (FUN_GetStage = 58 = 0x003A)
    pub const FN_GET_STAGE: u16 = 0x003A;
    /// クエストステージ到達判定 (`GetStageDone <QuestID> <Stage>`)
    /// 参照元: `references/openmw/components/esm4/script.hpp:101` (FUN_GetStageDone = 59 = 0x003B)
    pub const FN_GET_STAGE_DONE: u16 = 0x003B;
    /// クエストステージ設定 (`SetStage [QuestFormID] [Stage]`)
    pub const FN_SET_STAGE: u16 = 0x0049;
    /// クエスト開始 (`StartQuest [QuestFormID]`)
    pub const FN_START_QUEST: u16 = 0x004A;
    /// クエスト停止 (`StopQuest [QuestFormID]`)
    pub const FN_STOP_QUEST: u16 = 0x004B;
    /// アイテム所持数取得 (`GetItemCount [ItemFormID]`)
    pub const FN_GET_ITEM_COUNT: u16 = 0x0050;
    /// アイテム追加 (`AddItem [ItemFormID] [Count]`)
    pub const FN_ADD_ITEM: u16 = 0x0052;
    /// アイテム除去 (`RemoveItem [ItemFormID] [Count]`)
    pub const FN_REMOVE_ITEM: u16 = 0x0053;
    /// オブジェクトアクティベート (`Activate [Activator]`)
    pub const FN_ACTIVATE: u16 = 0x0066;
    /// オブジェクト有効化 (`Enable`)
    pub const FN_ENABLE: u16 = 0x0070;
    /// オブジェクト無効化 (`Disable`)
    pub const FN_DISABLE: u16 = 0x0071;
    /// オブジェクト無効化判定 (`GetDisabled`)
    pub const FN_GET_DISABLED: u16 = 0x0072;
    /// ドア・コンテナ解錠 (`Unlock`)
    pub const FN_UNLOCK: u16 = 0x0078;
    /// ドア・コンテナ施錠 (`Lock [Level]`)
    pub const FN_LOCK: u16 = 0x0079;
    /// セリフ発話 (`Say [TopicFormID]`)
    pub const FN_SAY: u16 = 0x00A0;
    /// 会話開始 (`StartConversation [ActorFormID]`)
    pub const FN_START_CONVERSATION: u16 = 0x00A1;
}
