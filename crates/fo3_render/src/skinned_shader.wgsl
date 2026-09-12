// Fallout 3 / Gamebryo 2.6 準拠 GPU ハードウェアスキニング WGSL シェーダー
// 参照元: `knowledge/actor_and_skin_mesh.md`, Gamebryo 2.6 `NiSkinPartition`

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

struct PointLight {
    pos_radius: vec4<f32>,
    color_falloff: vec4<f32>,
};

struct LightingUniform {
    ambient_color: vec4<f32>,
    dir_light_color: vec4<f32>,
    dir_light_dir: vec4<f32>,
    fog_color_near: vec4<f32>,
    fog_far_power: vec4<f32>,
    point_lights: array<PointLight, 16>,
};

struct ModelUniform {
    world: mat4x4<f32>,
    specular_color: vec4<f32>,
    emissive_color: vec4<f32>,
    tint_color: vec4<f32>,
    alpha_test: u32,
    alpha_test_func: u32,
    alpha_threshold: f32,
    has_glow_map: u32,
};

// パーティション内のボーン変換行列パレット（最大 80 ボーン）
struct BonePalette {
    matrices: array<mat4x4<f32>, 80>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> lighting: LightingUniform;

@group(1) @binding(0)
var<uniform> model: ModelUniform;

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var t_normal: texture_2d<f32>;
@group(2) @binding(3)
var t_glow: texture_2d<f32>;

@group(3) @binding(0)
var<uniform> bones: BonePalette;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) tangent: vec3<f32>,
    @location(5) bitangent: vec3<f32>,
    @location(6) bone_indices: vec4<u32>,
    @location(7) bone_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) world_tangent: vec3<f32>,
    @location(5) world_bitangent: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Gamebryo 2.6 ハードウェアスキニング: 最大 4 ボーンの線形重み付けブレンド
    let skin_mat = in.bone_weights.x * bones.matrices[in.bone_indices.x]
                 + in.bone_weights.y * bones.matrices[in.bone_indices.y]
                 + in.bone_weights.z * bones.matrices[in.bone_indices.z]
                 + in.bone_weights.w * bones.matrices[in.bone_indices.w];

    let skinned_pos = skin_mat * vec4<f32>(in.position, 1.0);
    let world_pos = model.world * skinned_pos;
    out.world_pos = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;

    // 法線・接線ベクトルのスキニング変換
    let skinned_normal = (skin_mat * vec4<f32>(in.normal, 0.0)).xyz;
    let skinned_tangent = (skin_mat * vec4<f32>(in.tangent, 0.0)).xyz;
    let skinned_bitangent = (skin_mat * vec4<f32>(in.bitangent, 0.0)).xyz;

    out.world_normal = normalize((model.world * vec4<f32>(skinned_normal, 0.0)).xyz);
    out.world_tangent = (model.world * vec4<f32>(skinned_tangent, 0.0)).xyz;
    out.world_bitangent = (model.world * vec4<f32>(skinned_bitangent, 0.0)).xyz;

    out.uv = in.uv;
    out.color = in.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_color = textureSample(t_diffuse, s_diffuse, in.uv);

    // NiAlphaProperty アルファテスト (Gamebryo 2.6 仕様)
    if (model.alpha_test != 0u) {
        let alpha = tex_color.a;
        let thresh = model.alpha_threshold;
        var test_passed: bool = true;
        switch (model.alpha_test_func) {
            case 0u: { test_passed = true; } // TEST_ALWAYS
            case 1u: { test_passed = alpha < thresh; } // TEST_LESS
            case 2u: { test_passed = abs(alpha - thresh) < 0.0039; } // TEST_EQUAL
            case 3u: { test_passed = alpha <= thresh; } // TEST_LESS_EQUAL
            case 4u: { test_passed = alpha > thresh; } // TEST_GREATER
            case 5u: { test_passed = abs(alpha - thresh) >= 0.0039; } // TEST_NOT_EQUAL
            case 6u: { test_passed = alpha >= thresh; } // TEST_GREATER_EQUAL
            case 7u: { test_passed = false; } // TEST_NEVER
            default: { test_passed = true; }
        }
        if (!test_passed) {
            discard;
        }
    }

    // 法線マップ (接空間 -> ワールド空間)
    let n_sample = textureSample(t_normal, s_diffuse, in.uv);
    let t_norm = normalize(in.world_tangent);
    let b_norm = normalize(in.world_bitangent);
    let g_norm = normalize(in.world_normal);

    var n_local = n_sample.xyz * 2.0 - 1.0;
    n_local = vec3<f32>(n_local.x, -n_local.y, n_local.z);
    let N = normalize(t_norm * n_local.x + b_norm * n_local.y + g_norm * n_local.z);

    let gloss = n_sample.a;

    // ディレクショナルライト (太陽光 / 主光源)
    let L = normalize(-lighting.dir_light_dir.xyz);
    let V = normalize(camera.camera_pos.xyz - in.world_pos);
    let H = normalize(L + V);

    let NdotL = max(dot(N, L), 0.0);
    let diff = lighting.dir_light_color.rgb * NdotL;

    // Blinn-Phong スペキュラ (Fallout 3: 20.0 * gloss^2)
    let NdotH = max(dot(N, H), 0.0);
    let spec_power = max(1.0, 32.0 * gloss);
    let spec_intensity = pow(NdotH, spec_power) * gloss;
    let spec = lighting.dir_light_color.rgb * model.specular_color.rgb * spec_intensity;

    // 点光源の累積
    var point_diffuse = vec3<f32>(0.0);
    var point_specular = vec3<f32>(0.0);

    for (var i = 0u; i < 16u; i = i + 1u) {
        let p_pos = lighting.point_lights[i].pos_radius.xyz;
        let radius = lighting.point_lights[i].pos_radius.w;
        if (radius <= 0.0) {
            continue;
        }
        let p_color = lighting.point_lights[i].color_falloff.rgb;

        let to_light = p_pos - in.world_pos;
        let dist = length(to_light);
        if (dist >= radius) {
            continue;
        }

        let p_dir = to_light / dist;
        let atten = 1.0 - (dist / radius);
        let smooth_atten = atten * atten;

        let p_ndotl = max(dot(N, p_dir), 0.0);
        point_diffuse += p_color * p_ndotl * smooth_atten;

        let p_h = normalize(p_dir + V);
        let p_ndoth = max(dot(N, p_h), 0.0);
        let p_spec_int = pow(p_ndoth, spec_power) * gloss;
        point_specular += p_color * model.specular_color.rgb * p_spec_int * smooth_atten;
    }

    // 環境光 + ディフューズ + スペキュラ + エミッシブ合成
    let ambient = lighting.ambient_color.rgb;
    let total_light = ambient + diff + point_diffuse;
    let tinted_tex = tex_color.rgb * model.tint_color.rgb;
    var final_color = tinted_tex * in.color.rgb * total_light + spec + point_specular + model.emissive_color.rgb;

    // グローマップ (自己発光テクスチャ)
    if (model.has_glow_map != 0u) {
        let glow_color = textureSample(t_glow, s_diffuse, in.uv);
        final_color += glow_color.rgb * tinted_tex;
    }

    // 距離フォグ (Fallout 3 指数フォグ)
    let fog_near = lighting.fog_color_near.w;
    let fog_far = lighting.fog_far_power.x;
    let fog_power = lighting.fog_far_power.y;
    let cam_dist = length(camera.camera_pos.xyz - in.world_pos);

    if (cam_dist > fog_near && fog_far > fog_near) {
        let fog_factor = clamp((cam_dist - fog_near) / (fog_far - fog_near), 0.0, 1.0);
        let fog_amount = pow(fog_factor, fog_power);
        final_color = mix(final_color, lighting.fog_color_near.rgb, fog_amount);
    }

    return vec4<f32>(final_color, tex_color.a);
}
