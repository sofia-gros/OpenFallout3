//! # fo3_testbed
//!
//! Fallout 3 アセット（NIF / BSA / VFS）検証用 CLI ツール。
//!
//! - NIF パース検証: `fo3_testbed nif <path/to/file.nif>`
//! - NIF 詳細ダンプ: `fo3_testbed nif-dump <path/to/file.nif>`
//! - BSA ファイル一覧表示: `fo3_testbed bsa-list <path/to/archive.bsa>`
//! - BSA ファイル抽出検証: `fo3_testbed bsa-extract <path/to/archive.bsa> <relative/path>`
//! - VFS 統合読み込み検証: `fo3_testbed vfs-test <data_dir> <relative/path>`

use std::env;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;
use fo3_bsa::BsaArchive;
use fo3_esm::EsmReader;
use fo3_gamebryo_core::Vec3;
use fo3_nif::{NifBlock, NifFile, NifHeader};
use fo3_vfs::VfsManager;

fn print_usage() {
    println!("使用法:");
    println!("  cargo run -p fo3_testbed -- nif <path/to/mesh.nif>");
    println!("  cargo run -p fo3_testbed -- nif-dump <path/to/mesh.nif>");
    println!("  cargo run -p fo3_testbed -- bsa-list <path/to/archive.bsa> [filter]");
    println!("  cargo run -p fo3_testbed -- bsa-extract <path/to/archive.bsa> <relative/path>");
    println!("  cargo run -p fo3_testbed -- vfs-test <data_dir> <relative/path>");
    println!("  cargo run -p fo3_testbed -- vfs-nif-dump <data_dir> <relative/path>");
    println!("  cargo run -p fo3_testbed -- esm-header <path/to/file.esm>");
    println!("  cargo run -p fo3_testbed -- esm-groups <path/to/file.esm>");
    println!("  cargo run -p fo3_testbed -- esm-stat <path/to/file.esm> [limit]");
    println!("  cargo run -p fo3_testbed -- esm-ltex <path/to/file.esm> [limit]");
    println!("  cargo run -p fo3_testbed -- esm-cell <path/to/file.esm> <cell_edid>");
    println!("  cargo run -p fo3_testbed -- collision-batch <data_dir> [limit]");
    println!("  cargo run -p fo3_testbed -- collision-lines <data_dir> <relative/path>");
    println!("  cargo run -p fo3_testbed -- skin-test <data_dir> <relative/path>");
    println!("  cargo run -p fo3_testbed -- anim-test <data_dir> <relative/path> <kf_path>");
}

fn test_nif(nif_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIF パース検証: {} ===", nif_path);

    let file = File::open(nif_path)?;
    let mut reader = BufReader::new(file);

    let header = NifHeader::read(&mut reader)?;

    println!("ヘッダー文字列: {}", header.header_string.trim());
    println!("NIF バージョン: {:#X}", header.version);
    println!("ユーザーバージョン: {}", header.user_version);
    println!("BS バージョン: {}", header.bs_header.bs_version);
    println!("著者情報: {}", header.bs_header.author.value);
    println!("エクスポートスクリプト: {}", header.bs_header.export_script.value);
    println!("ブロック総数: {}", header.num_blocks);
    println!("使用ブロック型数: {}", header.block_types.len());
    println!("ブロック型一覧:");
    for (i, btype) in header.block_types.iter().enumerate() {
        println!("  [{}] {}", i, btype);
    }
    println!("グローバル文字列数: {}", header.strings.len());

    println!("\nヘッダーパース成功！");
    Ok(())
}

