// Fallout 3 単体メッシュビューアー用 WGSL シェーダー

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
    alpha_test: u32,
    alpha_test_func: u32,
    alpha_threshold: f32,
    _padding: u32,
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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) tangent: vec3<f32>,
    @location(5) bitangent: vec3<f32>,
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

    let world_pos = model.world * vec4<f32>(in.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;

    // 法線・接線ベクトルのワールド空間変換 (回転・スケール)
    out.world_normal = normalize((model.world * vec4<f32>(in.normal, 0.0)).xyz);
    out.world_tangent = (model.world * vec4<f32>(in.tangent, 0.0)).xyz;
    out.world_bitangent = (model.world * vec4<f32>(in.bitangent, 0.0)).xyz;

    out.uv = in.uv;
    out.color = in.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let base_color = tex_color * in.color;

    // アルファテスト (NiAlphaProperty の動的カットアウト判定)
    // 参照元: references/nifskope/build/nif.xml:L1518, references/nifskope/src/gl/glproperty.cpp:L204-237
    if (model.alpha_test != 0u) {
        let alpha = base_color.a;
        let ref_val = model.alpha_threshold;
        var test_passed: bool = true;
        switch (model.alpha_test_func) {
            case 0u: { test_passed = true; } // TEST_ALWAYS
            case 1u: { test_passed = alpha < ref_val; } // TEST_LESS
            case 2u: { test_passed = abs(alpha - ref_val) < 0.0039; } // TEST_EQUAL (1/256 許容)
            case 3u: { test_passed = alpha <= ref_val; } // TEST_LESS_EQUAL
            case 4u: { test_passed = alpha > ref_val; } // TEST_GREATER (標準カットアウト)
            case 5u: { test_passed = abs(alpha - ref_val) >= 0.0039; } // TEST_NOT_EQUAL
            case 6u: { test_passed = alpha >= ref_val; } // TEST_GREATER_EQUAL
            case 7u: { test_passed = false; } // TEST_NEVER
            default: { test_passed = true; }
        }
        if (!test_passed) {
            discard;
        }
    }

    // 法線マップ (Slot 1: _n.dds) サンプリング
    // RGB: タンジェント空間法線 [-1, 1], A: グロス / スペキュラ強度
    // 参照元: references/openmw/components/nifosg/nifloader.cpp:L2401-2426
    let normal_sample = textureSample(t_normal, s_diffuse, in.uv);
    let gloss = normal_sample.a;

    // TBN 行列によるワールド空間法線の復元
    var N = normalize(in.world_normal);
    let t_len = length(in.world_tangent);
    let b_len = length(in.world_bitangent);
    if (t_len > 0.01 && b_len > 0.01) {
        let T = normalize(in.world_tangent);
        let B = normalize(in.world_bitangent);
        let tbn = mat3x3<f32>(T, B, N);
        let tangent_normal = normalize(normal_sample.rgb * 2.0 - 1.0);
        N = normalize(tbn * tangent_normal);
    }

    // カメラ視線方向ベクトル (ワールド空間)
    let V = normalize(camera.camera_pos.xyz - in.world_pos);

    // 1. 環境光 (Ambient)
    // 参照元: references/nifxml/nif.xml: NiAmbientLight, XCLL
    var diffuse_light = lighting.ambient_color.rgb;
    var specular_light = vec3<f32>(0.0);

    // 2. 指向性光 (Directional Light)
    // 参照元: Gamebryo 2.6 NiDirectionalLight
    let dir_len = length(lighting.dir_light_dir.xyz);
    if (dir_len > 0.001) {
        let L = normalize(lighting.dir_light_dir.xyz);
        let n_dot_l = max(dot(N, L), 0.0);
        diffuse_light += lighting.dir_light_color.rgb * n_dot_l;

        // Blinn-Phong スペキュラ反射
        if (n_dot_l > 0.0 && gloss > 0.01) {
            let H = normalize(L + V);
            let n_dot_h = max(dot(N, H), 0.0);
            let spec = pow(n_dot_h, 32.0) * gloss;
            specular_light += lighting.dir_light_color.rgb * spec;
        }
    }

    // 3. 配置点光源 (Point Lights, 最大 16 灯)
    // 参照元: Gamebryo 2.6 NiPointLight, references/openmw/files/shaders/lib/light/util.glsl:L45-97
    // 減衰式: (1.0 - (dist / radius))^falloff
    let num_lights = min(u32(lighting.fog_far_power.z), 16u);
    for (var i = 0u; i < num_lights; i = i + 1u) {
        let pl = lighting.point_lights[i];
        let to_light = pl.pos_radius.xyz - in.world_pos;
        let dist = length(to_light);
        let radius = pl.pos_radius.w;
        if (dist < radius && radius > 0.001) {
            let L = to_light / dist;
            let n_dot_l = max(dot(N, L), 0.0);
            let falloff = max(pl.color_falloff.w, 0.0);
            let norm_dist = clamp(dist / radius, 0.0, 1.0);
            let atten = pow(1.0 - norm_dist, falloff);
            let light_intensity = pl.color_falloff.rgb * (atten * n_dot_l);
            diffuse_light += light_intensity;

            // 点光源の Blinn-Phong スペキュラ反射
            if (n_dot_l > 0.0 && gloss > 0.01) {
                let H = normalize(L + V);
                let n_dot_h = max(dot(N, H), 0.0);
                let spec = pow(n_dot_h, 32.0) * (gloss * atten);
                specular_light += pl.color_falloff.rgb * spec;
            }
        }
    }

    // 拡散光と鏡面反射光を合成
    var lit_rgb = base_color.rgb * diffuse_light + specular_light;

    // 4. フォグ計算
    // 参照元: Gamebryo 2.6 NiFogProperty, references/nifxml/nif.xml: NiFogProperty
    // 計算式: factor = clamp((dist - near) / (far - near), 0.0, 1.0)^power
    let fog_near = lighting.fog_color_near.w;
    let fog_far = lighting.fog_far_power.x;
    let fog_power = lighting.fog_far_power.y;
    let fog_color = lighting.fog_color_near.rgb;

    if (fog_far > fog_near && fog_far > 0.0) {
        let cam_dist = length(camera.camera_pos.xyz - in.world_pos);
        let fog_factor = clamp((cam_dist - fog_near) / (fog_far - fog_near), 0.0, 1.0);
        let fog_blend = pow(fog_factor, fog_power);
        lit_rgb = mix(lit_rgb, fog_color, fog_blend);
    }

    return vec4<f32>(lit_rgb, base_color.a);
}
