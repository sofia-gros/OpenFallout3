import sys
content = open('crates/fo3_render/src/scene/traversal.rs', 'r', encoding='utf-8').read()

patch_args = '''pub fn traverse_block(
    block_index: i32,
    parent_world: &NiTransform,
    parent_alpha: Option<&fo3_nif::NiAlphaProperty>,
    parent_material: Option<&fo3_nif::NiMaterialProperty>,
    parent_bone_name: Option<&str>,
    nif: &NifFile,'''

content = content.replace('''pub fn traverse_block(
    block_index: i32,
    parent_world: &NiTransform,
    parent_alpha: Option<&fo3_nif::NiAlphaProperty>,
    parent_material: Option<&fo3_nif::NiMaterialProperty>,
    nif: &NifFile,''', patch_args)

patch_node = '''            let node_name = nif.get_string(node.av.net.name_index as u32).unwrap_or("");
            let current_bone_name = if !node_name.is_empty() { Some(node_name) } else { parent_bone_name };
            
            for &child in &node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    current_bone_name,
                    nif,'''

content = content.replace('''            for &child in &node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    nif,''', patch_node)

patch_fade = '''            let node_name = nif.get_string(fade.node.av.net.name_index as u32).unwrap_or("");
            let current_bone_name = if !node_name.is_empty() { Some(node_name) } else { parent_bone_name };

            for &child in &fade.node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    current_bone_name,
                    nif,'''

content = content.replace('''            for &child in &fade.node.children {
                traverse_block(
                    child,
                    &world_transform,
                    current_alpha,
                    current_material,
                    nif,''', patch_fade)

patch_rigid1 = '''                                if let Some(bone_name) = collector.attach_bone.or(parent_bone_name) {
                                    let local_transform = compute_rigid_part_local_transform(
                                        block_index as usize,
                                        bone_name,
                                        &collector.shape_transforms,
                                        &shape.geom.av,
                                    );
                                    collector.anim_rigids.push(AnimatedRigidMesh {
                                        mesh_index,
                                        bone_name: bone_name.to_string(),
                                        local_transform,
                                    });
                                }'''
content = content.replace('''                                if let Some(bone_name) = collector.attach_bone {
                                    let local_transform = compute_rigid_part_local_transform(
                                        block_index as usize,
                                        bone_name,
                                        &collector.shape_transforms,
                                        &shape.geom.av,
                                    );
                                    collector.anim_rigids.push(AnimatedRigidMesh {
                                        mesh_index,
                                        bone_name: bone_name.to_string(),
                                        local_transform,
                                    });
                                }''', patch_rigid1)

patch_rigid2 = '''                                if let Some(bone_name) = collector.attach_bone.or(parent_bone_name) {
                                    let local_transform = compute_rigid_part_local_transform(
                                        block_index as usize,
                                        bone_name,
                                        &collector.shape_transforms,
                                        &strips.geom.av,
                                    );
                                    collector.anim_rigids.push(AnimatedRigidMesh {
                                        mesh_index,
                                        bone_name: bone_name.to_string(),
                                        local_transform,
                                    });
                                }'''
content = content.replace('''                                if let Some(bone_name) = collector.attach_bone {
                                    let local_transform = compute_rigid_part_local_transform(
                                        block_index as usize,
                                        bone_name,
                                        &collector.shape_transforms,
                                        &strips.geom.av,
                                    );
                                    collector.anim_rigids.push(AnimatedRigidMesh {
                                        mesh_index,
                                        bone_name: bone_name.to_string(),
                                        local_transform,
                                    });
                                }''', patch_rigid2)


open('crates/fo3_render/src/scene/traversal.rs', 'w', encoding='utf-8').write(content)