fn dump_nif<R: std::io::BufRead>(reader: &mut R, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIF 詳細ブロックダンプ: {} ===", title);

    let nif = NifFile::read(reader)?;

    println!("ブロック総数: {}", nif.blocks.len());

    for (i, block) in nif.blocks.iter().enumerate() {
        let type_name = &nif.header.block_types[nif.header.block_type_indices[i] as usize];
        print!("  [{:03}] {:<28}", i, type_name);

        match block {
            NifBlock::NiTriShape(shape) => {
                let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
                println!("名前: {:<20} Data: {:?} Props: {:?}", format!("\"{}\"", name), shape.geom.data, shape.geom.av.properties);
            }
            NifBlock::NiTriShapeData(data) => {
                println!(
                    "頂点数: {:<6} 三角形数: {:<6} UV: {} 法線: {} 接線: {}",
                    data.common.num_vertices,
                    data.num_triangles,
                    data.common.uv_sets.len(),
                    !data.common.normals.is_empty(),
                    data.common.tangents.len(),
                );
                let (min, max) = data.common.vertices.iter().fold(
                    (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                    |(min, max), v| (min.min(Vec3::new(v.x, v.y, v.z)), max.max(Vec3::new(v.x, v.y, v.z))),
                );
                println!("          頂点 Min: {:?}, Max: {:?}", min, max);
            }
            NifBlock::NiTriStrips(strips) => {
                let name = nif.get_string(strips.geom.av.net.name_index).unwrap_or("");
                println!("名前: {:<20} Data: {:?}", format!("\"{}\"", name), strips.geom.data);
                println!("          Trans: {:?}, Scale: {}", strips.geom.av.translation, strips.geom.av.scale);
            }
            NifBlock::NiNode(node) => {
                let name = nif.get_string(node.av.net.name_index).unwrap_or("");
                println!("名前: {:<20} 子ノード数: {}", format!("\"{}\"", name), node.children.len());
                println!("          Trans: {:?}, Scale: {}", node.av.translation, node.av.scale);
            }
            NifBlock::BSFadeNode(fade) => {
                let name = nif.get_string(fade.node.av.net.name_index).unwrap_or("");
                println!("名前: {:<20} 子ノード数: {}", format!("\"{}\"", name), fade.node.children.len());
                println!("          Trans: {:?}, Scale: {}", fade.node.av.translation, fade.node.av.scale);
            }
            NifBlock::NiTriStripsData(data) => {
                println!(
                    "頂点数: {:<6} 三角形数: {:<6} ストリップ数: {} 接線: {}",
                    data.common.num_vertices,
                    data.num_triangles,
                    data.num_strips,
                    data.common.tangents.len(),
                );
                let (min, max) = data.common.vertices.iter().fold(
                    (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                    |(min, max), v| (min.min(Vec3::new(v.x, v.y, v.z)), max.max(Vec3::new(v.x, v.y, v.z))),
                );
                println!("          頂点 Min: {:?}, Max: {:?}", min, max);
            }
            NifBlock::BSShaderTextureSet(set) => {
                println!("スロット数: {}", set.textures.len());
                for (slot, tex) in set.textures.iter().enumerate() {
                    if !tex.is_empty() {
                        println!("          スロット [{}]: {}", slot, tex);
                    }
                }
            }
            NifBlock::BSShaderPPLightingProperty(prop) => {
                println!("TextureSet Ref: {} ShaderType: {}", prop.texture_set, prop.shader_type);
            }
            NifBlock::NiMaterialProperty(mat) => {
                println!("Alpha: {:.2} Glossiness: {:.1}", mat.alpha, mat.glossiness);
            }
            NifBlock::NiAlphaProperty(alpha) => {
                println!("Flags: {:#X} Threshold: {}", alpha.flags, alpha.threshold);
            }
            NifBlock::BhkCollisionObject(obj) => {
                println!("Target: {} Flags: {:#X} Body: {}", obj.target, obj.flags, obj.body);
            }
            NifBlock::BhkRigidBody(body) | NifBlock::BhkRigidBodyT(body) => {
                println!(
                    "Shape: {} Layer: {} Mass: {:.2}kg Motion: {} Trans: {:?}",
                    body.world_obj.shape,
                    body.world_obj.havok_filter_layer,
                    body.mass,
                    body.motion_system,
                    body.translation,
                );
            }
            NifBlock::BhkMoppBvTreeShape(mopp) => {
                println!(
                    "Shape: {} Scale: {:.2} MOPPデータ長: {} bytes Offset: {:?}",
                    mopp.shape,
                    mopp.scale,
                    mopp.mopp_data.len(),
                    mopp.mopp_offset,
                );
            }
            NifBlock::BhkPackedNiTriStripsShape(shape) => {
                println!(
                    "Data: {} Radius: {:.3} Scale: {:?}",
                    shape.data,
                    shape.radius,
                    shape.scale,
                );
            }
            NifBlock::HkPackedNiTriStripsData(data) => {
                println!(
                    "頂点数: {} (圧縮={}) 三角形数: {} サブシェイプ数: {}",
                    data.vertices.len(),
                    data.is_compressed,
                    data.triangles.len(),
                    data.sub_shapes.len(),
                );
                if let Some(first_sub) = data.sub_shapes.first() {
                    println!(
                        "          SubShape[0]: Layer={}, Vertices={}, Material={:#X}",
                        first_sub.havok_filter_layer,
                        first_sub.num_vertices,
                        first_sub.material,
                    );
                }
                if let (Some(first), Some(last)) = (data.vertices.first(), data.vertices.last()) {
                    println!("          Vert[0]: {:?}, Vert[last]: {:?}", first, last);
                }
            }
            NifBlock::BhkBoxShape(box_shape) => {
                println!("Radius: {:.3} Dimensions: {:?}", box_shape.radius, box_shape.dimensions);
            }
            NifBlock::BhkSphereShape(sphere) => {
                println!("Radius: {:.3} Material: {:#X}", sphere.radius, sphere.material);
            }
            NifBlock::BhkCapsuleShape(capsule) => {
                println!("Pt1: {:?} (r={:.2}), Pt2: {:?} (r={:.2})", capsule.first_point, capsule.radius1, capsule.second_point, capsule.radius2);
            }
            NifBlock::BhkBlendCollisionObject(blend) => {
                println!("Target: {} Flags: {:#X} Body: {} HeirGain: {:.2} VelGain: {:.2}", blend.col.target, blend.col.flags, blend.col.body, blend.heir_gain, blend.vel_gain);
            }
            NifBlock::BhkConvexVerticesShape(convex) => {
                println!("Radius: {:.3} 頂点数: {} 法線数: {}", convex.radius, convex.vertices.len(), convex.normals.len());
                if let (Some(first), Some(last)) = (convex.vertices.first(), convex.vertices.last()) {
                    println!("          Vert[0]: {:?}, Vert[last]: {:?}", first, last);
                }
            }
            NifBlock::BhkConvexTransformShape(ct) => {
                println!("Shape: {} Material: {:#X} Radius: {:.3}", ct.shape, ct.material, ct.radius);
            }
            NifBlock::BhkConvexListShape(list) => {
                println!("子凸形状数: {} Material: {:#X}", list.sub_shapes.len(), list.material);
            }
            NifBlock::BhkListShape(list) => {
                println!("子形状数: {} フィルター数: {}", list.sub_shapes.len(), list.filters.len());
            }
            NifBlock::BSShaderNoLightingProperty(prop) => {
                println!("File: \"{}\" Clamp: {}", prop.file_name, prop.texture_clamp_mode);
            }
            NifBlock::NiStencilProperty(sten) => {
                println!("Flags: {:#06X} DrawMode: {} DoubleSided: {}", sten.flags, sten.draw_mode(), sten.is_double_sided());
            }
            NifBlock::BSXFlags(bsx) => {
                println!("Flags: {:#010X} Havok: {} Col: {} Anim: {}", bsx.flags, bsx.has_havok(), bsx.has_collision(), bsx.has_animation());
            }
            NifBlock::NiStringExtraData(extra) => {
                let s = nif.get_string(extra.string_data_index).unwrap_or("");
                println!("String: \"{}\"", s);
            }
            NifBlock::NiIntegerExtraData(extra) => {
                println!("Int: {}", extra.integer_data);
            }
            NifBlock::NiFloatExtraData(extra) => {
                println!("Float: {:.3}", extra.float_data);
            }
            NifBlock::BSBound(bound) => {
                println!("Center: {:?} Dim: {:?}", bound.center, bound.dimensions);
            }
            NifBlock::BhkSPCollisionObject(obj) => {
                println!("SP Target: {} Body: {}", obj.target, obj.body);
            }
            NifBlock::BhkTransformShape(ts) => {
                println!("TransformShape Shape: {} Mat: {:?}", ts.shape, ts.material);
            }
            NifBlock::BhkNiTriStripsShape(ss) => {
                println!("NiTriStripsShape Strips: {} Mat: {:?}", ss.strips_data.len(), ss.material);
            }
            NifBlock::BhkSimpleShapePhantom(p) => {
                println!("SimpleShapePhantom Shape: {}", p.common.shape);
            }
            NifBlock::BSDismemberSkinInstance(bdsi) => {
                println!(
                    "Data: {} Partition: {} SkeletonRoot: {} ボーン数: {} DismemberParts: {}",
                    bdsi.skin_instance.data,
                    bdsi.skin_instance.skin_partition,
                    bdsi.skin_instance.skeleton_root,
                    bdsi.skin_instance.bones.len(),
                    bdsi.partitions.len()
                );
            }
            NifBlock::NiSkinData(sd) => {

                println!(
                    "ルート変換 Scale: {:.3} ボーン数: {}",
                    sd.skin_transform_scale,
                    sd.bone_list.len()
                );
            }
            NifBlock::NiSkinInstance(inst) => {
                println!(
                    "Data: {} Partition: {} SkeletonRoot: {} ボーン数: {}",
                    inst.data,
                    inst.skin_partition,
                    inst.skeleton_root,
                    inst.bones.len()
                );
            }
            NifBlock::NiSkinPartition(part) => {
                println!("パーティション数: {}", part.partitions.len());
                for (pi, p) in part.partitions.iter().enumerate() {
                    println!(
                        "          [{}] 頂点数: {} 三角形数: {} ボーン数: {}",
                        pi, p.num_vertices, p.num_triangles, p.num_bones
                    );
                }
            }
            NifBlock::NiStringPalette(pal) => {
                println!("パレットバッファ長: {} バイト", pal.palette.len());
            }
            NifBlock::NiTransformInterpolator(interp) => {
                println!(
                    "Trans: {:?} Scale: {:.3} DataRef: {}",
                    interp.transform.translation, interp.transform.scale, interp.data
                );
            }
            NifBlock::NiTransformData(data) => {
                println!(
                    "回転キー数: {} (Type={:?}), 移動キー数: {}, スケールキー数: {}",
                    data.quaternion_keys.len(),
                    data.rotation_type,
                    data.translations.keys.len(),
                    data.scales.keys.len()
                );
            }
            NifBlock::NiControllerSequence(seq) => {
                let name = nif.get_string(seq.name_index as u32).unwrap_or("");
                println!(
                    "名前: \"{}\" 時間: {:.2}s - {:.2}s 制御ブロック数: {} サイクル: {}",
                    name, seq.start_time, seq.stop_time, seq.controlled_blocks.len(), seq.cycle_type
                );
            }
            NifBlock::NiBSplineBasisData(b) => {
                println!("制御点数: {}", b.num_control_points);
            }
            NifBlock::NiBSplineData(d) => {
                println!(
                    "Float制御点: {} 点, Compact制御点: {} 点",
                    d.float_control_points.len(),
                    d.compact_control_points.len()
                );
            }
            NifBlock::NiBSplineCompTransformInterpolator(interp) => {
                println!(
                    "時間: {:.2}s - {:.2}s, SplineData: {}, Basis: {}, Trans: {:?}",
                    interp.start_time, interp.stop_time, interp.spline_data, interp.basis_data, interp.transform.translation
                );
            }
            NifBlock::Unknown { type_name: _, data } => {
                println!("(未対応/スキップ - {} バイト)", data.len());
            }



        }
    }

    println!("\n全ブロックの読み込み・ダンプ完了！");
    Ok(())
}

fn test_nif_dump(nif_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(nif_path)?;
    let mut reader = BufReader::new(file);
    dump_nif(&mut reader, nif_path)
}

fn create_vfs(data_dir: &str) -> Result<VfsManager, Box<dyn std::error::Error>> {
    let mut vfs = VfsManager::new();
    let data_path = Path::new(data_dir);
    vfs.add_loose_root(data_path);

    let bsa_names = [
        "Fallout - Meshes.bsa",
        "Fallout - Textures.bsa",
        "Fallout - Misc.bsa",
    ];
    for bsa_name in &bsa_names {
        let bsa_file = data_path.join(bsa_name);
        if bsa_file.exists() {
            let archive = BsaArchive::open(&bsa_file)?;
            println!("BSA マウント: {} (ファイル数: {})", bsa_name, archive.list_files().len());
            vfs.add_bsa(archive);
        }
    }
    Ok(vfs)
}

fn test_vfs_nif_dump(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut vfs = create_vfs(data_dir)?;

    println!("VFS からファイルを読み込み中: {}", relative_path);
    let bytes = vfs.read(relative_path)?;
    println!("ファイル読み込み成功 ({} バイト)。NIF パースを開始...", bytes.len());

    let mut cursor = Cursor::new(bytes);
    dump_nif(&mut cursor, relative_path)
}

fn test_bsa_list(bsa_path: &str, filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== BSA アーカイブ検証: {} ===", bsa_path);

    let archive = BsaArchive::open(bsa_path)?;
    let header = archive.header();

    println!("バージョン: {}", header.version);
    println!("フラグ: {:#X}", header.flags);
    println!("ディレクトリ数: {}", header.folder_count);
    println!("ファイル総数: {}", header.file_count);

    let files = archive.list_files();
    println!("展開ファイル数: {}", files.len());

    if let Some(query) = filter {
        let q = query.to_ascii_lowercase();
        let matched: Vec<_> = files.iter().filter(|f| f.to_ascii_lowercase().contains(&q)).collect();
        println!("\n検索ワード '{}' に一致したファイル ({} 件):", query, matched.len());
        for (i, f) in matched.iter().take(40).enumerate() {
            println!("  [{}] {}", i, f);
        }
        if matched.len() > 40 {
            println!("  ... (他 {} 件)", matched.len() - 40);
        }
    } else {
        println!("\n先頭 10 件のファイル:");
        for (i, f) in files.iter().take(10).enumerate() {
            println!("  [{}] {}", i, f);
        }

        if files.len() > 10 {
            println!("...\n末尾 5 件のファイル:");
            for (i, f) in files.iter().skip(files.len() - 5).enumerate() {
                println!("  [{}] {}", files.len() - 5 + i, f);
            }
        }
    }

    println!("\nBSA ファイル一覧取得成功！");
    Ok(())
}

fn test_bsa_extract(bsa_path: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== BSA ファイル抽出検証: {} ({}) ===", bsa_path, relative_path);

    let mut archive = BsaArchive::open(bsa_path)?;
    let data = archive.extract_file(relative_path)?;

    println!("抽出成功: {} バイト解凍展開完了", data.len());

    // NIF ファイルの場合は NifHeader でパース検証
    if relative_path.to_ascii_lowercase().ends_with(".nif") {
        println!("\n--- 抽出された NIF データの整合性検証 ---");
        let mut cursor = Cursor::new(&data);
        let header = NifHeader::read(&mut cursor)?;
        println!("NIF バージョン: {:#X}", header.version);
        println!("ブロック総数: {}", header.num_blocks);
        println!("使用ブロック型一覧:");
        for (i, btype) in header.block_types.iter().enumerate() {
            println!("  [{}] {}", i, btype);
        }
        println!("NIF ヘッダーパース検証成功！");
    }

    Ok(())
}

fn test_vfs(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== VFS 統合読み込み検証 ===");
    println!("Data ディレクトリ: {}", data_dir);
    println!("ターゲット相対パス: {}", relative_path);

    let mut vfs = VfsManager::new();
    let data_path = Path::new(data_dir);

    // 1. ルーズファイルルートを登録
    vfs.add_loose_root(data_path);
    println!("ルーズファイルルート登録: OK");

    // 2. BSA アーカイブを自動探索してマウント
    let bsa_names = [
        "Fallout - Meshes.bsa",
        "Fallout - Textures.bsa",
        "Fallout - Misc.bsa",
    ];

    for bsa_name in &bsa_names {
        let bsa_file = data_path.join(bsa_name);
        if bsa_file.exists() {
            let archive = BsaArchive::open(&bsa_file)?;
            println!("BSA マウント成功: {} (ファイル数: {})", bsa_name, archive.list_files().len());
            vfs.add_bsa(archive);
        }
    }

    // 3. ファイル存在判定
    let exists = vfs.exists(relative_path);
    println!("VFS exists 判定: {}", exists);

    // 4. 透過読み込み
    let data = vfs.read(relative_path)?;
    println!("VFS 読み込み成功: {} バイト取得完了", data.len());

    // NIF ファイルの場合はパース検証
    if relative_path.to_ascii_lowercase().ends_with(".nif") {
        println!("\n--- 読み込まれた NIF データの整合性検証 ---");
        let mut cursor = Cursor::new(&data);
        let header = NifHeader::read(&mut cursor)?;
        println!("NIF バージョン: {:#X}", header.version);
        println!("ブロック総数: {}", header.num_blocks);
        println!("NIF ヘッダーパース検証成功！");
    }

    Ok(())
}

fn test_esm_header(esm_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ESM ヘッダーパース検証: {} ===", esm_path);

    let reader = EsmReader::open(esm_path)?;

    println!("TES4 レコードヘッダー:");
    println!("  シグネチャ: {}", reader.header_record.type_id);
    println!("  データサイズ: {} バイト", reader.header_record.data_size);
    println!("  フラグ: {:#010X}", reader.header_record.flags);
    println!("  FormID: {:#010X}", reader.header_record.form_id.0);
    println!("  バージョン: {}", reader.header_record.form_version);

    println!("\nTES4 サブレコード情報:");
    println!("  フォーマットバージョン: {}", reader.header.version);
    println!("  レコード総数: {}", reader.header.num_records);
    println!("  次オブジェクトID: {:#010X}", reader.header.next_object_id);
    println!("  作者: \"{}\"", reader.header.author);
    println!("  説明: \"{}\"", reader.header.description);
    println!("  マスターファイル数: {}", reader.header.masters.len());
    for (i, master) in reader.header.masters.iter().enumerate() {
        println!("    [{}] {}", i, master);
    }

    println!("\nESM ヘッダーパース検証成功！");
    Ok(())
}

fn test_esm_groups(esm_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ESM トップレベルグループ一覧: {} ===", esm_path);

    let mut reader = EsmReader::open(esm_path)?;
    let groups = reader.list_top_groups()?;

    println!("検出トップレベルグループ数: {}", groups.len());
    println!("{:<4} {:<8} {:<12} {:<10}", "No", "タイプ", "サイズ(B)", "グループ種別");
    println!("{:-<40}", "");

    for (i, group) in groups.iter().enumerate() {
        let label_str = String::from_utf8_lossy(&group.label);
        println!(
            "[{:02}] {:<8} {:<12} {}",
            i,
            label_str,
            group.group_size,
            group.group_type
        );
    }

    println!("\nトップレベルグループ一覧取得完了！");
    Ok(())
}

fn test_esm_stat(esm_path: &str, limit: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ESM STAT レコードパース検証: {} ===", esm_path);
    if let Some(lim) = limit {
        println!("最大取得件数: {}", lim);
    }

    let mut reader = EsmReader::open(esm_path)?;
    let stats = reader.read_stat_records(limit)?;

    println!("読み込み件数: {}", stats.len());
    for (i, stat) in stats.iter().enumerate() {
        println!(
            "[{:03}] FormID: {:#010X} | EDID: {:<30} | Model: {}",
            i,
            stat.form_id.0,
            format!("\"{}\"", stat.edid),
            stat.model
        );
    }

    println!("\nSTAT レコードパース検証成功！");
    Ok(())
}

fn test_esm_ltex(esm_path: &str, limit: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ESM LTEX (地形テクスチャ) レコードパース検証: {} ===", esm_path);
    if let Some(lim) = limit {
        println!("最大取得件数: {}", lim);
    }

    println!("=== 地形テクスチャ解決マップ構築検証 ===");
    let mut reader2 = EsmReader::open(esm_path)?;
    let land_tex_map = reader2.read_landscape_texture_map()?;
    println!("解決済み地形テクスチャ数: {} 件", land_tex_map.len());
    let mut count = 0;
    for (fid, (diff, norm)) in &land_tex_map {
        println!("  LTEX {:#010X} => Diffuse: \"{}\", Normal: \"{}\"", fid.0, diff, norm);
        count += 1;
        if count >= limit.unwrap_or(10) {
            break;
        }
    }
    // reader を巻き戻してサブレコードを直接ダンプ
    let mut r = EsmReader::open(esm_path)?;
    while let Some(e) = r.read_next_entry()? {
        if let fo3_esm::EsmEntry::Group(g) = e {
            if g.target_record_type() == Some(fo3_esm::REC_LTEX) {
                while let Some(inner) = r.read_next_entry()? {
                    if let fo3_esm::EsmEntry::Record(hdr, subs) = inner {
                        println!("Record FormID: {:#010X}, Type: {}", hdr.form_id.0, hdr.type_id);
                        for s in subs {
                            let str_val = if s.data.iter().all(|&b| b >= 0x20 && b <= 0x7E || b == 0) {
                                format!(" (ASCII: \"{}\")", s.as_string())
                            } else {
                                String::new()
                            };
                            println!("  Subrecord: {} ({} bytes){}", s.type_id, s.data.len(), str_val);
                        }
                        break;
                    }
                }
                break;
            }
        }
    }

    println!("\nLTEX レコードパース検証成功！");
    Ok(())
}

fn test_esm_cell(esm_path: &str, target_edid: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ESM セル検索 & REFR 抽出検証: セル \"{}\" ({}) ===", target_edid, esm_path);

    let mut reader = EsmReader::open(esm_path)?;

    println!("全 3D モデル保持レコード (STAT, SCOL, DOOR, ACTI, FURN, etc.) のマップを構築中...");
    let model_map = reader.read_all_models_map()?;
    println!("モデル登録総件数: {} 件", model_map.len());

    println!("全 LIGHT レコードのマップを構築中...");
    let light_map = reader.read_light_map()?;
    println!("光源レコード登録総件数: {} 件", light_map.len());

    println!("セル \"{}\" を探索中...", target_edid);
    let result = reader.find_cell_by_edid(target_edid)?;

    match result {
        Some((cell, refrs, land)) => {
            println!("\n【セル情報】");
            println!("  FormID: {:#010X}", cell.form_id.0);
            println!("  EDID: {}", cell.edid);
            println!("  表示名: {:?}", cell.full_name);
            println!("  フラグ: {:#06X} (Interior: {})", cell.cell_flags, cell.is_interior());
            if let Some((x, y)) = cell.grid {
                println!("  グリッド座標: ({}, {})", x, y);
            }
            if let Some(ref lgt) = cell.lighting {
                println!("\n【セル環境照明 (XCLL)】");
                println!("  環境光 (Ambient):      RGBA({:?})", lgt.ambient);
                println!("  指向性光 (Directional): RGBA({:?}) | RotXY: {}, RotZ: {}", lgt.directional, lgt.rotation_xy, lgt.rotation_z);
                println!("  フォグ色 (Fog):         RGBA({:?}) | Near: {:.1}, Far: {:.1}, Clip: {:.1}, Power: {:.2}", lgt.fog_color, lgt.fog_near, lgt.fog_far, lgt.fog_clip_dist, lgt.fog_power);
            } else if let Some(ltmp) = cell.lighting_template {
                println!("\n【セル環境照明】テンプレート参照 FormID: {:#010X} (Flags: {:?})", ltmp.0, cell.lighting_template_flags);
            } else {
                println!("\n【セル環境照明 (XCLL)】なし (デフォルト屋外光または天候制御)");
            }
            if let Some(ref l) = land {
                println!("\n【地形 (LAND) 情報】");
                println!("  FormID: {:#010X}", l.form_id.0);
                println!("  標高オフセット: {:.1}", l.height_offset);
                let heights = l.compute_heights();
                let (min_h, max_h) = heights.iter().fold(
                    (f32::INFINITY, f32::NEG_INFINITY),
                    |(min, max), &h| (min.min(h), max.max(h)),
                );
                println!("  復元標高範囲: {:.1} ～ {:.1} (高低差: {:.1})", min_h, max_h, max_h - min_h);
                println!("  法線データ保持: {}", l.normals.is_some());
                println!("  頂点色保持: {}", l.vertex_colors.is_some());
                println!("  ベーステクスチャ (BTXT): [Q0: {:#010X}, Q1: {:#010X}, Q2: {:#010X}, Q3: {:#010X}]", l.base_textures[0].0, l.base_textures[1].0, l.base_textures[2].0, l.base_textures[3].0);
                println!("  追加テクスチャレイヤー数 (ATXT/VTXT): {} 件", l.layers.len());
            } else {
                println!("\n【地形 (LAND) 情報】なし (屋内セルまたは地形非保持)");
            }

            println!("\n【配置参照オブジェクト (REFR) 総数: {} 件】", refrs.len());
            let mut resolved_count = 0;
            let mut light_count = 0;
            for (i, refr) in refrs.iter().enumerate() {
                let model_info = if let Some(light) = light_map.get(&refr.base_object) {
                    resolved_count += 1;
                    light_count += 1;
                    format!("LIGHT: \"{}\" -> 半径: {}, 色: RGBA({:?}){}", light.edid, light.radius, light.colour, light.model.as_deref().map(|m| format!(" [Mesh: {}]", m)).unwrap_or_default())
                } else if let Some(info) = model_map.get(&refr.base_object) {
                    resolved_count += 1;
                    format!("{}: \"{}\" -> {}", info.record_type, info.edid, info.model)
                } else {
                    format!("Base: {:#010X}", refr.base_object.0)
                };

                if i < 30 || i >= refrs.len().saturating_sub(5) {
                    println!(
                        "  [{:03}] FormID: {:#010X} | Pos: [{:>8.1}, {:>8.1}, {:>8.1}] | Rot: [{:>5.2}, {:>5.2}, {:>5.2}] | Scale: {:.2} | {}",
                        i,
                        refr.form_id.0,
                        refr.position[0], refr.position[1], refr.position[2],
                        refr.rotation[0], refr.rotation[1], refr.rotation[2],
                        refr.scale,
                        model_info
                    );
                } else if i == 30 {
                    println!("  ... (中略: 残り {} 件) ...", refrs.len().saturating_sub(35));
                }
            }
            println!("\n3D オブジェクト解決数: {} / {} (うち配置光源 LIGHT: {} 件)", resolved_count, refrs.len(), light_count);
            let mut unresolved_set = std::collections::BTreeSet::new();
            for refr in &refrs {
                if !model_map.contains_key(&refr.base_object) {
                    unresolved_set.insert(refr.base_object);
                }
            }
            println!("未解決 Base FormID 数: {} 件", unresolved_set.len());
            for fid in &unresolved_set {
                println!("  未解決 FormID: {:#010X}", fid.0);
            }

            println!("\nESM 全体を走査して未解決 FormID のレコード種別を調査中...");
            let found_unresolved = scan_records_for_formids(esm_path, &unresolved_set)?;
            for (fid, (rtype, edid, model)) in found_unresolved {
                println!("  FormID {:#010X} => レコード型: {}, EDID: {:?}, MODL: {:?}", fid.0, rtype, edid, model);
            }

            println!("セル検証完了！");
        }
        None => {
            println!("セル \"{}\" は見つかりませんでした。", target_edid);
        }
    }

    Ok(())
}

fn test_esm_world_dump(esm_path: &str, world_edid: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::SeekFrom;
    use fo3_esm::{EsmEntry, GroupHeader};
    use fo3_esm::types::{REC_CELL, REC_REFR};

    println!("=== ESM ワールド空間詳細ダンプ: \"{}\" ({}) ===", world_edid, esm_path);
    let mut reader = EsmReader::open(esm_path)?;
    let (world_rec, group_start, group_end) = reader
        .find_world_by_edid(world_edid)?
        .expect("World not found");

    println!("ワールド発見: \"{}\" (FormID: {:#010X})", world_rec.edid, world_rec.form_id.0);
    println!("グループ範囲: {} .. {} (サイズ: {} バイト)", group_start, group_end, group_end - group_start);

    reader.seek(SeekFrom::Start(group_start))?;

    fn dump_group_recursive<R: std::io::Read + std::io::Seek>(
        reader: &mut EsmReader<R>,
        end_pos: u64,
        indent: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pad = "  ".repeat(indent);
        while reader.stream_position()? < end_pos {
            let entry = match reader.read_next_entry()? {
                Some(e) => e,
                None => break,
            };

            match entry {
                EsmEntry::Group(group) => {
                    let group_size = group.group_size as u64 - GroupHeader::SIZE as u64;
                    let inner_end = reader.stream_position()? + group_size;
                    let label_val = u32::from_le_bytes(group.label);
                    println!(
                        "{}GRUP: Type={}, Label={:#010X} (Size: {})",
                        pad, group.group_type, label_val, group.group_size
                    );
                    dump_group_recursive(reader, inner_end, indent + 1)?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == REC_CELL {
                        let cell = fo3_esm::CellRecord::from_record(&header, &subrecords)?;
                        println!(
                            "{}RECORD: CELL FormID={:#010X}, EDID=\"{}\", Grid={:?}",
                            pad, cell.form_id.0, cell.edid, cell.grid
                        );
                    } else if header.type_id == REC_REFR {
                        let refr = fo3_esm::RefrRecord::from_record(&header, &subrecords)?;
                        println!(
                            "{}RECORD: REFR FormID={:#010X}, Base={:#010X}, Pos={:?}",
                            pad, refr.form_id.0, refr.base_object.0, refr.position
                        );
                    } else {
                        println!("{}RECORD: {} FormID={:#010X}", pad, header.type_id, header.form_id.0);
                    }
                }
            }
        }
        Ok(())
    }

    dump_group_recursive(&mut reader, group_end, 0)?;
    Ok(())
}

fn scan_records_for_formids(
    esm_path: &str,
    target_ids: &std::collections::BTreeSet<fo3_esm::FormId>,
) -> Result<std::collections::BTreeMap<fo3_esm::FormId, (fo3_esm::FourCC, String, String)>, Box<dyn std::error::Error>> {
    use std::io::SeekFrom;
    use fo3_esm::{EsmEntry, FourCC, GroupHeader};

    let mut reader = EsmReader::open(esm_path)?;
    let start_pos = 24 + reader.header_record.data_size as u64;
    reader.seek(SeekFrom::Start(start_pos))?;

    let mut results = std::collections::BTreeMap::new();

    fn scan_group<R: std::io::Read + std::io::Seek>(
        reader: &mut EsmReader<R>,
        group_end: u64,
        target_ids: &std::collections::BTreeSet<fo3_esm::FormId>,
        results: &mut std::collections::BTreeMap<fo3_esm::FormId, (fo3_esm::FourCC, String, String)>,
    ) -> std::io::Result<()> {
        let sub_edid = FourCC(*b"EDID");
        let sub_modl = FourCC(*b"MODL");

        while reader.stream_position()? < group_end {
            if let Some(entry) = reader.read_next_entry()? {
                match entry {
                    EsmEntry::Group(group) => {
                        let rtype = group.target_record_type();
                        let inner_end = reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                        if rtype == Some(FourCC(*b"CELL")) || rtype == Some(FourCC(*b"WRLD")) {
                            reader.seek(SeekFrom::Start(inner_end))?;
                        } else {
                            scan_group(reader, inner_end, target_ids, results)?;
                        }
                    }
                    EsmEntry::Record(header, subrecords) => {
                        if target_ids.contains(&header.form_id) {
                            let mut edid = String::new();
                            let mut model = String::new();
                            for sub in &subrecords {
                                if sub.type_id == sub_edid {
                                    edid = sub.as_string();
                                } else if sub.type_id == sub_modl {
                                    model = sub.as_string();
                                }
                            }
                            println!("    発見! FormID {:#010X} ({}): edid={:?}, model={:?}", header.form_id.0, header.type_id, edid, model);
                            results.insert(header.form_id, (header.type_id, edid, model));
                        }
                    }
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    let end_pos = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(start_pos))?;
    scan_group(&mut reader, end_pos, target_ids, &mut results)?;

    Ok(results)
}

fn test_esm_worlds(esm_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::SeekFrom;
    use fo3_esm::{EsmEntry, FourCC, GroupHeader};

    println!("=== ESM ワールドスペース (WRLD) 一覧走査: {} ===", esm_path);
    let mut reader = EsmReader::open(esm_path)?;
    let start_pos = 24 + reader.header_record.data_size as u64;
    reader.seek(SeekFrom::Start(start_pos))?;

    let sub_edid = FourCC(*b"EDID");
    let sub_full = FourCC(*b"FULL");
    let rec_wrld = FourCC(*b"WRLD");

    while let Some(entry) = reader.read_next_entry()? {
        match entry {
            EsmEntry::Group(group) => {
                let rtype = group.target_record_type();
                let group_end = reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                if rtype == Some(rec_wrld) {
                    println!("Top WRLD グループを発見！ワールドスペースを走査中...");
                    while reader.stream_position()? < group_end {
                        if let Some(inner) = reader.read_next_entry()? {
                            match inner {
                                EsmEntry::Record(header, subrecords) => {
                                    if header.type_id == rec_wrld {
                                        let mut edid = String::new();
                                        let mut full = String::new();
                                        for s in &subrecords {
                                            if s.type_id == sub_edid {
                                                edid = s.as_string();
                                            } else if s.type_id == sub_full {
                                                full = s.as_string();
                                            }
                                        }
                                        println!("  WRLD FormID: {:#010X} | EDID: {:<25} | Name: {:?}", header.form_id.0, edid, full);
                                    }
                                }
                                EsmEntry::Group(child_group) => {
                                    let child_end = reader.stream_position()? + (child_group.group_size as u64 - GroupHeader::SIZE as u64);
                                    let group_label_id = u32::from_le_bytes(child_group.label);
                                    // MegatonWorld (0x00000A74) の子グループの場合
                                    if group_label_id == 0x00000A74 {
                                        println!("    -> MegatonWorld (0x00000A74) の子グループ発見 (Type: {})", child_group.group_type);
                                        scan_world_cells(&mut reader, child_end)?;
                                    } else {
                                        reader.seek(SeekFrom::Start(child_end))?;
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    break;
                } else {
                    let rem = group.group_size as u64 - GroupHeader::SIZE as u64;
                    reader.skip(rem)?;
                }
            }
            EsmEntry::Record(rec, _) => {
                reader.skip(rec.data_size as u64)?;
            }
        }
    }

    Ok(())
}

fn scan_world_cells<R: std::io::Read + std::io::Seek>(
    reader: &mut EsmReader<R>,
    group_end: u64,
) -> std::io::Result<()> {
    use fo3_esm::{EsmEntry, FourCC, GroupHeader};
    let sub_edid = FourCC(*b"EDID");
    let sub_full = FourCC(*b"FULL");
    let rec_cell = FourCC(*b"CELL");

    while reader.stream_position()? < group_end {
        if let Some(entry) = reader.read_next_entry()? {
            match entry {
                EsmEntry::Group(group) => {
                    let inner_end = reader.stream_position()? + (group.group_size as u64 - GroupHeader::SIZE as u64);
                    scan_world_cells(reader, inner_end)?;
                }
                EsmEntry::Record(header, subrecords) => {
                    if header.type_id == rec_cell {
                        let mut edid = String::new();
                        let mut full = String::new();
                        for s in &subrecords {
                            if s.type_id == sub_edid {
                                edid = s.as_string();
                            } else if s.type_id == sub_full {
                                full = s.as_string();
                            }
                        }
                        println!("      [CELL] FormID: {:#010X} | EDID: {:<25} | Name: {:?}", header.form_id.0, edid, full);
                    }
                }
            }
        } else {
            break;
        }
    }
    Ok(())
}

/// 実アセットの T-Pose スキニングをヘッドレス検証する。
///
/// スキンメッシュ NIF (`meshes\armor\...\outfit*.nif` など) からボーン階層を解決し、
/// `apply_skinning_cpu_with_bones` でバインドポーズ変形を適用して、
/// 出力頂点が有限値かつ元のメッシュ BBox を保持していることを確認する。
///
/// 参照元: knowledge/actor_and_skin_mesh.md, Gamebryo 2.6 NiSkinInstance::Update
fn test_skin(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use fo3_gamebryo_core::NiTransform;
    use fo3_render::{collect_bone_world_transforms, resolve_bone_world_transforms};
    use fo3_render::skinning::apply_skinning_cpu_with_bones;
    use glam::Vec3;

    println!("=== スキンメッシュ T-Pose 変形検証: {} ===", relative_path);
    let mut vfs = create_vfs(data_dir)?;
    let bytes = vfs.read(relative_path)?;
    let mut cursor = Cursor::new(bytes);
    let nif = NifFile::read(&mut cursor)?;

    // ボーン階層プレパス (バインドポーズのワールド変換を事前登録)
    let mut bone_world_map = HashMap::new();
    collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);
    println!("ボーン (NiNode/BSFadeNode) 登録数: {}", bone_world_map.len());

    // ブロック9 (Pelvis) の親チェーンとローカル変換
    let mut cur = 9i32;
    println!("=== ブロック9 (Pelvis) の親チェーン ===");
    while cur >= 0 && (cur as usize) < nif.blocks.len() {
        let (name, trans, rot) = match &nif.blocks[cur as usize] {
            NifBlock::NiNode(n) => (nif.get_string(n.av.net.name_index).unwrap_or(""), n.av.translation, n.av.rotation.m),
            NifBlock::BSFadeNode(f) => (nif.get_string(f.node.av.net.name_index).unwrap_or(""), f.node.av.translation, f.node.av.rotation.m),
            _ => break,
        };
        println!("  block {}: \"{}\" trans=({:.3},{:.3},{:.3}) rot={:?}", cur, name, trans.x, trans.y, trans.z, rot);
        // 親を探す
        let parent = nif.blocks.iter().position(|b| match b {
            NifBlock::NiNode(n) => n.children.contains(&cur),
            NifBlock::BSFadeNode(f) => f.node.children.contains(&cur),
            _ => false,
        });
        match parent {
            Some(p) => cur = p as i32,
            None => break,
        }
    }

    let mut shape_count = 0;
    let mut skinned_count = 0;
    let mut failed_count = 0;

    for (i, block) in nif.blocks.iter().enumerate() {
        let NifBlock::NiTriShape(shape) = block else { continue };
        shape_count += 1;
        if shape.geom.skin_instance < 0 {
            println!("  [{:03}] NiTriShape (スキンなし): {}", i, nif.get_string(shape.geom.av.net.name_index).unwrap_or(""));
            continue;
        }
        let inst_idx = shape.geom.skin_instance as usize;
        if inst_idx >= nif.blocks.len() {
            failed_count += 1;
            continue;
        }
        let inst = match &nif.blocks[inst_idx] {
            NifBlock::NiSkinInstance(x) => x,
            NifBlock::BSDismemberSkinInstance(x) => &x.skin_instance,
            _ => { failed_count += 1; continue; }
        };
        let data_idx = shape.geom.data as usize;
        let data = match nif.blocks.get(data_idx) {
            Some(NifBlock::NiTriShapeData(d)) => d,
            _ => { failed_count += 1; continue; }
        };

        println!("shape i={} inst_idx={} inst.data={}", i, inst_idx, inst.data);
        let input_min = data.common.vertices.iter().fold(Vec3::splat(f32::INFINITY), |a, v| a.min(Vec3::new(v.x, v.y, v.z)));
        let input_max = data.common.vertices.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, v| a.max(Vec3::new(v.x, v.y, v.z)));

        let bone_mats = resolve_bone_world_transforms(inst, &bone_world_map);
        let bone_refs = if bone_mats.is_empty() { None } else { Some(bone_mats.as_slice()) };
        match apply_skinning_cpu_with_bones(data, inst, &nif, bone_refs) {
            Some((pos, nrm)) => {
                let finite = pos.iter().chain(nrm.iter()).all(|v| v.iter().all(|c| c.is_finite()));
                let out_min = pos.iter().fold(Vec3::splat(f32::INFINITY), |a, v| a.min(Vec3::from_slice(v)));
                let out_max = pos.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, v| a.max(Vec3::from_slice(v)));
                let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
                println!(
                    "  [{:03}] \"{}\" 頂点{} ボーン{}本 finite={} BBox 入力 {:?}~{:?} → 出力 {:?}~{:?}",
                    i, name, pos.len(), inst.bones.len(), finite, input_min, input_max, out_min, out_max
                );
                if finite {
                    skinned_count += 1;
                } else {
                    failed_count += 1;
                    println!("        !! 出力頂点に非有限値が含まれています");
                }
            }
            None => {
                failed_count += 1;
                println!("  [{:03}] スキニング失敗 (NiSkinData 未解決): {}", i, nif.get_string(shape.geom.av.net.name_index).unwrap_or(""));
            }
        }
    }

    println!("\n【結果】NiTriShape: {} 個, スキン正常: {} 個, 失敗/スキップ: {} 個, 登録ボーン: {} 本",
        shape_count, skinned_count, failed_count, bone_world_map.len());
    if skinned_count == 0 {
        println!("警告: スキンメッシュが 1 件も正常変形していません。");
    }
    Ok(())
}

/// 一時プローブ: 全合成式×行列変換の組合せでバインドポーズ一致度を測定する。
fn skin_matrix_probe(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use fo3_gamebryo_core::NiTransform;
    use fo3_nif::types::{Matrix33, Vector3};
    use fo3_render::{collect_bone_world_transforms, resolve_bone_world_transforms};
    use glam::{Mat3, Mat4, Vec3, Vec4};

    fn build_mat(translation: Vector3, rotation: Matrix33, scale: f32, transpose: bool) -> Mat4 {
        let r = &rotation.m;
        let rot = if transpose {
            Mat4::from_mat3(Mat3::from_cols_array_2d(r) * scale)
        } else {
            Mat4::from_mat3(Mat3::from_cols_array(&[
                r[0][0], r[1][0], r[2][0],
                r[0][1], r[1][1], r[2][1],
                r[0][2], r[1][2], r[2][2],
            ]) * scale)
        };
        Mat4::from_translation(Vec3::new(translation.x, translation.y, translation.z)) * rot
    }

    let mut vfs = create_vfs(data_dir)?;
    let bytes = vfs.read(relative_path)?;
    let mut cursor = Cursor::new(bytes);
    let nif = NifFile::read(&mut cursor)?;

    let mut bone_world_map = HashMap::new();
    collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);

    let formulas: Vec<(&str, fn(Mat4, Mat4, Mat4) -> Mat4)> = vec![
        ("I*B*R", |i, b, r| i * b * r),
        ("B*I*R", |i, b, r| b * i * r),
        ("I*B", |i, b, _r| i * b),
        ("B*R*I", |i, b, r| b * r * i),
        ("R*I*B", |i, b, r| r * i * b),
        ("R*B*I", |i, b, r| r * b * i),
        ("B*I", |i, b, _r| b * i),
        ("I*R*B", |i, b, r| i * r * b),
    ];
    let convs: Vec<(&str, bool)> = vec![("nontrans", false), ("trans", true)];

    // 形状ごとの測定: combo -> max dev
    let mut shapes: Vec<(String, Vec<(usize, Vec<(String, String, f32)>)>)> = Vec::new();

    for (i, block) in nif.blocks.iter().enumerate() {
        let NifBlock::NiTriShape(shape) = block else { continue };
        if shape.geom.skin_instance < 0 { continue; }
        let inst_idx = shape.geom.skin_instance as usize;
        if inst_idx >= nif.blocks.len() { continue; }
        let inst = match &nif.blocks[inst_idx] {
            NifBlock::NiSkinInstance(x) => x,
            NifBlock::BSDismemberSkinInstance(x) => &x.skin_instance,
            _ => continue,
        };
        let data_idx = shape.geom.data as usize;
        let data = match nif.blocks.get(data_idx) {
            Some(NifBlock::NiTriShapeData(d)) => d,
            _ => continue,
        };
        let skin_data = match &nif.blocks[inst.data as usize] {
            NifBlock::NiSkinData(d) => d,
            _ => continue,
        };
        let part_data = match &nif.blocks[inst.skin_partition as usize] {
            NifBlock::NiSkinPartition(p) => p,
            _ => continue,
        };
        let bone_worlds = resolve_bone_world_transforms(inst, &bone_world_map);

        let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("?").to_string();
        let n_verts = data.common.vertices.len();
        let src: Vec<Vec4> = data.common.vertices.iter().map(|v| Vec4::new(v.x, v.y, v.z, 1.0)).collect();

        // 最初の形状 or 単ボーン形状の構造を印字
        let is_single_bone = part_data.partitions.iter().all(|p| p.bones.len() <= 1) && !part_data.partitions.is_empty();
        if shapes.is_empty() || is_single_bone {
            println!("--- 最初の形状 \"{}\" の構造 ---", name);
            println!("  NiSkinData#{} root: t=({:.4},{:.4},{:.4}) s={}", inst.data,
                skin_data.skin_transform_translation.x, skin_data.skin_transform_translation.y, skin_data.skin_transform_translation.z,
                skin_data.skin_transform_scale);
            println!("  root rot m = {:?}", skin_data.skin_transform_rotation.m);
            for (pi, partition) in part_data.partitions.iter().enumerate() {
                println!("  partition[{}]: num_bones={} num_verts={} n_weights={}", pi, partition.bones.len(),
                    partition.vertex_map.len(), partition.num_weights_per_vertex);
                for (bi, &b_in_list) in partition.bones.iter().enumerate() {
let bd = if (b_in_list as usize) < skin_data.bone_list.len() { Some(&skin_data.bone_list[b_in_list as usize]) } else { None };
                        let bw = if (b_in_list as usize) < bone_worlds.len() { bone_worlds[b_in_list as usize] } else { continue };
                        let btrans = if let Some(bd) = bd { format!("({:.4},{:.4},{:.4})", bd.skin_transform_translation.x, bd.skin_transform_translation.y, bd.skin_transform_translation.z) } else { "??".into() };
                        let bfix = if let Some(bd) = bd { format!("rot rows: {:?}", bd.skin_transform_rotation.m.iter().map(|r| format!("({:.3},{:.3},{:.3})", r[0], r[1], r[2])).collect::<Vec<_>>().join(" ")) } else { "rot: ?".into() };
                        println!("    bone[{}] in_list={}  invBind t={}  {}  boneWorld t=({:.4},{:.4},{:.4})",
                            bi, b_in_list, btrans, bfix, bw.transform_point3(Vec3::ZERO).x, bw.transform_point3(Vec3::ZERO).y, bw.transform_point3(Vec3::ZERO).z);
                    if let Some(bd) = bd {
                        let nel = |m: Mat4| format!("t=({:.2},{:.2},{:.2}) rot=({:.3},{:.3},{:.3})", m.transform_point3(Vec3::ZERO).x, m.transform_point3(Vec3::ZERO).y, m.transform_point3(Vec3::ZERO).z, m.to_cols_array()[0], m.to_cols_array()[4], m.to_cols_array()[8]);
                        let mi_t = build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale, true);
                        let mi_n = build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale, false);
                        println!("        Mb*Mi(trans) = {}  |  Mi*Mb(trans) = {}", nel(bw * mi_t), nel(mi_t * bw));
                        println!("        Mb*Mi(nontr) = {}  |  Mi*Mb(nontr) = {}", nel(bw * mi_n), nel(mi_n * bw));
                    }
                }
                if let Some(first_w) = partition.vertex_weights.first() {
                    println!("    vertex[0]: weights={:?} bone_idx={:?} → geo={}", first_w, partition.bone_indices[0], partition.vertex_map[0]);
                }
            }
        }

        let mut combo_max: Vec<(String, String, f32)> = Vec::new();
        for (fname, f) in &formulas {
            for (cname, transpose) in &convs {
                let root = build_mat(skin_data.skin_transform_translation, skin_data.skin_transform_rotation, skin_data.skin_transform_scale, *transpose);
                let mut worst = 0.0f32;
                for partition in &part_data.partitions {
                    if partition.vertex_map.is_empty() || partition.vertex_weights.is_empty() { continue; }
                    let mut bm: Vec<Mat4> = Vec::with_capacity(partition.bones.len());
                    for &bone_in_list in &partition.bones {
                        let bone_in_list = bone_in_list as usize;
                        if bone_in_list < skin_data.bone_list.len() && bone_in_list < bone_worlds.len() {
                            let bd = &skin_data.bone_list[bone_in_list];
                            let ib = build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale, *transpose);
                            let bw = bone_worlds[bone_in_list];
                            bm.push(f(ib, bw, root));
                        } else {
                            bm.push(Mat4::IDENTITY);
                        }
                    }
                    let n_inf = (partition.num_weights_per_vertex as usize).min(4);
                    for vi in 0..partition.vertex_map.len() {
                        let geo_idx = partition.vertex_map[vi] as usize;
                        if geo_idx >= n_verts { continue; }
                        let mut acc = Vec4::ZERO;
                        for k in 0..n_inf {
                            let w = partition.vertex_weights[vi][k];
                            if w < 1e-6 { continue; }
                            let bi = partition.bone_indices[vi][k] as usize;
                            if bi >= bm.len() { continue; }
                            acc += w * (bm[bi] * src[geo_idx]);
                        }
                        if !acc.is_finite() { continue; }
                        let dev = (acc.truncate() - src[geo_idx].truncate()).length();
                        if dev > worst { worst = dev; }
                    }
                }
                let best = worst;
                combo_max.push((fname.to_string(), cname.to_string(), best));
            }
        }
        // 良い組合せ順 (min dev asc)
        combo_max.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        shapes.push((name, vec![(i, combo_max)]));
    }

    // 報告: 各形状の best 3 と、形状横断で最も小さな max を得る combo
    println!("形状ごとの best (maxdev)：");
    for (name, v) in &shapes {
        let (_, combos) = &v[0];
        let best3: Vec<String> = combos.iter().take(4)
            .map(|(f, c, d)| format!("{} {} = {:.4}", f, c, d))
            .collect();
        println!("  \"{}\": {}", name, best3.join(" | "));
    }
    // ボード全体で全形状 maxdev<0.5 を満たす combo を探す
    let mut winners: Vec<(String, String)> = Vec::new();
    let combo_keys: Vec<(String, String)> = {
        let mut s = std::collections::BTreeSet::new();
        for (_, vv) in &shapes { for (_, combos) in vv { for (f, c, _) in combos { s.insert((f.clone(), c.clone())); } } }
        s.into_iter().collect()
    };
    for (f, c) in &combo_keys {
        let all_ok = shapes.iter().all(|(_, vv)| {
            vv[0].1.iter().any(|(f2, c2, d)| f2 == f && c2 == c && *d < 0.5)
        });
        if all_ok { winners.push((f.clone(), c.clone())); }
    }
    println!("\n全形状で maxdev<0.5 を満たす組合せ: {:?}", winners);

    // ——— OpenMW 流 (NiSkinData.bone_list の BoneData weights を使用) ———
    println!("\n=== NiSkinData.bone_list の BoneVertData 重みで OpenMW 式 (I*B*R trans) を検証 ===");
    for (_i, block) in nif.blocks.iter().enumerate() {
        let NifBlock::NiTriShape(shape) = block else { continue };
        if shape.geom.skin_instance < 0 { continue; }
        let inst = match &nif.blocks[shape.geom.skin_instance as usize] {
            NifBlock::NiSkinInstance(x) => x,
            NifBlock::BSDismemberSkinInstance(x) => &x.skin_instance,
            _ => continue,
        };
        let data = match nif.blocks.get(shape.geom.data as usize) {
            Some(NifBlock::NiTriShapeData(d)) => d,
            _ => continue,
        };
        let skin_data = match &nif.blocks[inst.data as usize] {
            NifBlock::NiSkinData(d) => d,
            _ => continue,
        };
        let bone_worlds = resolve_bone_world_transforms(inst, &bone_world_map);
        let root = build_mat(skin_data.skin_transform_translation, skin_data.skin_transform_rotation, skin_data.skin_transform_scale, true);
        let n_verts = data.common.vertices.len();
        let src: Vec<Vec4> = data.common.vertices.iter().map(|v| Vec4::new(v.x, v.y, v.z, 1.0)).collect();
        let mut acc: Vec<Vec4> = vec![Vec4::ZERO; n_verts];
        let mut n_inf: Vec<f32> = vec![0.0; n_verts];
        for (bi, bd) in skin_data.bone_list.iter().enumerate() {
            let bw = if bi < bone_worlds.len() { bone_worlds[bi] } else { continue };
            let ib = build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale, true);
            let mat = ib * bw * root;
            for bv in &bd.vertex_weights {
                let vidx = bv.index as usize;
                if vidx < n_verts {
                    acc[vidx] += bv.weight * (mat * src[vidx]);
                    n_inf[vidx] += bv.weight;
                }
            }
        }
        let mut worst = 0.0f32;
        let mut used = 0usize;
        for vidx in 0..n_verts {
            if n_inf[vidx] < 1e-6 { continue; }
            used += 1;
            // 重み正規化
            let out = acc[vidx] / n_inf[vidx];
            let dev = (out.truncate() - src[vidx].truncate()).length();
            if dev > worst { worst = dev; }
        }
        let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("?").to_string();
        // 形状ノード自身のローカル変換と親チェーンにおける世界変換
        let (tnx, tny, tnz) = (shape.geom.av.translation.x, shape.geom.av.translation.y, shape.geom.av.translation.z);
        let (scx, _scz) = (shape.geom.av.scale, 0.0f32);
        println!("  \"{}\" blob={} verts={} used={} worst_dev={:.4}  | shape_node t=({:.3},{:.3},{:.3}) scale={} rot0={:.3}", name, skin_data.bone_list.len(), n_verts, used, worst, tnx, tny, tnz, scx, shape.geom.av.rotation.m[0][0]);
        // 各形状の bone[0] の invBind と boneWorld を対比 + meathead のパーティション経路を再検証
        if let (Some(bd), Some(bw)) = (skin_data.bone_list.first(), bone_worlds.first().copied()) {
            let ib = build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale, true);
            let ip = ib.inverse();
            println!("      bone0: inst_bone={}  invBind t=({:.3},{:.3},{:.3})  ->  boneWorld t=({:.3},{:.3},{:.3})  invBind^-1 t=({:.3},{:.3},{:.3})",
                inst.bones.first().map(|b| *b).unwrap_or(-1),
                bd.skin_transform_translation.x, bd.skin_transform_translation.y, bd.skin_transform_translation.z,
                bw.transform_point3(Vec3::ZERO).x, bw.transform_point3(Vec3::ZERO).y, bw.transform_point3(Vec3::ZERO).z,
                ip.transform_point3(Vec3::ZERO).x, ip.transform_point3(Vec3::ZERO).y, ip.transform_point3(Vec3::ZERO).z);
            println!("        invBind rot rows: {:?}", bd.skin_transform_rotation.m.iter().map(|r| format!("({:.3},{:.3},{:.3})", r[0], r[1], r[2])).collect::<Vec<_>>().join(" "));
            println!("        boneWorld rot cols: {:?}", (0..3).map(|cc| { let c = bw.to_cols_array(); format!("({:.3},{:.3},{:.3})", c[cc*4], c[cc*4+1], c[cc*4+2]) }).collect::<Vec<_>>().join(" "));
            println!("      root: t=({:.3},{:.3},{:.3}) rot rows: {:?}",
                skin_data.skin_transform_translation.x, skin_data.skin_transform_translation.y, skin_data.skin_transform_translation.z,
                skin_data.skin_transform_rotation.m.iter().map(|r| format!("({:.3},{:.3},{:.3})", r[0], r[1], r[2])).collect::<Vec<_>>().join(" "));
            // パーティション経路（combo probe と同じ実装）で meathead/meatneck の正確な dev を出す
            if let NifBlock::NiSkinPartition(part_data) = &nif.blocks[inst.skin_partition as usize] {
                for (pi, partition) in part_data.partitions.iter().enumerate() {
                    if partition.vertex_map.is_empty() || partition.vertex_weights.is_empty() { continue; }
                    let mut bm: Vec<Mat4> = Vec::with_capacity(partition.bones.len());
                    for &b_in_list in &partition.bones {
                        let b_in_list = b_in_list as usize;
                        if b_in_list < skin_data.bone_list.len() && b_in_list < bone_worlds.len() {
                            let bdd = &skin_data.bone_list[b_in_list];
                            let iib = build_mat(bdd.skin_transform_translation, bdd.skin_transform_rotation, bdd.skin_transform_scale, true);
                            bm.push(iib * bone_worlds[b_in_list] * root);
                        } else { bm.push(Mat4::IDENTITY); }
                    }
                    let n_inf = (partition.num_weights_per_vertex as usize).min(4);
                    let mut worst_p = 0.0f32;
                    for vi in 0..partition.vertex_map.len() {
                        let geo_idx = partition.vertex_map[vi] as usize;
                        if geo_idx >= n_verts { continue; }
                        let mut acc = Vec4::ZERO;
                        for k in 0..n_inf {
                            let w = partition.vertex_weights[vi][k];
                            if w < 1e-6 { continue; }
                            let bi = partition.bone_indices[vi][k] as usize;
                            if bi >= bm.len() { continue; }
                            acc += w * (bm[bi] * src[geo_idx]);
                        }
                        if !acc.is_finite() { continue; }
                        let dev = (acc.truncate() - src[geo_idx].truncate()).length();
                        if dev > worst_p { worst_p = dev; }
                    }
                    println!("      partition[{}]: bones={} worst_dev={:.4} bm[0].t=({:.2},{:.2},{:.2})",
                        pi, partition.bones.len(), worst_p,
                        bm.first().map(|m| m.transform_point3(Vec3::ZERO)).unwrap_or(Vec3::ZERO).x,
                        bm.first().map(|m| m.transform_point3(Vec3::ZERO)).unwrap_or(Vec3::ZERO).y,
                        bm.first().map(|m| m.transform_point3(Vec3::ZERO)).unwrap_or(Vec3::ZERO).z);
                }
            }
        }
    }
    Ok(())
}
/// 仮説検証プローブ:
/// アーマー NIF 内の"代理ボーン"ではなく、実スケルトン (skeleton.nif) のバインド
/// ワールド変換を名前解決で引き、NifSkope 準拠の nodeWorld·Σ(w·(boneWorld·invBind)) が
/// すべての形状でバインドポーズ一致 (dev≈0) になるかを確認する。
fn skeleton_bind_probe(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use fo3_gamebryo_core::NiTransform;
    use fo3_nif::types::{Matrix33, Vector3};
    use fo3_render::collect_bone_world_transforms;
    use glam::{Mat3, Mat4, Vec3, Vec4};

    fn build_mat(translation: Vector3, rotation: Matrix33, scale: f32) -> Mat4 {
        let r = &rotation.m;
        let rot = Mat4::from_mat3(Mat3::from_cols_array_2d(r) * scale);
        Mat4::from_translation(Vec3::new(translation.x, translation.y, translation.z)) * rot
    }
    fn default_ni() -> NiTransform {
        NiTransform { rotation: Mat3::IDENTITY, translation: Vec3::ZERO, scale: 1.0 }
    }

    let mut vfs = create_vfs(data_dir)?;

    // 1) スケルトン NIF を候補パスからロードし、name -> world(trans) を構築
    let candidates = [
        "meshes/characters/_male/skeleton.nif",
        "meshes/characters/male/skeleton.nif",
        "meshes/characters/_female/skeleton.nif",
        "meshes/characters/female/skeleton.nif",
        "meshes/characters/_1stperson/skeleton.nif",
    ];
    let mut skeleton_name_world: HashMap<String, Mat4> = HashMap::new();
    for c in candidates {
        let Ok(bytes) = vfs.read(c) else { continue };
        let mut cur = Cursor::new(bytes);
        if let Ok(skel) = NifFile::read(&mut cur) {
            let mut world_map = HashMap::new();
            collect_bone_world_transforms(0, &default_ni(), &skel, &mut world_map);
            for (idx, block) in skel.blocks.iter().enumerate() {
                let av_opt = match block {
                    NifBlock::NiNode(n) => Some(&n.av),
                    NifBlock::BSFadeNode(n) => Some(&n.node.av),
                    _ => None,
                };
                if let Some(av) = av_opt {
                    if let Some(names) = skel.get_string(av.net.name_index) {
                        if let Some(m) = world_map.get(&(idx as i32)) {
                            skeleton_name_world.insert(names.to_string(), *m);
                        }
                    }
                }
            }
            println!("SKELETON '{}' ロード成功 (name->world {} 件)", c, skeleton_name_world.len());
            break;
        }
    }
    if skeleton_name_world.is_empty() {
        println!("スケルトンがロードできませんでした (候補全滅)");
    }

    // 2) アーマー NIF
    let bytes = vfs.read(relative_path)?;
    let mut cursor = Cursor::new(bytes);
    let nif = NifFile::read(&mut cursor)?;
    let mut armor_world_map = HashMap::new();
    collect_bone_world_transforms(0, &default_ni(), &nif, &mut armor_world_map);

    let name_of = |idx: u32| nif.get_string(idx).map(|s| s.to_string());

    for (i, block) in nif.blocks.iter().enumerate() {
        let NifBlock::NiTriShape(shape) = block else { continue };
        if shape.geom.skin_instance < 0 { continue; }
        let inst = match &nif.blocks[shape.geom.skin_instance as usize] {
            NifBlock::NiSkinInstance(x) => x,
            NifBlock::BSDismemberSkinInstance(x) => &x.skin_instance,
            _ => continue,
        };
        let data = match nif.blocks.get(shape.geom.data as usize) {
            Some(NifBlock::NiTriShapeData(d)) => d,
            _ => continue,
        };
        let skin_data = match &nif.blocks[inst.data as usize] {
            NifBlock::NiSkinData(d) => d,
            _ => continue,
        };
        let part_data = match &nif.blocks[inst.skin_partition as usize] {
            NifBlock::NiSkinPartition(p) => p,
            _ => continue,
        };
        let _shape_name = name_of(shape.geom.av.net.name_index).unwrap_or_default();
        let n_verts = data.common.vertices.len();
        let src: Vec<Vec4> = data.common.vertices.iter().map(|v| Vec4::new(v.x, v.y, v.z, 1.0)).collect();

        // shape のワールド変換 (親チェーンで armor_world_map の世界変換を合成)
        let shape_world = {
            let av = &shape.geom.av;
            let mut acc = Mat4::from_translation(Vec3::new(av.translation.x, av.translation.y, av.translation.z))
                * Mat4::from_mat3(Mat3::from_cols_array_2d(&av.rotation.m) * av.scale);
            let mut cur = i as i32;
            loop {
                let parent = nif.blocks.iter().position(|b| match b {
                    NifBlock::NiNode(n) => n.children.contains(&cur),
                    NifBlock::BSFadeNode(b) => b.node.children.contains(&cur),
                    _ => false,
                });
                match parent {
                    Some(pi) => {
                        if let Some(pw) = armor_world_map.get(&(pi as i32)) {
                            acc = *pw * acc;
                            break;
                        }
                        cur = pi as i32;
                    }
                    None => break,
                }
            }
            acc
        };

        // ボーン名解決: 形状の inst.bones[i] -> block name -> skeleton world (無ければ proxy world)
        let bone_names: Vec<Option<String>> = inst.bones.iter().map(|&b| name_of(b.max(0) as u32)).collect();
        let mut _resolved_names = 0usize;
        let mut bone_worlds: Vec<Mat4> = inst.bones.iter().map(|&b| armor_world_map.get(&b).copied().unwrap_or(Mat4::IDENTITY)).collect();
        for (bi, bname) in bone_names.iter().enumerate() {
            if let Some(name) = bname {
                if let Some(skw) = skeleton_name_world.get(name) {
                    bone_worlds[bi] = *skw;
                    _resolved_names += 1;
                }
            }
        }

        // dev 測定 (NifSkope 準拠: nodeWorld · Σ w·(boneWorld·invBind) · v  vs nodeWorld·v)
        let root = build_mat(skin_data.skin_transform_translation, skin_data.skin_transform_rotation, skin_data.skin_transform_scale);
        let mut worst_a = 0.0f32; // ノード・ルートなし
        let mut worst_b = 0.0f32; // nodeWorld あり (NifSkope 相当)
        let mut worst_c = 0.0f32; // nodeWorld · root あり
        for partition in &part_data.partitions {
            if partition.vertex_map.is_empty() || partition.vertex_weights.is_empty() { continue; }
            let mut bm_skel: Vec<Mat4> = Vec::with_capacity(partition.bones.len());
            for &b_in_list in &partition.bones {
                let bi = b_in_list as usize;
                if bi < skin_data.bone_list.len() {
                    let bd = &skin_data.bone_list[bi];
                    bm_skel.push(build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale) * bone_worlds[bi]);
                } else { bm_skel.push(Mat4::ZERO); }
            }
            let n_inf = (partition.num_weights_per_vertex as usize).min(4);
            for vi in 0..partition.vertex_map.len() {
                let geo_idx = partition.vertex_map[vi] as usize;
                if geo_idx >= n_verts { continue; }
                let mut acc = Vec4::ZERO;
                for k in 0..n_inf {
                    let w = partition.vertex_weights[vi][k];
                    if w < 1e-6 { continue; }
                    let pbi = partition.bone_indices[vi][k] as usize;
                    if pbi >= bm_skel.len() { continue; }
                    acc += w * (bm_skel[pbi] * src[geo_idx]);
                }
                if !acc.is_finite() { continue; }
                let ref_mesh = shape_world * src[geo_idx];
                worst_a = worst_a.max((acc.truncate() - src[geo_idx].truncate()).length());
                worst_b = worst_b.max(((shape_world * acc).truncate() - ref_mesh.truncate()).length());
                worst_c = worst_c.max(((shape_world * root * acc).truncate() - ref_mesh.truncate()).length());
            }
        }

        // 全ボーンの対比 (invBind⁻¹ vs proxy world vs skeleton world)
        for (bi, bd) in skin_data.bone_list.iter().enumerate().take(9) {
            let ib = build_mat(bd.skin_transform_translation, bd.skin_transform_rotation, bd.skin_transform_scale);
            let ip = ib.inverse();
            let proxy = bone_worlds.get(bi).copied().unwrap_or(Mat4::IDENTITY);
            let bname = bone_names.get(bi).and_then(|s| s.clone()).unwrap_or_default();
            let skw = skeleton_name_world.get(&bname).copied().unwrap_or(proxy);
            let d_skel = (ip.transform_point3(Vec3::ZERO) - skw.transform_point3(Vec3::ZERO)).length();
            let d_proxy = (ip.transform_point3(Vec3::ZERO) - proxy.transform_point3(Vec3::ZERO)).length();
            println!(
                "      bone[{}]({}) skel_t={:?} | invBind^-1_t={:?} | d_skel={:.3} proxy_t={:?} d_proxy={:.3}",
                bi, bname,
                skw.transform_point3(Vec3::ZERO),
                ip.transform_point3(Vec3::ZERO),
                d_skel,
                proxy.transform_point3(Vec3::ZERO),
                d_proxy,
            );
        }
    }
    Ok(())
}
fn skin_probe(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use fo3_gamebryo_core::NiTransform;
    use fo3_render::collect_bone_world_transforms;
    use glam::Mat4;
    let mut vfs = create_vfs(data_dir)?;
    let bytes = vfs.read(relative_path)?;
    let mut cursor = Cursor::new(bytes);
    let nif = NifFile::read(&mut cursor)?;
    let mut bone_world_map = HashMap::new();
    collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);

    let name_of = |idx: i32| -> String {
        if let Some(name) = nif.get_string(idx.max(0) as u32) { name.to_string() } else { format!("#{}", idx) }
    };

    for (i, block) in nif.blocks.iter().enumerate() {
        let (inst, label) = match block {
            NifBlock::NiSkinInstance(x) => (x, format!("NiSkinInstance#{}", i)),
            NifBlock::BSDismemberSkinInstance(x) => (&x.skin_instance, format!("BSDismember#{}", i)),
            _ => continue,
        };
        println!("{}: skeleton_root={}({})", label, name_of(inst.skeleton_root), inst.skeleton_root);
        for (bi, &b) in inst.bones.iter().enumerate() {
            let bw = bone_world_map.get(&b).copied().unwrap_or(Mat4::IDENTITY);
            println!("   bone[{}] -> block {} ({})  bone_world_t={:?}", bi, b, name_of(b), bw.transform_point3(glam::Vec3::ZERO));
        }
    }
    Ok(())
}

