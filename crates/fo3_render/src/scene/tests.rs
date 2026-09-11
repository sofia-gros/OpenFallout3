    use super::*;

    /// `resolve_bone_world_transforms` が `NiSkinInstance.bones` のブロックインデックスから
    /// 各ボーンのワールド変換行列を正しく解決できることを検証する。
    #[test]
    fn test_resolve_bone_world_transforms() {
        // block_index 0: ルートボーン "Bip01" → 並進 (1, 2, 3)
        // block_index 1: 子ボーン "Bip01 Pelvis" → 並進 (4, 5, 6)
        let mut bone_world_map = HashMap::new();
        bone_world_map.insert(
            0i32,
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        );
        bone_world_map.insert(
            1i32,
            Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0)),
        );

        let skin_instance = fo3_nif::NiSkinInstance {
            data: -1,
            skin_partition: -1,
            skeleton_root: 0,
            bones: vec![0, 1],
        };

        let transforms = resolve_bone_world_transforms(&skin_instance, &bone_world_map);
        assert_eq!(transforms.len(), 2);
        // ボーン 0: 並進 (1, 2, 3) が保持されている
        assert_eq!(
            transforms[0].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 2.0, 3.0)
        );
        // ボーン 1: 並進 (4, 5, 6) が保持されている
        assert_eq!(
            transforms[1].transform_point3(Vec3::ZERO),
            Vec3::new(4.0, 5.0, 6.0)
        );
    }

    /// マップに存在しないブロックインデックスは `Mat4::IDENTITY` で補完されることを検証する。
    #[test]
    fn test_resolve_bone_missing_identity() {
        let mut bone_world_map = HashMap::new();
        bone_world_map.insert(10i32, Mat4::from_scale(Vec3::splat(2.0)));

        let skin_instance = fo3_nif::NiSkinInstance {
            data: -1,
            skin_partition: -1,
            skeleton_root: 10,
            bones: vec![10, 999], // 999 はマップに存在しない
        };

        let transforms = resolve_bone_world_transforms(&skin_instance, &bone_world_map);
        assert_eq!(transforms.len(), 2);
        assert_eq!(transforms[1], Mat4::IDENTITY);
    }

    /// プレパス `collect_bone_world_transforms` が、NiTriShape (メッシュ) ブロックが
    /// ボーン NiNode より先の子ノード配列順で現れる NIF でも全ボーンのワールド変換を
    /// 解決できることを検証する。
    ///
    /// 装備 NIF (`meshes\armor\...\outfit*.nif`) では NiTriShape が NiNode (ボーン) より
    /// ブロック先頭側に配置されるため、メッシュビルドより先に全ボーンを登録しておく必要がある。
    #[test]
    fn test_collect_bone_world_transforms_mesh_first_order() {
        use fo3_nif::blocks::{NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: 0,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        // ルート[0] の子ノード配列: 並び順は [メッシュ1, ボーン2]
        let root = NiNode {
            av: make_av(Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
            children: vec![1, 2],
            effects: vec![],
        };
        // ボーン[2] "Bip01" はローカル並進 (10, 20, 30)
        let bone = NiNode {
            av: make_av(Vector3 { x: 10.0, y: 20.0, z: 30.0 }),
            children: vec![],
            effects: vec![],
        };

        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![],
            },
            blocks: vec![
                NifBlock::NiNode(root),
                // スキンメッシュを模した未対応ブロック (メッシュが先)
                NifBlock::Unknown {
                    type_name: "NiTriShape".to_string(),
                    data: vec![],
                },
                NifBlock::NiNode(bone),
            ],
        };

        let mut bone_world_map = HashMap::new();
        collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);

        // ルート[0] とボーン[2] の両方がワールド行列として登録されている
        assert!(bone_world_map.contains_key(&0));
        assert!(bone_world_map.contains_key(&2));
        // ボーン[2] のワールド並進が (10, 20, 30) として保持されている
        assert_eq!(
            bone_world_map[&2].transform_point3(Vec3::ZERO),
            Vec3::new(10.0, 20.0, 30.0)
        );
    }

    /// プレパスが BSFadeNode ルート (skeleton.nif の "Scene Root") でも
    /// 正しくボーン階層を登録できることを検証する。
    #[test]
    fn test_collect_bone_world_transforms_bsfade_root() {
        use fo3_nif::blocks::{BSFadeNode, NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: 0,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        // [0] BSFadeNode "Scene Root" (skeleton.nif のルート) → 子 [1]
        let fade_root = BSFadeNode {
            node: NiNode {
                av: make_av(Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
                children: vec![1],
                effects: vec![],
            },
        };
        // [1] ボーン "Bip01" (ローカル並進 (1, 2, 3))
        let pelvis = NiNode {
            av: make_av(Vector3 { x: 1.0, y: 2.0, z: 3.0 }),
            children: vec![],
            effects: vec![],
        };

        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![],
            },
            blocks: vec![NifBlock::BSFadeNode(fade_root), NifBlock::NiNode(pelvis)],
        };

        let mut bone_world_map = HashMap::new();
        collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut bone_world_map);

        assert!(bone_world_map.contains_key(&0));
        assert_eq!(
            bone_world_map[&1].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    /// `recompute_bone_world_map_with_pose` が、姿勢のオーバーライドをボーンのローカル変換に
    /// 反映し、Forward Kinematics でワールド行列を再計算できることを検証する。
    #[test]
    fn test_recompute_bone_world_map_with_pose() {
        use crate::animation::SkeletonPose;
        use fo3_nif::blocks::{NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |name_idx: u32, translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: name_idx,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        // [0] "Bip01": ローカル並進 (1, 2, 3) → 子 [1]
        let root = NiNode {
            av: make_av(1, Vector3 { x: 1.0, y: 2.0, z: 3.0 }),
            children: vec![1],
            effects: vec![],
        };
        // [1] "Bip01 Pelvis": ローカル並進 (4, 5, 6)
        let pelvis = NiNode {
            av: make_av(2, Vector3 { x: 4.0, y: 5.0, z: 6.0 }),
            children: vec![],
            effects: vec![],
        };

        // 文字列プール: 0 = "", 1 = "Bip01", 2 = "Bip01 Pelvis"
        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01".to_string(),
                    "Bip01 Pelvis".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiNode(root), NifBlock::NiNode(pelvis)],
        };

        // バインドポーズ FK: 骨盤のワールド並進 = (1+4, 2+5, 3+6) = (5, 7, 9)
        let mut map = HashMap::new();
        collect_bone_world_transforms(0, &NiTransform::default(), &nif, &mut map);
        assert_eq!(
            map[&1].transform_point3(Vec3::ZERO),
            Vec3::new(5.0, 7.0, 9.0)
        );

        // 姿勢: "Bip01 Pelvis" のローカル並進を (10, 20, 30) に上書き
        let mut pose = SkeletonPose::default();
        pose.overrides.insert(
            "Bip01 Pelvis".to_string(),
            NiTransform {
                rotation: glam::Mat3::IDENTITY,
                translation: Vec3::new(10.0, 20.0, 30.0),
                scale: 1.0,
            }.into(),
        );

        recompute_bone_world_map_with_pose(&nif, &pose, &mut map);
        // 骨盤のワールド並進 = (1+10, 2+20, 3+30) = (11, 22, 33)
        assert_eq!(
            map[&1].transform_point3(Vec3::ZERO),
            Vec3::new(11.0, 22.0, 33.0)
        );
        // ルート自体はポーズ対象外なのでバインドポーズのまま
        assert_eq!(
            map[&0].transform_point3(Vec3::ZERO),
            Vec3::new(1.0, 2.0, 3.0)
        );
    }

    /// `recompute_bone_world_maps_with_pose` と `resolve_bone_world_transforms_by_name` が、
    /// スケルトン側のノード名とパーツメッシュ側のボーン名を正しくマッチングしてワールド変換を解決することを検証する。
    ///
    /// 参照元: `knowledge/actor_and_skin_mesh.md` (セクション 4.4)
    #[test]
    fn test_recompute_bone_world_maps_with_pose_and_name_resolution() {
        use crate::animation::SkeletonPose;
        use fo3_nif::blocks::{NiAVObject, NiNode, NiObjectNET};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NiSkinInstance;
        use fo3_nif::NifFile;
        use fo3_nif::{Matrix33, Vector3};

        let make_av = |name_idx: u32, translation: Vector3| NiAVObject {
            net: NiObjectNET {
                name_index: name_idx,
                extra_data_list: vec![],
                controller: -1,
            },
            flags: 0,
            translation,
            rotation: Matrix33 {
                m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            scale: 1.0,
            properties: vec![],
            collision_object: -1,
        };

        let root = NiNode {
            av: make_av(1, Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
            children: vec![1],
            effects: vec![],
        };
        let pelvis = NiNode {
            av: make_av(2, Vector3 { x: 10.0, y: 0.0, z: 0.0 }),
            children: vec![],
            effects: vec![],
        };

        // スケルトン NIF: [0] = "Bip01", [1] = "Bip01 Pelvis"
        let skel_nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01".to_string(),
                    "Bip01 Pelvis".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiNode(root), NifBlock::NiNode(pelvis)],
        };

        // パーツメッシュ NIF: [0] = ダミーボーン "Bip01 Pelvis" (親子なし)
        let mesh_pelvis_bone = NiNode {
            av: make_av(1, Vector3 { x: 0.0, y: 0.0, z: 0.0 }),
            children: vec![],
            effects: vec![],
        };
        let mesh_nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![
                    String::new(),
                    "Bip01 Pelvis".to_string(),
                ],
            },
            blocks: vec![NifBlock::NiNode(mesh_pelvis_bone)],
        };

        let mut pose = SkeletonPose::default();
        pose.overrides.insert(
            "Bip01 Pelvis".to_string(),
            NiTransform {
                rotation: glam::Mat3::IDENTITY,
                translation: Vec3::new(100.0, 200.0, 300.0),
                scale: 1.0,
            }.into(),
        );

        let mut bone_world_map = HashMap::new();
        let mut bone_name_world_map = HashMap::new();
        recompute_bone_world_maps_with_pose(
            &skel_nif,
            &pose,
            &mut bone_world_map,
            &mut bone_name_world_map,
        );

        // スケルトンのボーン名マップに "Bip01 Pelvis" のワールド座標 (100, 200, 300) が格納されていること
        assert!(bone_name_world_map.contains_key("Bip01 Pelvis"));
        assert_eq!(
            bone_name_world_map["Bip01 Pelvis"].transform_point3(Vec3::ZERO),
            Vec3::new(100.0, 200.0, 300.0)
        );

        // パーツメッシュの NiSkinInstance がボーン [0] ("Bip01 Pelvis") を参照しているとき
        let skin_instance = NiSkinInstance {
            data: 0,
            skin_partition: 0,
            skeleton_root: 0,
            bones: vec![0],
        };

        // 名前引きで解決すると、スケルトン側の (100, 200, 300) が得られること
        let resolved = resolve_bone_world_transforms_by_name(
            &skin_instance,
            &mesh_nif,
            &bone_name_world_map,
            None,
        );
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0].transform_point3(Vec3::ZERO),
            Vec3::new(100.0, 200.0, 300.0)
        );
    }

    /// `is_dismember_hidden` が、全パーティションで `editor_visible == false` (切断面ゴアキャップ) のメッシュを
    /// 非表示と判定し、`editor_visible == true` を持つ通常メッシュを表示と判定することを検証する。
    ///
    /// 参照元: `references/nifskope/build/nif.xml:L2530`, `knowledge/actor_and_skin_mesh.md` (セクション 4.5)
    #[test]
    fn test_is_dismember_hidden() {
        use fo3_nif::blocks::{BSDismemberSkinInstance, BodyPartList};
        use fo3_nif::NiSkinInstance;
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};

        let dummy_skin = NiSkinInstance {
            data: 0,
            skin_partition: 0,
            skeleton_root: 0,
            bones: vec![],
        };

        // 通常メッシュ: パーティション [0] の flag = 0x0101 (editor_visible = true)
        let normal_bdsi = BSDismemberSkinInstance {
            skin_instance: dummy_skin.clone(),
            partitions: vec![
                BodyPartList { part_flag: 0x0101, body_part: 5 },
                BodyPartList { part_flag: 0x0001, body_part: 3 },
            ],
        };

        // 切断面ゴアキャップメッシュ: 全パーティションの flag = 0x0100 / 0x0000 (editor_visible = false)
        let gore_cap_bdsi = BSDismemberSkinInstance {
            skin_instance: dummy_skin,
            partitions: vec![
                BodyPartList { part_flag: 0x0100, body_part: 105 }, // BP_SECTIONCAP_RIGHTARM
                BodyPartList { part_flag: 0x0000, body_part: 103 }, // BP_SECTIONCAP_LEFTARM
            ],
        };

        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![],
            },
            blocks: vec![
                NifBlock::BSDismemberSkinInstance(normal_bdsi),
                NifBlock::BSDismemberSkinInstance(gore_cap_bdsi),
            ],
        };

        // 通常メッシュ（ブロック 0）は非表示にならない
        assert_eq!(is_dismember_hidden(0, &nif), false);
        // 切断面ゴアキャップ（ブロック 1）は通常時非表示になる
        assert_eq!(is_dismember_hidden(1, &nif), true);
        // スキンインスタンスなし（-1）は非表示にならない
        assert_eq!(is_dismember_hidden(-1, &nif), false);
    }

    /// `collect_anim_skin_meshes_for_part` が、マルチパーツ構成において各パーツのインデックスと
    /// オフセットを正しく付与してスキンメッシュを収集することを検証する。
    #[test]
    fn test_collect_anim_skin_meshes_for_part() {
        use fo3_nif::blocks::{BSDismemberSkinInstance, BodyPartList, NiAVObject, NiGeometry, NiObjectNET, NiTriShape, NiTriShapeData};
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};
        use fo3_nif::NiSkinInstance;
        use fo3_nif::{Matrix33, Vector3};

        let dummy_skin = NiSkinInstance {
            data: 0,
            skin_partition: 0,
            skeleton_root: 0,
            bones: vec![],
        };

        let normal_bdsi = BSDismemberSkinInstance {
            skin_instance: dummy_skin,
            partitions: vec![
                BodyPartList { part_flag: 0x0101, body_part: 5 },
            ],
        };

        let tri_shape = NiTriShape {
            geom: NiGeometry {
                av: NiAVObject {
                    net: NiObjectNET { name_index: 1, extra_data_list: vec![], controller: -1 },
                    flags: 0,
                    translation: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
                    rotation: Matrix33 { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] },
                    scale: 1.0,
                    properties: vec![],
                    collision_object: -1,
                },
                data: 1,
                skin_instance: 2,
                material_data: fo3_nif::blocks::geometry::MaterialData {
                    material_names: vec![],
                    material_extra_data: vec![],
                    active_material: 0,
                    material_needs_update: false,
                },
            },
        };

        let dummy_geo_data = NiTriShapeData {
            common: fo3_nif::blocks::geometry::NiGeometryDataCommon {
                num_vertices: 0,
                vertices: vec![],
                bs_data_flags: 0,
                normals: vec![],
                tangents: vec![],
                bitangents: vec![],
                bounding_sphere: fo3_nif::types::BoundingSphere {
                    center: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
                    radius: 0.0,
                },
                vertex_colors: vec![],
                uv_sets: vec![],
            },
            num_triangles: 0,
            triangles: vec![],
            match_groups: vec![],
        };

        let nif = NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![String::new(), "PartMesh".to_string()],
            },
            blocks: vec![
                NifBlock::NiTriShape(tri_shape),
                NifBlock::NiTriShapeData(dummy_geo_data),
                NifBlock::BSDismemberSkinInstance(normal_bdsi),
            ],
        };

        // メッシュ名配列（オフセット 2）
        let mesh_names = ["OtherMesh0", "OtherMesh1", "PartMesh"];

        let anims = collect_anim_skin_meshes_for_names(3, 2, &nif, &mesh_names);
        assert_eq!(anims.len(), 1);
        assert_eq!(anims[0].mesh_index, 2);
        assert_eq!(anims[0].part_index, 3);
        assert_eq!(anims[0].geo_data_block, 1);
        assert_eq!(anims[0].skin_instance_block, 2);
    }

    /// 実アセットを用いたアクターパーツのボーン名解決検証テスト。
    ///
    /// スケルトン `skeleton.nif` から得られるボーン名マップに対し、
    /// 全身パーツ (頭, 体, 両手, 両目, 歯, 舌) の全スキンメッシュが参照するボーン名が
    /// すべて過不足なく解決できることを検証する。
    #[test]
    fn test_actor_parts_bone_resolution_real_assets() {
        use fo3_vfs::VfsManager;
        use fo3_bsa::BsaArchive;
        use std::path::Path;
        use std::io::Cursor;
        use crate::recompute_bone_world_maps_with_pose;
        use crate::animation::SkeletonPose;

        let data_dir = Path::new(r"A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data");
        if !data_dir.exists() {
            return;
        }

        let mut vfs = VfsManager::new();
        vfs.add_loose_root(data_dir);
        let mesh_bsa = data_dir.join("Fallout - Meshes.bsa");
        if let Ok(archive) = BsaArchive::open(&mesh_bsa) {
            vfs.add_bsa(archive);
        }

        // スケルトン NIF のロード
        let skel_bytes = match vfs.read("meshes\\characters\\_male\\skeleton.nif") {
            Ok(b) => b,
            Err(_) => return,
        };
        let mut cur = Cursor::new(skel_bytes);
        let skel_nif = NifFile::read(&mut cur).expect("Failed to parse skeleton.nif");

        let mut bone_world_map = HashMap::new();
        let mut bone_name_world_map = HashMap::new();
        let initial_pose = SkeletonPose::default();
        recompute_bone_world_maps_with_pose(
            &skel_nif,
            &initial_pose,
            &mut bone_world_map,
            &mut bone_name_world_map,
        );

        println!("スケルトン解決ボーン名総数: {} 件", bone_name_world_map.len());
        for name in &["Bip01", "Bip01 NonAccum", "Bip01 Pelvis", "Bip01 Spine", "Bip01 Spine1", "Bip01 Spine2", "Bip01 Neck", "Bip01 Head", "Bip01 L Thigh", "Bip01 L Clavicle", "Bip01 L UpperArm", "Bip01 L Hand"] {
            if let Some(mat) = bone_name_world_map.get(*name) {
                println!("  ボーン {}: pos = {:?}", name, mat.w_axis.truncate());
            } else {
                println!("  ボーン {}: (未存在)", name);
            }
        }

        // アイドルアニメーション KF 適用後のボーン位置検証
        let kf_path = "meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf";
        if let Ok(kf_bytes) = vfs.read(kf_path) {
            let mut cur = Cursor::new(kf_bytes);
            if let Ok(kf_nif) = NifFile::read(&mut cur) {
                if let Some(_clip) = crate::animation::AnimationClip::from_kf(&kf_nif) {
                    let mut pose = SkeletonPose::default();
                    crate::animation::apply_pose(&kf_nif, &mut pose, 0.0);
                    println!("Bip01 override: {:?}", pose.overrides.get("Bip01"));
                    println!("Bip01 NonAccum override: {:?}", pose.overrides.get("Bip01 NonAccum"));
                    let mut anim_bone_world_map = HashMap::new();
                    let mut anim_bone_name_world_map = HashMap::new();
                    recompute_bone_world_maps_with_pose(
                        &skel_nif,
                        &pose,
                        &mut anim_bone_world_map,
                        &mut anim_bone_name_world_map,
                    );
                    let pelvis_z = anim_bone_name_world_map["Bip01 Pelvis"].w_axis.z;
                    assert!(
                        (pelvis_z - 66.33).abs() < 1.0,
                        "Bip01 Pelvis の高さが不正です (ダブルトランスフォーム再発の恐れ): z = {}",
                        pelvis_z
                    );
                    let head_z = anim_bone_name_world_map["Bip01 Head"].w_axis.z;
                    assert!(
                        (head_z - 111.41).abs() < 2.0,
                        "Bip01 Head の高さが不正です: z = {}",
                        head_z
                    );
                }
            }
        }

        let part_paths = [
            "meshes\\characters\\head\\headhuman.nif",
            "meshes\\characters\\head\\headghoul.nif",
            "meshes\\characters\\head\\eyelefthuman.nif",
            "meshes\\characters\\head\\eyerighthuman.nif",
            "meshes\\characters\\head\\teethupperhuman.nif",
            "meshes\\characters\\head\\teethlowerhuman.nif",
            "meshes\\characters\\head\\tonguehuman.nif",
            "meshes\\characters\\hair\\hairmessy02.nif",
            "meshes\\characters\\_male\\upperbody.nif",
            "meshes\\armor\\wastelandclothing01\\outfitm.nif",
            "meshes\\armor\\wastelandclothing01\\outfitf.nif",
            "meshes\\characters\\_male\\righthand.nif",
            "meshes\\characters\\_male\\lefthand.nif",
            "meshes\\characters\\_male\\femalerighthand.nif",
            "meshes\\characters\\_male\\femalelefthand.nif",
        ];

        for path in &part_paths {
            let bytes = vfs.read(path).unwrap_or_else(|_| panic!("Failed to read {}", path));
            let mut c = Cursor::new(bytes);
            let part_nif = NifFile::read(&mut c).unwrap_or_else(|_| panic!("Failed to parse {}", path));

            // 全スキンインスタンスのボーンを検証
            let mut missing_bones = Vec::new();
            let mut total_bones = 0;
            for block in &part_nif.blocks {
                let inst = match block {
                    NifBlock::NiSkinInstance(i) => i,
                    NifBlock::BSDismemberSkinInstance(d) => &d.skin_instance,
                    _ => continue,
                };
                for &bone_idx in &inst.bones {
                    if bone_idx < 0 || bone_idx as usize >= part_nif.blocks.len() {
                        continue;
                    }
                    total_bones += 1;
                    let bone_name = match &part_nif.blocks[bone_idx as usize] {
                        NifBlock::NiNode(n) => part_nif.get_string(n.av.net.name_index),
                        NifBlock::BSFadeNode(f) => part_nif.get_string(f.node.av.net.name_index),
                        _ => None,
                    };
                    if let Some(name) = bone_name {
                        if !bone_name_world_map.contains_key(name) {
                            missing_bones.push(name.to_string());
                        }
                    } else {
                        missing_bones.push(format!("(unnamed block {})", bone_idx));
                    }
                }
            }
            assert!(missing_bones.is_empty(), "パーツ {} に未解決ボーンがあります: {:?}", path, missing_bones);
            if path.contains("upperbody") || path.contains("hand") || path.contains("outfit") {
                assert!(total_bones > 0, "スキンパーツ {} のボーン数が 0 です", path);
            }

            // righthand.nif の場合、各ボーンのワールド位置をスケルトンと比較
            if path.contains("righthand.nif") {
                let mut part_bone_map = HashMap::new();
                crate::collect_bone_world_transforms(0, &fo3_gamebryo_core::NiTransform::default(), &part_nif, &mut part_bone_map);
                for block in &part_nif.blocks {
                    let inst = match block {
                        NifBlock::NiSkinInstance(i) => i,
                        NifBlock::BSDismemberSkinInstance(d) => &d.skin_instance,
                        _ => continue,
                    };
                    for &bone_idx in &inst.bones {
                        if bone_idx < 0 || bone_idx as usize >= part_nif.blocks.len() { continue; }
                        let name = match &part_nif.blocks[bone_idx as usize] {
                            NifBlock::NiNode(n) => part_nif.get_string(n.av.net.name_index),
                            _ => None,
                        };
                        if let Some(name) = name {
                            let skel_pos = bone_name_world_map[name].transform_point3(glam::Vec3::ZERO);
                            let part_pos = part_bone_map[&bone_idx].transform_point3(glam::Vec3::ZERO);
                            let diff = (skel_pos - part_pos).length();
                            assert!(diff < 0.5, "ボーン {} の位置が不一致: diff = {}", name, diff);
                        }
                    }
                }
            }

            // HeadParts のアタッチ先ボーン検出およびアライメント補正検証
            let attach_bone = crate::find_attach_bone_name(&part_nif);
            if path.contains("eye") || path.contains("teeth") || path.contains("tongue") || path.contains("hair") {
                assert_eq!(
                    attach_bone.as_deref(),
                    Some("Bip01 Head"),
                    "パーツ {} のアタッチ先ボーンが Bip01 Head ではありません: {:?}",
                    path,
                    attach_bone
                );

                // collect_anim_rigid_meshes_for_part による姿勢アライメント検証
                let mesh_names: Vec<&str> = vec!["mesh_0", "mesh_1"];
                let rigids = super::collect_anim_rigid_meshes_for_part(0, "Bip01 Head", &part_nif, &mesh_names);
                for rigid in &rigids {
                    if let Some(head_mat) = bone_name_world_map.get("Bip01 Head") {
                        let combined = *head_mat * rigid.local_transform;
                        // 合成回転行列の上方向 (Z軸) 成分が正立 (+Z 方向) になっていることを検証
                        let z_axis = combined.col(2).truncate();
                        assert!(
                            z_axis.z > 0.8,
                            "パーツ {} の剛体アタッチメント姿勢が正立していません: combined Z軸 = {:?}",
                            path,
                            z_axis
                        );
                    }
                }
            }
        }
    }

    /// `RenderActorInstance` のデータ構造整合性と、アニメーション更新時の剛体・スキン追従動作を検証する。
    #[test]
    fn test_render_actor_instance_hierarchy() {
        use std::sync::Arc;
        use fo3_gamebryo_core::NiTransform;
        use fo3_nif::header::{BSStreamHeader, ExportString, NifHeader};

        let dummy_nif = Arc::new(NifFile {
            header: NifHeader {
                header_string: "Gamebryo File Format, Version 20.2.0.7\n".to_string(),
                version: 0x14020007,
                endian_type: 1,
                user_version: 11,
                num_blocks: 0,
                bs_header: BSStreamHeader {
                    bs_version: 34,
                    author: ExportString { value: String::new() },
                    process_script: None,
                    export_script: ExportString { value: String::new() },
                },
                block_types: vec![],
                block_type_indices: vec![],
                block_sizes: vec![],
                strings: vec![],
            },
            blocks: vec![],
        });

        let mut scene = RenderScene {
            meshes: Vec::new(),
            collision_meshes: Vec::new(),
            bounds_center: glam::Vec3::ZERO,
            bounds_radius: 100.0,
            anim_skin_meshes: Vec::new(),
            anim_rigid_meshes: Vec::new(),
            actors: Vec::new(),
        };

        let actor = RenderActorInstance {
            form_id: 0x00012345,
            name: "TestNPC".to_string(),
            world_transform: NiTransform::from_euler_xyz(glam::Vec3::new(100.0, 200.0, 300.0), glam::Vec3::ZERO, 1.0),
            skeleton_nif: dummy_nif.clone(),
            parts: vec![dummy_nif.clone()],
            anim_player: None,
            kf_nif: None,
            anim_pose: crate::animation::SkeletonPose::default(),
            anim_skin_meshes: Vec::new(),
            anim_rigid_meshes: vec![
                AnimatedRigidMesh {
                    mesh_index: 0,
                    bone_name: "Bip01 Head".to_string(),
                    local_transform: glam::Mat4::IDENTITY,
                }
            ],
        };

        scene.actors.push(actor);
        assert_eq!(scene.actors.len(), 1);
        assert_eq!(scene.actors[0].name, "TestNPC");
        assert_eq!(scene.actors[0].world_transform.translation, glam::Vec3::new(100.0, 200.0, 300.0));
        assert_eq!(scene.actors[0].anim_rigid_meshes[0].bone_name, "Bip01 Head");
    }

    #[test]
    fn test_inspect_hair_and_hands() {
        use fo3_vfs::VfsManager;
        use fo3_bsa::BsaArchive;
        use std::path::Path;
        use std::io::Cursor;

        let data_dir = Path::new(r"A:\SteamLibrary\steamapps\common\Fallout 3 goty\Data");
        if !data_dir.exists() { return; }
        let mut vfs = VfsManager::new();
        vfs.add_loose_root(data_dir);
        let mesh_bsa = data_dir.join("Fallout - Meshes.bsa");
        if let Ok(archive) = BsaArchive::open(&mesh_bsa) {
            vfs.add_bsa(archive);
        }

        let hair_paths = [
            "meshes\\characters\\hair\\hairbun.nif",
            "meshes\\characters\\hair\\hairmessy02.nif",
            "meshes\\characters\\hair\\hairmessy03.nif",
        ];

        for path in &hair_paths {
            if let Ok(bytes) = vfs.read(path) {
                let mut cur = Cursor::new(bytes);
                if let Ok(nif) = NifFile::read(&mut cur) {
                    println!("\n=== Hair NIF: {} ===", path);
                    for (i, block) in nif.blocks.iter().enumerate() {
                        match block {
                            NifBlock::NiTriShape(shape) => {
                                let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
                                println!("  Block {}: NiTriShape '{}', skin_inst={}, props={:?}", i, name, shape.geom.skin_instance, shape.geom.av.properties);
                                for &p in &shape.geom.av.properties {
                                    if p >= 0 && (p as usize) < nif.blocks.len() {
                                        match &nif.blocks[p as usize] {
                                            NifBlock::NiAlphaProperty(a) => {
                                                println!("    AlphaProp: flags=0x{:04X}, blend={}, test={}, test_func={}, thresh={}",
                                                    a.flags, a.is_blend_enabled(), a.is_test_enabled(), a.test_func(), a.threshold_normalized());
                                            }
                                            NifBlock::NiMaterialProperty(m) => {
                                                println!("    MaterialProp: specular={:?}, gloss={}", m.specular_color, m.glossiness);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            NifBlock::NiAlphaProperty(a) => {
                                println!("  Block {}: NiAlphaProperty flags=0x{:04X}, raw_thresh={}, blend={}, test={}, test_func={}, thresh={}",
                                    i, a.flags, a.threshold, a.is_blend_enabled(), a.is_test_enabled(), a.test_func(), a.threshold_normalized());
                                println!("    net: name_idx={}, extra={:?}, ctl={}", a.net.name_index, a.net.extra_data_list, a.net.controller);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let hand_paths = [
            "meshes\\characters\\_male\\righthand.nif",
            "meshes\\characters\\_male\\femalerighthand.nif",
        ];
        for path in &hand_paths {
            if let Ok(bytes) = vfs.read(path) {
                let mut cur = Cursor::new(bytes);
                if let Ok(nif) = NifFile::read(&mut cur) {
                    println!("\n=== Hand NIF: {} ===", path);
                    for (i, block) in nif.blocks.iter().enumerate() {
                        match block {
                            NifBlock::NiTriShape(shape) => {
                                let name = nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
                                println!("  Block {}: NiTriShape '{}', data={}, skin_inst={}, props={:?}", i, name, shape.geom.data, shape.geom.skin_instance, shape.geom.av.properties);
                            }
                            NifBlock::NiTriShapeData(data) => {
                                println!("  Block {}: NiTriShapeData, num_vertices={}, num_triangles={}", i, data.common.num_vertices, data.num_triangles);
                            }
                            NifBlock::BSDismemberSkinInstance(d) => {
                                println!("  Block {}: BSDismemberSkinInstance, partitions={:?}, bones={:?}", i, d.partitions, d.skin_instance.bones);
                                for &b in &d.skin_instance.bones {
                                    if b >= 0 && (b as usize) < nif.blocks.len() {
                                        let bname = match &nif.blocks[b as usize] {
                                            NifBlock::NiNode(n) => nif.get_string(n.av.net.name_index),
                                            _ => None,
                                        };
                                        println!("    Bone block {}: {:?}", b, bname);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // スケルトンと femalerighthand.nif のスキニング計算を実際に実行し、頂点座標を検証
        let skel_bytes = vfs.read("meshes\\characters\\_male\\skeleton.nif").unwrap();
        let skel_nif = NifFile::read(&mut Cursor::new(skel_bytes)).unwrap();
        let hand_bytes = vfs.read("meshes\\characters\\_male\\femalerighthand.nif").unwrap();
        let hand_nif = NifFile::read(&mut Cursor::new(hand_bytes)).unwrap();

        let mut bone_world_map = HashMap::new();
        let mut bone_name_world_map = HashMap::new();
        let initial_pose = crate::animation::SkeletonPose::default();
        crate::recompute_bone_world_maps_with_pose(&skel_nif, &initial_pose, &mut bone_world_map, &mut bone_name_world_map);

        let _shape = match &hand_nif.blocks[1] {
            NifBlock::NiTriShape(s) => s,
            _ => panic!("Block 1 is not NiTriShape"),
        };
        let data = match &hand_nif.blocks[5] {
            NifBlock::NiTriShapeData(d) => d,
            _ => panic!("Block 5 is not NiTriShapeData"),
        };
        let inst = match &hand_nif.blocks[6] {
            NifBlock::BSDismemberSkinInstance(d) => &d.skin_instance,
            _ => panic!("Block 6 is not BSDismemberSkinInstance"),
        };

        let bone_transforms = crate::resolve_bone_world_transforms_by_name(inst, &hand_nif, &bone_name_world_map, None);
        println!("\n=== Female Right Hand Skinning Check ===");
        println!("Resolved bone transforms count: {}", bone_transforms.len());
        if let Some((pos, _nrm)) = crate::skinning::apply_skinning_cpu_with_bones(data, inst, &hand_nif, Some(&bone_transforms)) {
            println!("Skinned vertices count: {}", pos.len());
            let mut min = glam::Vec3::splat(f32::MAX);
            let mut max = glam::Vec3::splat(f32::MIN);
            for p in &pos {
                let v = glam::Vec3::new(p[0], p[1], p[2]);
                min = min.min(v);
                max = max.max(v);
            }
            println!("Skinned hand bounds: min={:?}, max={:?}", min, max);
        } else {
            println!("ERROR: apply_skinning_cpu_with_bones returned None!");
        }

        // 左手 (femalelefthand.nif) のスキニング計算も検証
        let left_hand_bytes = vfs.read("meshes\\characters\\_male\\femalelefthand.nif").unwrap();
        let left_hand_nif = NifFile::read(&mut Cursor::new(left_hand_bytes)).unwrap();
        let left_data = match &left_hand_nif.blocks[5] {
            NifBlock::NiTriShapeData(d) => d,
            _ => panic!("Block 5 is not NiTriShapeData"),
        };
        let left_inst = match &left_hand_nif.blocks[6] {
            NifBlock::BSDismemberSkinInstance(d) => &d.skin_instance,
            _ => panic!("Block 6 is not BSDismemberSkinInstance"),
        };
        let left_bone_transforms = crate::resolve_bone_world_transforms_by_name(left_inst, &left_hand_nif, &bone_name_world_map, None);
        println!("\n=== Female Left Hand Skinning Check ===");
        println!("Resolved left bone transforms count: {}", left_bone_transforms.len());
        if let Some((pos, _nrm)) = crate::skinning::apply_skinning_cpu_with_bones(left_data, left_inst, &left_hand_nif, Some(&left_bone_transforms)) {
            println!("Skinned left vertices count: {}", pos.len());
            let mut min = glam::Vec3::splat(f32::MAX);
            let mut max = glam::Vec3::splat(f32::MIN);
            for p in &pos {
                let v = glam::Vec3::new(p[0], p[1], p[2]);
                min = min.min(v);
                max = max.max(v);
            }
            println!("Skinned left hand bounds: min={:?}, max={:?}", min, max);
            // 左手は -X 側に存在することを確認
            assert!(min.x < -30.0, "左手は -X 側に配置される必要があります: min.x={}", min.x);
        } else {
            println!("ERROR: left apply_skinning_cpu_with_bones returned None!");
        }

        // アニメーション再生状態での手のスキニング検証 (指ボーンが潰れないことの検証)
        let kf_bytes = vfs.read("meshes\\characters\\_male\\idleanims\\ttnpchappysubtlelistena.kf").unwrap();
        let kf_nif = NifFile::read(&mut Cursor::new(kf_bytes)).unwrap();
        let mut anim_pose = crate::animation::SkeletonPose::default();
        crate::animation::apply_pose(&kf_nif, &mut anim_pose, 1.0);

        let mut anim_bone_world_map = HashMap::new();
        let mut anim_bone_name_world_map = HashMap::new();
        crate::recompute_bone_world_maps_with_pose(&skel_nif, &anim_pose, &mut anim_bone_world_map, &mut anim_bone_name_world_map);

        let anim_right_transforms = crate::resolve_bone_world_transforms_by_name(inst, &hand_nif, &anim_bone_name_world_map, None);
        let anim_left_transforms = crate::resolve_bone_world_transforms_by_name(left_inst, &left_hand_nif, &anim_bone_name_world_map, None);

        let (right_anim_pos, _) = crate::skinning::apply_skinning_cpu_with_bones(data, inst, &hand_nif, Some(&anim_right_transforms)).unwrap();
        let (left_anim_pos, _) = crate::skinning::apply_skinning_cpu_with_bones(left_data, left_inst, &left_hand_nif, Some(&anim_left_transforms)).unwrap();

        let r_min = right_anim_pos.iter().fold(glam::Vec3::splat(f32::MAX), |a, p| a.min(glam::Vec3::new(p[0], p[1], p[2])));
        let r_max = right_anim_pos.iter().fold(glam::Vec3::splat(f32::MIN), |a, p| a.max(glam::Vec3::new(p[0], p[1], p[2])));
        let l_min = left_anim_pos.iter().fold(glam::Vec3::splat(f32::MAX), |a, p| a.min(glam::Vec3::new(p[0], p[1], p[2])));
        let l_max = left_anim_pos.iter().fold(glam::Vec3::splat(f32::MIN), |a, p| a.max(glam::Vec3::new(p[0], p[1], p[2])));

        println!("\n=== Animated Hands Bounds (ttnpchappysubtlelistena.kf) ===");
        println!("Right hand: min={:?}, max={:?}, span_x={}", r_min, r_max, r_max.x - r_min.x);
        println!("Left hand:  min={:?}, max={:?}, span_x={}", l_min, l_max, l_max.x - l_min.x);

        // 指が同一座標に潰れていないことの検証: 手のバウンディングボックスのサイズが一定以上あること
        assert!((r_max.x - r_min.x) > 5.0, "右手の手・指スパンが潰れていません: span_x={}", r_max.x - r_min.x);
        assert!((l_max.x - l_min.x) > 5.0, "左手の手・指スパンが潰れていません: span_x={}", l_max.x - l_min.x);
        assert!(r_min.z > 30.0 && r_max.z < 100.0, "右手の高さが正常範囲内");
        assert!(l_min.z > 30.0 && l_max.z < 100.0, "左手の高さが正常範囲内");

        let outfit_bytes = vfs.read("meshes\\armor\\wastelandclothing01\\outfitf.nif").unwrap();
        let outfit_nif = NifFile::read(&mut Cursor::new(outfit_bytes)).unwrap();
        // outfitf.nif 内の腕メッシュ (Arms03:0, Arms03:1, Arms, Arms04) のボーンと頂点範囲を調査
        for (i, block) in outfit_nif.blocks.iter().enumerate() {
            if let NifBlock::NiTriShape(shape) = block {
                let name = outfit_nif.get_string(shape.geom.av.net.name_index).unwrap_or("");
                if name.starts_with("Arms") || name.starts_with("arms") {
                    println!("\n--- Outfit Mesh: {} (Block {}) ---", name, i);
                    if shape.geom.skin_instance >= 0 {
                        if let NifBlock::BSDismemberSkinInstance(bdsi) = &outfit_nif.blocks[shape.geom.skin_instance as usize] {
                            let mut bone_names = Vec::new();
                            for &b in &bdsi.skin_instance.bones {
                                if b >= 0 && (b as usize) < outfit_nif.blocks.len() {
                                    if let NifBlock::NiNode(n) = &outfit_nif.blocks[b as usize] {
                                        bone_names.push(outfit_nif.get_string(n.av.net.name_index).unwrap_or(""));
                                    }
                                }
                            }
                            println!("  Bones ({}) : {:?}", bone_names.len(), bone_names);
                            let resolved = crate::resolve_bone_world_transforms_by_name(&bdsi.skin_instance, &outfit_nif, &bone_name_world_map, None);
                            if let NifBlock::NiTriShapeData(d) = &outfit_nif.blocks[shape.geom.data as usize] {
                                if let Some((pos, _)) = crate::skinning::apply_skinning_cpu_with_bones(d, &bdsi.skin_instance, &outfit_nif, Some(&resolved)) {
                                    let mut min = glam::Vec3::splat(f32::MAX);
                                    let mut max = glam::Vec3::splat(f32::MIN);
                                    for p in &pos {
                                        let v = glam::Vec3::new(p[0], p[1], p[2]);
                                        min = min.min(v);
                                        max = max.max(v);
                                    }
                                    println!("  Skinned bounds: min={:?}, max={:?}", min, max);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

