#version 460

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 output_color;
layout(push_constant) uniform Frame {
    float time;
    float growth;
    float anisotropy;
    float temperature;
    float damage;
    float show_field;
    float show_particles;
    float particle_count;
    float yaw;
    float pitch;
    float zoom;
    float smart_lod;
    float lod_bias;
} frame;

mat2 rotate2(float angle) {
    float c = cos(angle), s = sin(angle);
    return mat2(c, -s, s, c);
}

float crystal_shape(vec3 point, float growth) {
    point.yz = rotate2(frame.pitch) * point.yz;
    point.xz = rotate2(frame.yaw) * point.xz;
    point.xz = rotate2(0.39) * point.xz;
    vec3 absolute_point = abs(point);
    float cube = max(absolute_point.x, max(absolute_point.y, absolute_point.z));
    float octahedron = (absolute_point.x + absolute_point.y + absolute_point.z) * 0.58;
    return max(cube, mix(octahedron, cube, frame.anisotropy)) - growth;
}

vec3 crystal_normal(vec3 point, float growth) {
    const float epsilon = 0.002;
    return normalize(vec3(
        crystal_shape(point + vec3(epsilon, 0, 0), growth) - crystal_shape(point - vec3(epsilon, 0, 0), growth),
        crystal_shape(point + vec3(0, epsilon, 0), growth) - crystal_shape(point - vec3(0, epsilon, 0), growth),
        crystal_shape(point + vec3(0, 0, epsilon), growth) - crystal_shape(point - vec3(0, 0, epsilon), growth)
    ));
}

void main() {
    vec2 point_uv = vec2(uv.x * 1.35, -uv.y);
    vec3 ray_origin = vec3(0.0, 0.0, 2.8 / frame.zoom);
    vec3 ray_direction = normalize(vec3(point_uv, -1.7));
    float growth = frame.growth;
    float distance_along_ray = 0.0;
    bool hit = false;
    vec3 point = vec3(0.0);
    for (int step = 0; step < 112; ++step) {
        point = ray_origin + ray_direction * distance_along_ray;
        float distance_to_surface = crystal_shape(point, growth);
        if (distance_to_surface < 0.001) { hit = true; break; }
        distance_along_ray += max(distance_to_surface * 0.58, 0.004);
        if (distance_along_ray > 6.0) break;
    }
    vec3 background = mix(vec3(0.006, 0.012, 0.018), vec3(0.018, 0.055, 0.075), max(0.0, 1.0 - length(point_uv)) * 0.5);
    if (!hit) { output_color = vec4(background, 1.0); return; }
    vec3 normal = crystal_normal(point, growth);
    vec3 light = normalize(vec3(-0.5, 0.8, 0.65));
    vec3 view = -ray_direction;
    float diffuse = 0.2 + 0.8 * max(dot(normal, light), 0.0);
    float fresnel = pow(1.0 - abs(dot(normal, view)), 3.0);
    float glint = pow(max(dot(reflect(-light, normal), view), 0.0), 30.0);
    vec3 cold = mix(vec3(0.12, 0.58, 0.88), vec3(0.48, 0.93, 1.0), frame.temperature);
    vec3 color = mix(cold, vec3(1.0, 0.04, 0.015), frame.damage * 0.88) * diffuse;
    color = mix(color, vec3(0.72, 0.96, 1.0), fresnel * 0.5) + glint;
    float lattice = 0.5 + 0.5 * sin((point.x * 73.0 + point.y * 41.0 - point.z * 57.0) * 8.0);
    color *= 0.82 + 0.18 * lattice;
    float density = pow(max(frame.particle_count, 1.0), 1.0 / 3.0);
    float automatic_scale = frame.zoom < 0.75 ? 0.25 : frame.zoom < 1.2 ? 0.5 : 1.0;
    density *= mix(1.0, automatic_scale, frame.smart_lod) * exp2(frame.lod_bias);
    vec3 cell = fract((point + vec3(1.5)) * density) - 0.5;
    float particle = 1.0 - smoothstep(0.11, 0.24, length(cell));
    if (frame.show_field < 0.5 && (frame.show_particles < 0.5 || particle < 0.12)) {
        output_color = vec4(background, 1.0);
        return;
    }
    color *= frame.show_field;
    color += vec3(0.78, 0.96, 1.0) * particle * frame.show_particles * (0.55 + 0.45 * fresnel);
    output_color = vec4(pow(color, vec3(0.82)), 1.0);
}