/// 実アセットの KF アニメーション適用をヘッドレス検証する。
///
/// スキン NIF + KF を読み込み、`AnimationPlayer` で時間を進めながら
/// `apply_pose` → `recompute_bone_world_map_with_pose` を実行し、
/// KF が駆動するボーンのワールド変換が時間経過で実際に変化すること、
/// およびメッシュがスキン変形されることを確認する。
///
/// 参照元: knowledge/animation_kf_format.md (セクション 5, 5.6)
fn test_anim(data_dir: &str, relative_path: &str, kf_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use fo3_gamebryo_core::NiTransform;
    use fo3_render::animation::{AnimationClip, AnimationPlayer, SkeletonPose};
    use fo3_render::{
        collect_bone_world_transforms, recompute_bone_world_map_with_pose,
        resolve_bone_world_transforms,
    };
    use fo3_render::skinning::apply_skinning_cpu_with_bones;
    use glam::{Mat4, Vec3};

    println!("=== KF アニメーション適用検証: {} + {} ===", relative_path, kf_path);
    let mut vfs = create_vfs(data_dir)?;

    let nif_bytes = vfs.read(relative_path)?;
    let mut nif_cursor = Cursor::new(nif_bytes);
    let nif = NifFile::read(&mut nif_cursor)?;

    let kf_bytes = vfs.read(kf_path)?;
    let mut kf_cursor = Cursor::new(kf_bytes);
    let kf = NifFile::read(&mut kf_cursor)?;

    // 配下ボーンノードの名前→ブロックインデックスを収集 (FK で上書きされるか判定用)
    let mut bone_index_by_name: HashMap<String, i32> = HashMap::new();
    for (i, block) in nif.blocks.iter().enumerate() {
        let name = match block {
            NifBlock::NiNode(n) => nif.get_string(n.av.net.name_index).unwrap_or("").to_string(),
            NifBlock::BSFadeNode(f) => nif.get_string(f.node.av.net.name_index).unwrap_or("").to_string(),
            _ => continue,
        };
        if !name.is_empty() {
            bone_index_by_name.insert(name, i as i32);
        }
    }

    let clip = AnimationClip::from_kf(&kf)
        .ok_or("KF から NiControllerSequence を検出できませんでした")?;
    println!(
        "クリップ: \"{}\" ({:.2}s - {:.2}s, duration {:.2}s, cycle={}, チャンネル {} 本)",
        clip.name, clip.start_time, clip.stop_time, clip.duration, clip.cycle_type, clip.channels.len()
    );

    // KF チャンネル名と NIF ボーンの共通集合 (実際に動かせるボーン)
    let drivable: Vec<(&String, i32)> = clip.channels.keys()
        .filter_map(|name| bone_index_by_name.get(name).map(|idx| (name, *idx)))
        .collect();
    println!("NIF 内に存在する駆動対象ボーン: {} 本", drivable.len());
    for (name, _) in drivable.iter().take(10) {
        println!("   - {}", name);
    }
    if drivable.is_empty() {
        return Err("アニメーションを NIF のボーンに適用できません（名前不一致）".into());
    }

    // バインドポーズのボーンワールド変換 (比較基準)
    let mut bind_map = HashMap::new();
    collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bind_map);
    let bind_t = |idx: i32| bind_map.get(&idx).copied().unwrap_or(Mat4::IDENTITY).transform_point3(Vec3::ZERO);

    // アニメーション再生: 30 フレームで 1 ループ
    let mut player = AnimationPlayer::new(clip.clone());
    let mut pose = SkeletonPose::default();
    let steps = 30;
    let dt = clip.duration / steps as f32;

    let mut max_drift = 0.0f32;
    for step in 0..=steps {
        let updated = if step == 0 { Vec::new() } else { player.update(&kf, dt, &mut pose) };
        let mut map = HashMap::new();
        recompute_bone_world_map_with_pose(&nif, &pose, &mut map);
        // 駆動ボーンのうち最初の 3 本のワールド位置をサンプリング
        for (_, idx) in drivable.iter().take(3) {
            let p = map.get(idx).copied().unwrap_or(Mat4::IDENTITY).transform_point3(Vec3::ZERO);
            let bind_p = bind_t(*idx);
            let drift = (p - bind_p).length();
            max_drift = max_drift.max(drift);
        }
        if updated.is_empty() && step > 0 {
            // 途中でボーン更新が止まるのは問題
            println!("  ステップ {}: ボーン更新なし (still {}", step, player.current_time);
        }
    }

    // 最初のスキンメッシュを最終フレームの姿勢で再スキニングし、BBox の移動を確認
    let mut skinned_drift = 0.0f32;
    for block in &nif.blocks {
        let NifBlock::NiTriShape(shape) = block else { continue };
        if shape.geom.skin_instance < 0 { continue; }
        let inst_idx = shape.geom.skin_instance as usize;
        let Some(inst) = (match nif.blocks.get(inst_idx) {
            Some(NifBlock::NiSkinInstance(x)) => Some(x),
            Some(NifBlock::BSDismemberSkinInstance(x)) => Some(&x.skin_instance),
            _ => None,
        }) else { continue };
        let Some(NifBlock::NiTriShapeData(data)) = nif.blocks.get(shape.geom.data as usize) else { continue };

        let mut map = HashMap::new();
        recompute_bone_world_map_with_pose(&nif, &pose, &mut map);
        let bone_mats = resolve_bone_world_transforms(inst, &map);
        let bone_refs = if bone_mats.is_empty() { None } else { Some(bone_mats.as_slice()) };
        let bind_mats = resolve_bone_world_transforms(inst, &bind_map);
        let bind_refs = if bind_mats.is_empty() { None } else { Some(bind_mats.as_slice()) };

        if let (Some((pos, _)), Some((bind_pos, _))) =
            (apply_skinning_cpu_with_bones(data, inst, &nif, bone_refs),
             apply_skinning_cpu_with_bones(data, inst, &nif, bind_refs))
        {
            let center = |p: &[[f32; 3]]| {
                let mut s = Vec3::ZERO;
                for v in p { s += Vec3::from_slice(v); }
                if !p.is_empty() { s / p.len() as f32 } else { s }
            };
            skinned_drift = skinned_drift.max((center(&pos) - center(&bind_pos)).length());
            println!("スキンメッシュ \"{}\": 頂点中心のバインドからの移動 {:.4} units",
                nif.get_string(shape.geom.av.net.name_index).unwrap_or(""), skinned_drift);
            break;
        }
    }

    println!("\n【結果】クリップ duration {:.2}s を {} ステップで再生",
        clip.duration, steps);
    if max_drift > 1e-3 {
        println!("  ボーンワールド変換の最大変化量: {:.4} units (アニメーション適用 OK)", max_drift);
    } else {
        println!("  ボーンワールド変換の最大変化量: {:.4} units (変化が検出されません)", max_drift);
    }
    println!("  スキンメッシュ頂点中心の移動量: {:.4} units", skinned_drift);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "nif" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_nif(&args[2])?;
        }
        "nif-dump" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_nif_dump(&args[2])?;
        }
        "bsa-list" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            let filter = args.get(3).map(|s| s.as_str());
            test_bsa_list(&args[2], filter)?;
        }
        "bsa-extract" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_bsa_extract(&args[2], &args[3])?;
        }
        "vfs-test" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_vfs(&args[2], &args[3])?;
        }
        "vfs-nif-dump" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_vfs_nif_dump(&args[2], &args[3])?;
        }
        "esm-header" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_esm_header(&args[2])?;
        }
        "esm-groups" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_esm_groups(&args[2])?;
        }
        "esm-stat" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            let limit = args.get(3).and_then(|s| s.parse::<usize>().ok());
            test_esm_stat(&args[2], limit)?;
        }
        "esm-ltex" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            let limit = args.get(3).and_then(|s| s.parse::<usize>().ok());
            test_esm_ltex(&args[2], limit)?;
        }
        "esm-cell" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_esm_cell(&args[2], &args[3])?;
        }
        "esm-worlds" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            test_esm_worlds(&args[2])?;
        }
        "esm-world-dump" => {
            if args.len() < 4 {
                println!("使用法: esm-world-dump <path/to/file.esm> <world_edid>");
                return Ok(());
            }
            test_esm_world_dump(&args[2], &args[3])?;
        }
        "collision-batch" => {
            if args.len() < 3 {
                print_usage();
                return Ok(());
            }
            let limit = args.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(100);
            test_collision_batch(&args[2], limit)?;
        }
        "collision-lines" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            test_collision_lines(&args[2], &args[3])?;
        }
        "nif-coverage" => {
            if args.len() < 3 {
                println!("使用法: nif-coverage <data_dir> [limit]");
                return Ok(());
            }
            let limit = args.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1000);
            test_nif_coverage(&args[2], limit)?;
        }
        "skin-test" => {
            if args.len() < 4 {
                println!("使用法: skin-test <data_dir> <relative/nif/path>");
                return Ok(());
            }
            test_skin(&args[2], &args[3])?;
        }
        "anim-test" => {
            if args.len() < 5 {
                println!("使用法: anim-test <data_dir> <relative/nif/path> <kf_path>");
                return Ok(());
            }
            test_anim(&args[2], &args[3], &args[4])?;
        }
        "skeleton-bind-probe" => {
            if args.len() < 4 {
                print_usage();
                return Ok(());
            }
            skeleton_bind_probe(&args[2], &args[3])?;
        }
        "skin-probe" => {
            if args.len() < 4 {
                println!("使用法: skin-probe <data_dir> <relative/nif/path>");
                return Ok(());
            }
            skin_probe(&args[2], &args[3])?;
        }
        "skin-matrix-probe" => {
            if args.len() < 4 {
                println!("使用法: skin-matrix-probe <data_dir> <relative/nif/path>");
                return Ok(());
            }
            skin_matrix_probe(&args[2], &args[3])?;
        }
        _ => {
            if args[1].ends_with(".nif") {
                test_nif(&args[1])?;
            } else {
                print_usage();
            }
        }
    }

    Ok(())
}

