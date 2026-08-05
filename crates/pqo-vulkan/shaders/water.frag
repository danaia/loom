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

// Angstrom-space copy of the authoritative Gate 1 model contract.
const float OH_DISTANCE_ANGSTROM = 0.9572;
const float HOH_ANGLE_DEGREES = 104.52;
const float OXYGEN_CHARGE_E = -0.834;
const float HYDROGEN_CHARGE_E = 0.417;

struct Surface {
    float distance;
    float material;
};

mat3 rotation_x(float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return mat3(1.0, 0.0, 0.0, 0.0, c, s, 0.0, -s, c);
}

mat3 rotation_y(float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return mat3(c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c);
}

float sphere_distance(vec3 point, vec3 center, float radius) {
    return length(point - center) - radius;
}

float capsule_distance(vec3 point, vec3 start, vec3 end, float radius) {
    vec3 segment = end - start;
    float along = clamp(dot(point - start, segment) / dot(segment, segment), 0.0, 1.0);
    return length(point - (start + along * segment)) - radius;
}

Surface nearer(Surface a, Surface b) {
    return a.distance < b.distance ? a : b;
}

void water_geometry(out vec3 oxygen, out vec3 hydrogen_1, out vec3 hydrogen_2) {
    float half_angle = radians(HOH_ANGLE_DEGREES * 0.5);
    float local_x = OH_DISTANCE_ANGSTROM * sin(half_angle);
    float local_z = OH_DISTANCE_ANGSTROM * cos(half_angle);
    oxygen = vec3(0.0);
    hydrogen_1 = vec3(local_x, 0.0, local_z);
    hydrogen_2 = vec3(-local_x, 0.0, local_z);
}

Surface molecule_surface(vec3 point) {
    vec3 oxygen;
    vec3 hydrogen_1;
    vec3 hydrogen_2;
    water_geometry(oxygen, hydrogen_1, hydrogen_2);
    Surface result = Surface(sphere_distance(point, oxygen, 0.55), 1.0);
    result = nearer(result, Surface(sphere_distance(point, hydrogen_1, 0.34), 2.0));
    result = nearer(result, Surface(sphere_distance(point, hydrogen_2, 0.34), 2.0));
    result = nearer(result, Surface(capsule_distance(point, oxygen, hydrogen_1, 0.115), 3.0));
    result = nearer(result, Surface(capsule_distance(point, oxygen, hydrogen_2, 0.115), 3.0));
    result = nearer(result, Surface(capsule_distance(
        point, vec3(0.0, 0.0, -1.42), vec3(0.0, 0.0, -0.78), 0.026), 4.0));
    result = nearer(result, Surface(capsule_distance(
        point, vec3(0.0, 0.0, -0.82), vec3(0.12, 0.0, -0.99), 0.026), 4.0));
    result = nearer(result, Surface(capsule_distance(
        point, vec3(0.0, 0.0, -0.82), vec3(-0.12, 0.0, -0.99), 0.026), 4.0));
    return result;
}

vec3 surface_normal(vec3 point) {
    const float epsilon = 0.0015;
    vec2 offset = vec2(epsilon, 0.0);
    return normalize(vec3(
        molecule_surface(point + offset.xyy).distance - molecule_surface(point - offset.xyy).distance,
        molecule_surface(point + offset.yxy).distance - molecule_surface(point - offset.yxy).distance,
        molecule_surface(point + offset.yyx).distance - molecule_surface(point - offset.yyx).distance));
}

float soft_shadow(vec3 origin, vec3 direction) {
    float result = 1.0;
    float travel = 0.025;
    for (int step = 0; step < 48; ++step) {
        float distance_to_surface = molecule_surface(origin + direction * travel).distance;
        result = min(result, 15.0 * distance_to_surface / travel);
        travel += clamp(distance_to_surface, 0.012, 0.11);
        if (distance_to_surface < 0.0008 || travel > 5.0) break;
    }
    return clamp(result, 0.18, 1.0);
}

