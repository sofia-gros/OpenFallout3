import sys
content = open('crates/fo3_viewer/src/loader.rs', 'r', encoding='utf-8').read()

patch = '''
                    let mut animated_statics: Vec<(u32, std::sync::Arc<fo3_nif::NifFile>, fo3_gamebryo_core::NiTransform, std::sync::Arc<fo3_render::animation::AnimationClip>)> = Vec::new();

                    let cell_inputs: Vec<(
                        Vec<(&fo3_nif::NifFile, fo3_gamebryo_core::NiTransform)>,
                        Option<(&fo3_esm::records::LandRecord, i32, i32)>,
                    )> = cells
                        .iter()
                        .zip(all_cell_items.iter())
                        .map(|((cell, _, land), items)| {
                            let mut placed_refs = Vec::new();
                            for (form_id, n, t) in items {
                                if let Some(clip) = fo3_render::animation::AnimationClip::from_transform_controllers(n.as_ref()) {
                                    animated_statics.push((*form_id, n.clone(), *t, std::sync::Arc::new(clip)));
                                } else {
                                    placed_refs.push((n.as_ref(), *t));
                                }
                            }
                            let land_info = land
                                .as_ref()
                                .and_then(|l| cell.grid.map(|(gx, gy)| (l, gx, gy)));
                            (placed_refs, land_info)
                        })
                        .collect();
'''
content = content.replace('''                    let cell_inputs: Vec<(
                        Vec<(&NifFile, NiTransform)>,
                        Option<(&LandRecord, i32, i32)>,
                    )> = cells
                        .iter()
                        .zip(all_cell_items.iter())
                        .map(|((cell, _, land), items)| {
                            let placed_refs: Vec<(&NifFile, NiTransform)> =
                                items.iter().map(|(_, n, t)| (n.as_ref(), *t)).collect();
                            let land_info = land
                                .as_ref()
                                .and_then(|l| cell.grid.map(|(gx, gy)| (l, gx, gy)));
                            (placed_refs, land_info)
                        })
                        .collect();''', patch)

patch2 = '''
                    for (form_id, nif, transform, clip) in animated_statics {
                        scene.add_animated_static(
                            device,
                            queue,
                            context,
                            vfs,
                            form_id,
                            "AnimatedStatic",
                            &transform,
                            nif,
                            Some(clip),
                            &mut texture_cache,
                        );
                    }

                    if !cell_npcs.is_empty() {
'''
content = content.replace('''
                    if !cell_npcs.is_empty() {
''', patch2)

open('crates/fo3_viewer/src/loader.rs', 'w', encoding='utf-8').write(content)