fn test_collision_batch(data_dir: &str, limit: usize) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Havok コリジョンブロック一括パース検証 (最大 {} 件) ===", limit);
    let mut vfs = create_vfs(data_dir)?;

    let mesh_bsa_path = Path::new(data_dir).join("Fallout - Meshes.bsa");
    let bsa = BsaArchive::open(&mesh_bsa_path)?;
    let files = bsa.list_files();

    let mut total_meshes = 0;
    let mut meshes_with_collision = 0;
    let mut count_collision_obj = 0;
    let mut count_rigid_body = 0;
    let mut count_mopp = 0;
    let mut count_packed_shape = 0;
    let mut count_packed_data = 0;
    let mut count_compressed_data = 0;
    let mut count_box_shape = 0;
    let mut count_sphere_shape = 0;
    let mut count_capsule_shape = 0;
    let mut count_convex_vertices = 0;
    let mut count_convex_transform = 0;
    let mut count_convex_list = 0;
    let mut count_list_shape = 0;
    let mut count_blend_collision_obj = 0;
    let mut count_sp_collision_obj = 0;
    let mut count_transform_shape = 0;
    let mut count_ni_tri_strips_shape = 0;
    let mut count_simple_shape_phantom = 0;
    let mut count_unknown_bhk = 0;
    let mut unknown_types = std::collections::BTreeMap::<String, usize>::new();
    let mut total_collision_triangles = 0;
    let mut total_collision_vertices = 0;

    for file_path in files {
        if !file_path.ends_with(".nif") {
            continue;
        }
        total_meshes += 1;

        if let Ok(data) = vfs.read(file_path) {
            let mut cursor = Cursor::new(&data);
            if let Ok(nif) = NifFile::read(&mut cursor) {
                let mut has_col = false;
                for block in &nif.blocks {
                    match block {
                        NifBlock::BhkCollisionObject(_) => {
                            has_col = true;
                            count_collision_obj += 1;
                        }
                        NifBlock::BhkRigidBody(_) | NifBlock::BhkRigidBodyT(_) => {
                            count_rigid_body += 1;
                        }
                        NifBlock::BhkMoppBvTreeShape(_) => {
                            count_mopp += 1;
                        }
                        NifBlock::BhkPackedNiTriStripsShape(_) => {
                            count_packed_shape += 1;
                        }
                        NifBlock::HkPackedNiTriStripsData(d) => {
                            count_packed_data += 1;
                            if d.is_compressed {
                                count_compressed_data += 1;
                            }
                            total_collision_triangles += d.triangles.len();
                            total_collision_vertices += d.vertices.len();
                        }
                        NifBlock::BhkBoxShape(_) => {
                            count_box_shape += 1;
                        }
                        NifBlock::BhkSphereShape(_) => {
                            count_sphere_shape += 1;
                        }
                        NifBlock::BhkCapsuleShape(_) => {
                            count_capsule_shape += 1;
                        }
                        NifBlock::BhkConvexVerticesShape(s) => {
                            count_convex_vertices += 1;
                            total_collision_vertices += s.vertices.len();
                        }
                        NifBlock::BhkConvexTransformShape(_) => {
                            count_convex_transform += 1;
                        }
                        NifBlock::BhkConvexListShape(_) => {
                            count_convex_list += 1;
                        }
                        NifBlock::BhkListShape(_) => {
                            count_list_shape += 1;
                        }
                        NifBlock::BhkBlendCollisionObject(_) => {
                            has_col = true;
                            count_blend_collision_obj += 1;
                        }
                        NifBlock::BhkSPCollisionObject(_) => {
                            has_col = true;
                            count_sp_collision_obj += 1;
                        }
                        NifBlock::BhkTransformShape(_) => {
                            count_transform_shape += 1;
                        }
                        NifBlock::BhkNiTriStripsShape(_) => {
                            count_ni_tri_strips_shape += 1;
                        }
                        NifBlock::BhkSimpleShapePhantom(_) => {
                            count_simple_shape_phantom += 1;
                        }
                        NifBlock::Unknown { type_name, .. } => {
                            if type_name.starts_with("bhk") || type_name.starts_with("hk") {
                                count_unknown_bhk += 1;
                                *unknown_types.entry(type_name.clone()).or_insert(0) += 1;
                            }
                        }
                        _ => {}
                    }
                }
                if has_col {
                    meshes_with_collision += 1;
                }
            }
        }

        if total_meshes % 1000 == 0 {
            println!("進捗: {} / {} メッシュ処理中 (コリジョン含有: {})...", total_meshes, limit, meshes_with_collision);
        }

        if total_meshes >= limit {
            break;
        }
    }

    println!("検証メッシュ総数: {}", total_meshes);
    println!("コリジョン含有メッシュ数: {} ({:.1}%)", meshes_with_collision, (meshes_with_collision as f64 / total_meshes as f64) * 100.0);
    println!("パース成功ブロック内訳:");
    println!("  bhkCollisionObject:          {} 件", count_collision_obj);
    println!("  bhkSPCollisionObject:        {} 件", count_sp_collision_obj);
    println!("  bhkBlendCollisionObject:     {} 件", count_blend_collision_obj);
    println!("  bhkRigidBody / T:            {} 件", count_rigid_body);
    println!("  bhkMoppBvTreeShape:          {} 件", count_mopp);
    println!("  bhkPackedNiTriStripsShape:   {} 件", count_packed_shape);
    println!("  bhkNiTriStripsShape:         {} 件", count_ni_tri_strips_shape);
    println!("  hkPackedNiTriStripsData:     {} 件 (うち圧縮半精度: {} 件)", count_packed_data, count_compressed_data);
    println!("  bhkBoxShape:                 {} 件", count_box_shape);
    println!("  bhkSphereShape:              {} 件", count_sphere_shape);
    println!("  bhkCapsuleShape:             {} 件", count_capsule_shape);
    println!("  bhkConvexVerticesShape:      {} 件", count_convex_vertices);
    println!("  bhkConvexTransformShape:     {} 件", count_convex_transform);
    println!("  bhkTransformShape:           {} 件", count_transform_shape);
    println!("  bhkConvexListShape:          {} 件", count_convex_list);
    println!("  bhkListShape:                {} 件", count_list_shape);
    println!("  bhkSimpleShapePhantom:       {} 件", count_simple_shape_phantom);
    println!("抽出コリジョン総ポリゴン数:");
    println!("  総三角形数:                 {} 枚", total_collision_triangles);
    println!("  総頂点数:                   {} 点", total_collision_vertices);
    println!("未対応/未パース Havok ブロック: {} 件", count_unknown_bhk);
    for (tname, count) in &unknown_types {
        println!("    - {}: {} 件", tname, count);
    }
    println!("=== コリジョン一括検証完了 ===");
    Ok(())
}

