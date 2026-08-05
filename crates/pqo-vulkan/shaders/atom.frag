#version 460

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 output_color;

// Kept layout-compatible with the existing Vulkan control block. A hydrogen
// 1s state is rotationally invariant, so yaw and pitch intentionally do not
// deform or rotate the probability cloud.
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

bool intersect_sphere(vec3 ray_origin, vec3 ray_direction, float radius, out float near_t, out float far_t) {
    float projection = dot(ray_origin, ray_direction);
    float discriminant = projection * projection - (dot(ray_origin, ray_origin) - radius * radius);
    if (discriminant < 0.0) return false;
    float root = sqrt(discriminant);
    near_t = -projection - root;
    far_t = -projection + root;
    return far_t > 0.0;
}

float hydrogen_1s_density(vec3 position_bohr) {
    // |psi_100|^2 in atomic units: exp(-2r) / pi.
    return exp(-2.0 * length(position_bohr)) / 3.141592653589793;
}

void main() {
    vec2 screen = vec2(uv.x * 1.55, -uv.y) / max(frame.zoom, 0.1);
    vec3 ray_origin = vec3(0.0, 0.0, 18.0);
    vec3 ray_direction = normalize(vec3(screen, -1.8));
    vec3 background = mix(
        vec3(0.0015, 0.003, 0.009),
        vec3(0.008, 0.018, 0.045),
        max(0.0, 1.0 - length(screen) * 0.55));

    float near_t;
    float far_t;
    if (!intersect_sphere(ray_origin, ray_direction, 12.0, near_t, far_t)) {
        output_color = vec4(background, 1.0);
        return;
    }

    near_t = max(near_t, 0.0);
    const int sample_count = 192;
    float step_length = (far_t - near_t) / float(sample_count);
    float transmittance = 1.0;
    vec3 radiance = vec3(0.0);
    for (int sample_index = 0; sample_index < sample_count; ++sample_index) {
        float distance_along_ray = near_t + (float(sample_index) + 0.5) * step_length;
        vec3 position_bohr = ray_origin + ray_direction * distance_along_ray;
        float radius_bohr = length(position_bohr);
        float boundary_fade = 1.0 - smoothstep(9.0, 12.0, radius_bohr);
        float density = hydrogen_1s_density(position_bohr) * boundary_fade;
        float density_ratio = density * 3.141592653589793;
        float opacity = 1.0 - exp(-density * step_length * 11.0);
        vec3 tail_color = vec3(0.16, 0.06, 0.72);
        vec3 core_color = vec3(0.18, 0.86, 1.0);
        vec3 cloud_color = mix(tail_color, core_color, pow(density_ratio, 0.16));
        radiance += transmittance * opacity * cloud_color;
        transmittance *= 1.0 - opacity;
        if (transmittance < 0.002) break;
    }

    // A proton is about 1.6e-5 Bohr radii across and cannot be resolved beside
    // the cloud. This visible marker is deliberately exaggerated; it is not the
    // nuclear length scale used by the quantum model.
    float nucleus_near;
    float nucleus_far;
    if (intersect_sphere(ray_origin, ray_direction, 0.075, nucleus_near, nucleus_far)
        && nucleus_near >= near_t && nucleus_near <= far_t) {
        vec3 nucleus_point = ray_origin + ray_direction * nucleus_near;
        vec3 nucleus_normal = normalize(nucleus_point);
        float lighting = 0.32 + 0.68 * max(dot(nucleus_normal, normalize(vec3(-0.4, 0.7, 0.6))), 0.0);
        vec3 nucleus_color = vec3(1.0, 0.42, 0.035) * lighting * 1.8;
        radiance = mix(radiance, nucleus_color, 0.78);
        transmittance *= 0.04;
    }

    vec3 color = vec3(1.0) - exp(-radiance * 1.35);
    color += transmittance * background;
    output_color = vec4(color, 1.0);
}
