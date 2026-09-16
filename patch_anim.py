import sys
content = open('crates/fo3_render/src/animation.rs', 'r', encoding='utf-8').read()

patch = '''
    /// NIF 内のすべての NiTransformController から、デフォルトのアニメーションクリップを生成する
    pub fn from_transform_controllers(nif: &NifFile) -> Option<Self> {
        let mut channels = HashMap::new();
        let mut min_start = f32::MAX;
        let mut max_stop = f32::MIN;

        for block in &nif.blocks {
            if let NifBlock::NiTransformController(ctrl) = block {
                if ctrl.target >= 0 && (ctrl.target as usize) < nif.blocks.len() {
                    let target_name = match &nif.blocks[ctrl.target as usize] {
                        NifBlock::NiNode(n) => nif.get_string(n.av.net.name_index as u32).unwrap_or(""),
                        NifBlock::NiTriShape(t) => nif.get_string(t.geom.av.net.name_index as u32).unwrap_or(""),
                        _ => "",
                    };
                    if !target_name.is_empty() {
                        channels.insert(
                            target_name.to_string(),
                            BoneChannel {
                                bone_name: target_name.to_string(),
                                interpolator_index: ctrl.interpolator,
                            },
                        );
                        if ctrl.start_time < min_start {
                            min_start = ctrl.start_time;
                        }
                        if ctrl.stop_time > max_stop {
                            max_stop = ctrl.stop_time;
                        }
                    }
                }
            }
        }

        if channels.is_empty() {
            return None;
        }

        Some(AnimationClip {
            name: "Default".to_string(),
            start_time: min_start,
            stop_time: max_stop,
            duration: (max_stop - min_start).max(0.0),
            cycle_type: 0,
            frequency: 1.0,
            channels,
        })
    }
'''

if 'pub fn from_transform_controllers' not in content:
    content = content.replace('pub fn from_controller_sequence', patch + '\n    pub fn from_controller_sequence')
    open('crates/fo3_render/src/animation.rs', 'w', encoding='utf-8').write(content)