/// 指定 NIF から Havok コリジョンワイヤーフレームラインを抽出し検証する。
fn test_collision_lines(data_dir: &str, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== コリジョンライン抽出検証: {} ===", relative_path);
    let mut vfs = create_vfs(data_dir)?;

    let bytes = vfs.read(relative_path)?;
    println!("ファイルサイズ: {} バイト", bytes.len());

    let mut cursor = Cursor::new(bytes);
    let nif = NifFile::read(&mut cursor)?;
    println!("ブロック総数: {}", nif.blocks.len());

    let col_data = fo3_nif::extract_collision_data(&nif);
    println!("抽出剛体数 (RigidBody): {} 体", col_data.bodies.len());
    for (b_i, body) in col_data.bodies.iter().enumerate() {
        println!(
            "  [Body {:02}] 質量: {:.1}kg | 摩擦: {:.2} | 反発: {:.2} | レイヤー: {:?} | 位置: [{:.1}, {:.1}, {:.1}]",
            b_i, body.mass, body.friction, body.restitution, body.layer,
            body.translation[0], body.translation[1], body.translation[2],
        );
        match &body.shape {
            fo3_nif::CollisionShape::TriMesh { vertices, indices, material } => {
                println!("    -> TriMesh: 頂点数 {}, 三角形数 {}, 材質: {:?}", vertices.len(), indices.len(), material);
            }
            fo3_nif::CollisionShape::Box { half_extents, center, material } => {
                println!("    -> Box: 半径 [{:.1}, {:.1}, {:.1}], 中心 [{:.1}, {:.1}, {:.1}], 材質: {:?}", half_extents[0], half_extents[1], half_extents[2], center[0], center[1], center[2], material);
            }
            fo3_nif::CollisionShape::Sphere { center, radius, material } => {
                println!("    -> Sphere: 半径 {:.1}, 中心 [{:.1}, {:.1}, {:.1}], 材質: {:?}", radius, center[0], center[1], center[2], material);
            }
            fo3_nif::CollisionShape::Capsule { p1, p2, radius, material } => {
                println!("    -> Capsule: 半径 {:.1}, Pt1 [{:.1}, {:.1}, {:.1}], Pt2 [{:.1}, {:.1}, {:.1}], 材質: {:?}", radius, p1[0], p1[1], p1[2], p2[0], p2[1], p2[2], material);
            }
            fo3_nif::CollisionShape::ConvexHull { vertices, material } => {
                println!("    -> ConvexHull: 頂点数 {}, 材質: {:?}", vertices.len(), material);
            }
            fo3_nif::CollisionShape::Compound(children) => {
                println!("    -> Compound: 子形状数 {}", children.len());
            }
        }
    }

    let lines = fo3_render::extract_collision_lines(&nif);
    println!("抽出コリジョンライン頂点数: {} 点 ({} 本の線分)", lines.len(), lines.len() / 2);

    if !lines.is_empty() {
        let (min, max) = lines.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), v| {
                let p = Vec3::new(v.position[0], v.position[1], v.position[2]);
                (min.min(p), max.max(p))
            },
        );
        let size = max - min;
        println!("コリジョン AABB Min: [{:.3}, {:.3}, {:.3}]", min.x, min.y, min.z);
        println!("コリジョン AABB Max: [{:.3}, {:.3}, {:.3}]", max.x, max.y, max.z);
        println!("コリジョン AABB Size: [{:.3}, {:.3}, {:.3}]", size.x, size.y, size.z);
    } else {
        println!("警告: コリジョンラインが 0 件でした。コリジョンブロックが存在しないか未対応です。");
    }

    Ok(())
}

