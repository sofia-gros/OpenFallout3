// Fallout 3 単体メッシュビューアー用 WGSL シェーダー

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

struct ModelUniform {
    world: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

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

    // 平行光源 (斜め上方向からのキーライト)
    let light_dir = normalize(vec3<f32>(0.5, 0.5, 0.8));
    let n_dot_l = max(dot(in.world_normal, light_dir), 0.0);

    // 補助フィルライト (反対側からの弱い光)
    let fill_dir = normalize(vec3<f32>(-0.4, -0.4, 0.2));
    let fill_light = max(dot(in.world_normal, fill_dir), 0.0) * 0.25;

    // 環境光 (アンビエント)
    let ambient = 0.35;
    let lighting = ambient + n_dot_l * 0.65 + fill_light;

    let base_color = tex_color * in.color;
    // アルファテスト (透過テクスチャの切り抜き)
    if (base_color.a < 0.33) {
        discard;
    }

    let final_rgb = base_color.rgb * lighting;

    return vec4<f32>(final_rgb, base_color.a);
}