vec3 material_color(float material, vec3 normal, vec3 view_direction, vec3 light_direction) {
    vec3 base;
    float roughness;
    if (material < 1.5) {
        base = vec3(0.88, 0.075, 0.095);
        roughness = 0.28;
    } else if (material < 2.5) {
        base = vec3(0.92, 0.95, 1.0);
        roughness = 0.20;
    } else if (material < 3.5) {
        base = vec3(0.54, 0.61, 0.71);
        roughness = 0.34;
    } else {
        base = vec3(0.20, 0.82, 1.0);
        roughness = 0.16;
    }
    float diffuse = max(dot(normal, light_direction), 0.0);
    vec3 half_vector = normalize(light_direction + view_direction);
    float specular = pow(max(dot(normal, half_vector), 0.0), mix(90.0, 24.0, roughness));
    float rim = pow(1.0 - max(dot(normal, view_direction), 0.0), 3.0);
    return base * (0.20 + 0.80 * diffuse) + vec3(specular * 0.65) + base * rim * 0.28;
}

vec3 background_color(vec2 screen, vec3 ray_direction) {
    float radial = length(screen);
    vec3 horizon = vec3(0.018, 0.040, 0.075);
    vec3 edge = vec3(0.003, 0.008, 0.020);
    vec3 background = mix(horizon, edge, smoothstep(0.15, 1.15, radial));
    float atmospheric_light = pow(max(ray_direction.y * 0.5 + 0.5, 0.0), 5.0);
    return background + atmospheric_light * vec3(0.008, 0.018, 0.032);
}

void main() {
    vec2 screen = vec2(uv.x * 1.55, -uv.y) / max(frame.zoom, 0.1);
    vec3 ray_origin = vec3(0.0, 0.15, 5.2);
    vec3 ray_direction = normalize(vec3(screen, -1.9));
    mat3 model_rotation = rotation_y(frame.yaw + 0.42) * rotation_x(frame.pitch - 0.12);
    mat3 inverse_rotation = transpose(model_rotation);
    ray_origin = inverse_rotation * ray_origin;
    ray_direction = inverse_rotation * ray_direction;

    vec3 color = background_color(screen, ray_direction);
    float travel = 0.0;
    float material = 0.0;
    float oxygen_proximity = 100.0;
    float hydrogen_proximity = 100.0;
    vec3 oxygen;
    vec3 hydrogen_1;
    vec3 hydrogen_2;
    water_geometry(oxygen, hydrogen_1, hydrogen_2);
    bool hit = false;
    for (int step = 0; step < 128; ++step) {
        vec3 point = ray_origin + ray_direction * travel;
        Surface surface = molecule_surface(point);
        oxygen_proximity = min(oxygen_proximity, abs(sphere_distance(point, oxygen, 0.63)));
        hydrogen_proximity = min(hydrogen_proximity, min(
            abs(sphere_distance(point, hydrogen_1, 0.40)),
            abs(sphere_distance(point, hydrogen_2, 0.40))));
        if (surface.distance < 0.0012) {
            material = surface.material;
            hit = true;
            break;
        }
        travel += surface.distance * 0.78;
        if (travel > 9.0) break;
    }

    float negative_charge_glow = exp(-oxygen_proximity * 18.0) * abs(OXYGEN_CHARGE_E);
    float positive_charge_glow = exp(-hydrogen_proximity * 22.0) * HYDROGEN_CHARGE_E;
    color += vec3(0.07, 0.32, 0.72) * negative_charge_glow * 0.42;
    color += vec3(0.92, 0.34, 0.08) * positive_charge_glow * 0.28;

    if (hit) {
        vec3 point = ray_origin + ray_direction * travel;
        vec3 normal = surface_normal(point);
        vec3 light_direction = normalize(vec3(-0.55, 0.85, 0.62));
        vec3 view_direction = -ray_direction;
        float shadow = soft_shadow(point + normal * 0.008, light_direction);
        vec3 shaded = material_color(material, normal, view_direction, light_direction);
        shaded *= mix(0.55, 1.0, shadow);
        float fog = 1.0 - exp(-travel * 0.012);
        color = mix(shaded, color, fog);
    }

    color *= 1.0 - 0.24 * smoothstep(0.45, 1.35, length(screen));
    color = pow(color, vec3(0.86));
    output_color = vec4(color, 1.0);
}