/// NIF ブロック型のパースカバレッジ（網羅率）を一括調査する。
fn test_nif_coverage(data_dir: &str, limit: usize) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== NIF ブロック型パース網羅率（カバレッジ）検証 (最大 {} ファイル) ===", limit);
    let mut vfs = create_vfs(data_dir)?;

    let mesh_bsa_path = Path::new(data_dir).join("Fallout - Meshes.bsa");
    let bsa = BsaArchive::open(&mesh_bsa_path)?;
    let files = bsa.list_files();

    let mut scanned_files = 0;
    let mut parse_failed_files = 0;
    let mut total_blocks = 0;
    let mut parsed_blocks = 0;
    let mut unknown_blocks = 0;

    let mut parsed_type_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut unknown_type_counts = std::collections::BTreeMap::<String, usize>::new();

    for file_path in files {
        if !file_path.ends_with(".nif") {
            continue;
        }

        scanned_files += 1;
        if let Ok(bytes) = vfs.read(file_path) {
            let mut cursor = Cursor::new(bytes);
            match NifFile::read(&mut cursor) {
                Ok(nif) => {
                    for (i, block) in nif.blocks.iter().enumerate() {
                        total_blocks += 1;
                        let type_name = nif.header.block_types[nif.header.block_type_indices[i] as usize].clone();
                        match block {
                            NifBlock::Unknown { .. } => {
                                unknown_blocks += 1;
                                *unknown_type_counts.entry(type_name).or_insert(0) += 1;
                            }
                            _ => {
                                parsed_blocks += 1;
                                *parsed_type_counts.entry(type_name).or_insert(0) += 1;
                            }
                        }
                    }
                }
                Err(_) => {
                    parse_failed_files += 1;
                }
            }
        }

        if scanned_files >= limit {
            break;
        }
    }

    println!("\n【走査結果概要】");
    println!("  走査 NIF ファイル数: {} 件 (パースエラー: {} 件)", scanned_files, parse_failed_files);
    println!("  走査ブロック総数:    {} 個", total_blocks);
    println!("  パース成功ブロック:  {} 個 ({:.2}%)", parsed_blocks, (parsed_blocks as f64 / total_blocks as f64) * 100.0);
    println!("  未パース (Unknown):  {} 個 ({:.2}%)", unknown_blocks, (unknown_blocks as f64 / total_blocks as f64) * 100.0);

    println!("\n【パース成功ブロック型一覧 (上位 20 種)】");
    let mut sorted_parsed: Vec<_> = parsed_type_counts.into_iter().collect();
    sorted_parsed.sort_by(|a, b| b.1.cmp(&a.1));
    for (t, c) in sorted_parsed.iter().take(20) {
        println!("  - {:<30}: {:>6} 個", t, c);
    }

    println!("\n【未対応 (Unknown) ブロック型一覧 (頻度順)】");
    let mut sorted_unknown: Vec<_> = unknown_type_counts.into_iter().collect();
    sorted_unknown.sort_by(|a, b| b.1.cmp(&a.1));
    for (t, c) in &sorted_unknown {
        println!("  - {:<30}: {:>6} 個", t, c);
    }

    Ok(())
}

