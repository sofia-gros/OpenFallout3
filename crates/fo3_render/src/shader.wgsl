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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) world_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos4 = model.world * vec4<f32>(in.position, 1.0);
    out.clip_position = camera.view_proj * world_pos4;
    out.world_pos = world_pos4.xyz;

    // 法線を行列の上位 3x3 で変換 (スケールが等方であると仮定)
    let normal_matrix = mat3x3<f32>(
        model.world[0].xyz,
        model.world[1].xyz,
        model.world[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;
    out.color = in.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    let base_color = tex_color * in.color;

    // アルファテスト (透過テクスチャの切り抜き)
    if (base_color.a < 0.33) {
        discard;
    }

    // 1. 環境光 (Ambient)
    // 参照元: references/nifxml/nif.xml: NiAmbientLight, XCLL
    var total_light = lighting.ambient_color.rgb;

    // 2. 指向性光 (Directional Light)
    // 参照元: Gamebryo 2.6 NiDirectionalLight
    let dir_len = length(lighting.dir_light_dir.xyz);
    if (dir_len > 0.001) {
        let l_dir = normalize(lighting.dir_light_dir.xyz);
        let n_dot_l = max(dot(in.world_normal, l_dir), 0.0);
        total_light += lighting.dir_light_color.rgb * n_dot_l;
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
            let l_dir = to_light / dist;
            let n_dot_l = max(dot(in.world_normal, l_dir), 0.0);
            let falloff = max(pl.color_falloff.w, 0.0);
            let norm_dist = clamp(dist / radius, 0.0, 1.0);
            let atten = pow(1.0 - norm_dist, falloff);
            total_light += pl.color_falloff.rgb * (n_dot_l * atten);
        }
    }

    var lit_rgb = base_color.rgb * total_light;

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
