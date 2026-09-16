# Phase 14: NPCアニメーションとダイアログシステム、スクリプトVMの修正

## 1. 静的アニメーション (AnimatedStatic) の修正
fo3_render/src/scene/actor.rs において、kf_nif が存在しない場合（静的メッシュ）にアニメーションが再生されない問題がありました。
静的オブジェクトは自分自身の skeleton_nif 内にアニメーションシーケンスを保持しているため、kf_nif が None の場合はフォールバックとして skeleton_nif を使用して AnimationPlayer を更新するように修正しました。

## 2. NPCのT-Pose (アニメーションフォールバック) の修正
fo3_viewer/src/loader.rs において、resolve_idle_kf_from_pack がアニメーションを返さなかった場合（AIパッケージに IdleCollection が設定されていない場合など）に、NPCがT-Poseで固まる問題がありました。
これに対し、ハードコードで meshes\characters\_male\idleanims\mtidle.kf をデフォルトの待機アニメーションとしてフォールバック適用するロジックを追加し、フリーズを解決しました。

## 3. ダイアログシステムの音声途切れ問題の修正
fo3_script/src/vm.rs および fo3_viewer/src/audio.rs に関連する問題です。
Say や SayTo などのコマンドがクエストの Result Script から呼び出される際、従来はスクリプトの実行コンテキスト (self_id) が全て親クエスト（例: CG00）となっていました。
これにより、Dad、Mom、Dr. Li がすべて「CG00」という同じ話者IDで音声を再生しようとし、active_subtitles (HashMap) のキーが衝突して前の音声が上書き・停止されていました。
これを解決するため、ドット記法 (CG00DadREF.say) で呼び出された場合は、ドットの前のプレフィックスから解決した effective_self_id を話者として say_queue に渡すように修正しました。

## 4. スクリプトVM (vm.rs) の変数管理 (set コマンド) の修正
set コマンドによる変数代入時、クエスト変数（CG00.timer など）が正しくクエストマネージャーに反映されていませんでした。
ドット記法で指定された場合（prefix.sub）、prefix を edid_map から解決して対象クエストの FormID を特定し、quest_manager.set_quest_variable を呼び出して反映させるよう修正しました。この際、型の不一致を防ぐため f64 へのキャストを行っています。
