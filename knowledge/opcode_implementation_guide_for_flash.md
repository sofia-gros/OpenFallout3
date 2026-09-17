# Flash エージェント向け：オペコード量産実装ガイド

本ドキュメントは、Fallout 3 に存在する数百種類のスクリプト関数（オペコード）を、軽量モデル（3.8 Flash等）が並列かつ機械的に実装していくための「下地」および作業手順書です。

## 1. アーキテクチャの基本方針

現在の st_vm.rs にある巨大な match 文は、オペコードが増えると **1,000行上限ルール（Rule 3）** に抵触します。
そのため、オペコードは以下のカテゴリごとに別ファイルに分割して実装してください。

### ディレクトリ構造 (想定)
\\\
crates/fo3_script/src/
  ├── functions/
  │   ├── mod.rs          (各カテゴリの関数を統合して呼び出すディスパッチャ)
  │   ├── actor.rs        (NPC/アクター関連: GetIsSex, GetDistance, Kill 等)
  │   ├── item.rs         (インベントリ関連: AddItem, RemoveItem, EquipItem 等)
  │   ├── quest.rs        (クエスト関連: SetStage, GetStage, SetObjectiveDisplayed 等)
  │   ├── state.rs        (状態・ワールド関連: Enable, Disable, Unlock 等)
  │   └── math.rs         (演算・ユーティリティ: GetSecondsPassed, GetRandomPercent 等)
\\\

## 2. オペコード実装の手順 (Flash向け)

Flashエージェントは、以下の手順を厳格に守ってオペコードを追加してください。

### Step 1: 仕様の特定 (Zero Speculation)
- 外部Web検索を行う前に、\eferences/openmw/components/esm4/script.hpp\ 内の \FUN_\ で始まる定数一覧（例: \FUN_GetIsSex\, \FUN_GetDistance\）を参照し、存在確認を行ってください。
- 実装対象の関数の引数仕様（GECK仕様）については、必ず既存の動作や引数の型（FormIDなのか数値なのか）を確認してください。推測で実装してはいけません。

### Step 2: 実装コードのテンプレート
カテゴリファイル（例: \ctor.rs\）内に、以下のような関数シグネチャで実装を追加します。

\\\ust
use crate::parser::Expr;
use crate::vm::{ScriptVm, ScriptError};
use fo3_esm::FormId;

pub fn execute_actor_function(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<Option<f32>, ScriptError> {
    let get_form_id = |expr: &Expr| -> Result<FormId, ScriptError> {
        match expr {
            Expr::Number(n) => Ok(FormId(*n as u32)),
            Expr::Variable(v) => vm.resolve_form_id(v),
            _ => Err(ScriptError::InvalidArguments("Expected FormId".to_string())),
        }
    };

    match cmd {
        "getissex" => {
            let target = subject.unwrap_or(FormId(0x14)); // Player fallback
            let sex_arg = vm.eval_ast_expr(&args[0], subject)?;
            // TODO: VMから対象のアクターレコードを引き、性別を判定して返す
            // return Ok(Some(if is_male { 0.0 } else { 1.0 }));
            Ok(Some(0.0))
        }
        "kill" => {
            let target = subject.unwrap_or(FormId(0x14));
            // TODO: VMから対象のアクターのHealthを0にする処理
            Ok(Some(0.0))
        }
        _ => Ok(None), // このカテゴリに存在しない関数
    }
}
\\\

### Step 3: \unctions/mod.rs\ への登録
各カテゴリの \xecute_xxx_function\ を順番に呼び出し、処理されたら (Some が返ったら) 終了するディスパッチャを実装します。

\\\ust
pub fn dispatch(
    cmd: &str,
    args: &[Expr],
    subject: Option<FormId>,
    vm: &mut ScriptVm,
) -> Result<bool, ScriptError> {
    let lower_cmd = cmd.to_ascii_lowercase();

    // カテゴリ順に評価
    if let Some(res) = actor::execute_actor_function(&lower_cmd, args, subject, vm)? {
        return Ok(res != 0.0);
    }
    if let Some(res) = quest::execute_quest_function(&lower_cmd, args, subject, vm)? {
        return Ok(res != 0.0);
    }
    
    // 見つからない場合は未実装としてログを出す
    println!("[Script] WARNING: Unimplemented function: {}", cmd);
    Ok(false)
}
\\\

### Step 4: \st_vm.rs\ の書き換え
\Statement::Call\ の処理を、上記の \dispatch\ 関数を呼び出すだけのシンプルな形にリファクタリングします。

## 3. Flash エージェントへの指示プロンプト例

ユーザーまたはメインエージェントは、サブエージェント(Flash)を起動する際、以下のプロンプトを使用してください。

> 「\knowledge/opcode_implementation_guide_for_flash.md\ を読み込み、\crates/fo3_script/src/functions/\ ディレクトリを作成してオペコードのモジュール分割（actor, item, quest等）を実施してください。その後、\eferences/openmw\ 内の関数一覧を参考に、主要なオペコード30個を機械的に各ファイルへ実装してください。」
