# Fallout 3 実ゲームアセット環境設定

本環境における Fallout 3 GOTY 実ゲームアセットの配置パスおよび主要アーカイブの一覧です。
テストやアセット抽出検証時には以下のパスを参照します。

## 1. パス構成
- **ゲームルート**: `A:\SteamLibrary\steamapps\common\Fallout 3 goty`
- **Data ディレクトリ**: `A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data`

## 2. 主要アーカイブ (.bsa / .esm)
- `Data/Fallout3.esm` (マスターレコード, 288MB)
- `Data/Fallout - Meshes.bsa` (全メッシュ・NIF アーカイブ, 748MB)
- `Data/Fallout - Textures.bsa` (全テクスチャ・DDS アーカイブ, 1.1GB)
- `Data/Fallout - Sound.bsa` (効果音, 855MB)
- `Data/Fallout - Voices.bsa` (音声, 775MB)

## 3. ルーズアセット (.nif) 検証パス例
`Data/Meshes/` 配下にルーズな NIF ファイルが多数存在し、`fo3_testbed` で直接パース検証が可能です:
- `A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data\Meshes\StreetLights\lamppost01ONOFF.nif`
- `A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data\Meshes\StreetLights\nukasignbigGLOW.nif`
- `A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data\Meshes\StreetLights\parkinglotlight01ONOFF.nif`
