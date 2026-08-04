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
    float instance_count;
} frame;

mat2 rotate2(float angle) {
    float c = cos(angle), s = sin(angle);
    return mat2(c, -s, s, c);
}

vec3 orient_world(vec3 point) {
    point.yz = rotate2(frame.pitch) * point.yz;
    point.xz = rotate2(frame.yaw) * point.xz;
    return point;
}

float single_crystal_shape(vec3 point, float growth) {
    point.xz = rotate2(0.39) * point.xz;
    vec3 absolute_point = abs(point);
    float cube = max(absolute_point.x, max(absolute_point.y, absolute_point.z));
    float octahedron = (absolute_point.x + absolute_point.y + absolute_point.z) * 0.58;
    return max(cube, mix(octahedron, cube, frame.anisotropy)) - growth;
}

vec3 grid_candidate(vec3 point, vec2 minimum_cell, vec2 maximum_cell, vec2 grid_size, float spacing) {
    vec2 cell = clamp(round(point.xy / spacing + (grid_size - 1.0) * 0.5), minimum_cell, maximum_cell);
    vec2 center = (cell - (grid_size - 1.0) * 0.5) * spacing;
    return point - vec3(center, 0.0);
}

float scene_shape(vec3 point, float growth, out vec3 local_point) {
    point = orient_world(point);
    float count = clamp(round(frame.instance_count), 1.0, 1000.0);
    float columns = ceil(sqrt(count));
    float full_rows = floor(count / columns);
    float final_row = count - full_rows * columns;
    float rows = full_rows + min(final_row, 1.0);
    float spacing = max(1.55, growth * 2.35);
    float best = 1e10;

    if (full_rows > 0.0) {
        vec3 candidate = grid_candidate(point, vec2(0.0), vec2(columns - 1.0, full_rows - 1.0), vec2(columns, rows), spacing);
        float surface = single_crystal_shape(candidate, growth);
        if (surface < best) { best = surface; local_point = candidate; }
    }
    if (final_row > 0.0) {
        vec3 candidate = grid_candidate(point, vec2(0.0, full_rows), vec2(final_row - 1.0, full_rows), vec2(columns, rows), spacing);
        float surface = single_crystal_shape(candidate, growth);
        if (surface < best) { best = surface; local_point = candidate; }
    }
    return best;
}

vec3 crystal_normal(vec3 point, float growth) {
    const float epsilon = 0.002;
    vec3 ignored;
    return normalize(vec3(
        scene_shape(point + vec3(epsilon, 0, 0), growth, ignored) - scene_shape(point - vec3(epsilon, 0, 0), growth, ignored),
        scene_shape(point + vec3(0, epsilon, 0), growth, ignored) - scene_shape(point - vec3(0, epsilon, 0), growth, ignored),
        scene_shape(point + vec3(0, 0, epsilon), growth, ignored) - scene_shape(point - vec3(0, 0, epsilon), growth, ignored)
    ));
}

void main() {
    vec2 point_uv = vec2(uv.x * 1.35, -uv.y);
    float growth = frame.growth;
    float count = clamp(round(frame.instance_count), 1.0, 1000.0);
    float columns = ceil(sqrt(count));
    float rows = ceil(count / columns);
    float scene_radius = growth + 0.5 * max(columns - 1.0, rows - 1.0) * max(1.55, growth * 2.35);
    vec3 ray_origin = vec3(0.0, 0.0, (2.8 + scene_radius * 1.45) / frame.zoom);
    vec3 ray_direction = normalize(vec3(point_uv, -1.7));
    float distance_along_ray = 0.0;
    bool hit = false;
    vec3 point = vec3(0.0);
    vec3 local_point = vec3(0.0);
    for (int step = 0; step < 112; ++step) {
        point = ray_origin + ray_direction * distance_along_ray;
        float distance_to_surface = scene_shape(point, growth, local_point);
        if (distance_to_surface < 0.001) { hit = true; break; }
        distance_along_ray += max(distance_to_surface * 0.58, 0.004);
        if (distance_along_ray > 8.0 + scene_radius * 4.0) break;
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
    float lattice = 0.5 + 0.5 * sin((local_point.x * 73.0 + local_point.y * 41.0 - local_point.z * 57.0) * 8.0);
    color *= 0.82 + 0.18 * lattice;
    float density = pow(max(frame.particle_count, 1.0), 1.0 / 3.0);
    float automatic_scale = frame.zoom < 0.75 ? 0.25 : frame.zoom < 1.2 ? 0.5 : 1.0;
    density *= mix(1.0, automatic_scale, frame.smart_lod) * exp2(frame.lod_bias);
    vec3 cell = fract((local_point + vec3(1.5)) * density) - 0.5;
    float particle = 1.0 - smoothstep(0.11, 0.24, length(cell));
    if (frame.show_field < 0.5 && (frame.show_particles < 0.5 || particle < 0.12)) {
        output_color = vec4(background, 1.0);
        return;
    }
    color *= frame.show_field;
    color += vec3(0.78, 0.96, 1.0) * particle * frame.show_particles * (0.55 + 0.45 * fresnel);
    output_color = vec4(pow(color, vec3(0.82)), 1.0);
}
