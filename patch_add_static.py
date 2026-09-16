import sys
content = open('crates/fo3_render/src/scene/actor.rs', 'r', encoding='utf-8').read()

patch = '''
    pub fn add_animated_static(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        context: &RenderContext,
        vfs: &mut VfsManager,
        form_id: u32,
        name: &str,
        world_transform: &NiTransform,
        nif: Arc<NifFile>,
        anim_clip: Option<Arc<crate::animation::AnimationClip>>,
        texture_cache: &mut HashMap<String, GpuTexture>,
    ) -> usize {
        let parts = vec![nif.clone()];
        let part_refs: Vec<&NifFile> = parts.iter().map(|p| p.as_ref()).collect();
        let mut sub_scene = RenderScene::from_actor_parts(
            device,
            queue,
            context,
            &nif,
            &part_refs,
            vfs,
        );

        let start_idx = self.meshes.len();
        self.meshes.append(&mut sub_scene.meshes);

        for anim_skin in &mut sub_scene.anim_skin_meshes {
            anim_skin.mesh_index += start_idx;
        }
        for anim_rigid in &mut sub_scene.anim_rigid_meshes {
            anim_rigid.mesh_index += start_idx;
        }

        let anim_player = anim_clip.map(|clip| {
            let mut player = crate::animation::AnimationPlayer::new(clip);
            player.play();
            player
        });

        let actor = RenderActorInstance {
            form_id,
            name: name.to_string(),
            world_transform: world_transform.clone(),
            skeleton_nif: nif,
            parts,
            anim_player,
            kf_nif: None,
            sequence_manager: None,
            anim_skin_meshes: sub_scene.anim_skin_meshes,
            anim_rigid_meshes: sub_scene.anim_rigid_meshes,
            blend_from: None,
            blend_total: 0.0,
            anim_pose: crate::animation::SkeletonPose::default(),
        };

        let actor_idx = self.actors.len();
        self.actors.push(actor);
        actor_idx
    }
'''

content = content[:content.find('pub fn add_animated_static')] + patch + content[content.find('pub fn add_actor('):]
open('crates/fo3_render/src/scene/actor.rs', 'w', encoding='utf-8').write(content)
