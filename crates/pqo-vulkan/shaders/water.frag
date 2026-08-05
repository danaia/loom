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
    float pointer_x;
    float pointer_y;
    float pointer_down;
    float splash_time;
    float viewport_aspect;
    float sphere_mass_g;
    float sphere_count;
    float sphere_drop_time;
} frame;

// Angstrom-space copy of the authoritative rigid three-site model contract.
const float OH_DISTANCE_ANGSTROM = 0.9572;
const float HOH_ANGLE_DEGREES = 104.52;
const float OXYGEN_CHARGE_E = -0.834;
const float HYDROGEN_CHARGE_E = 0.417;
const float PI = 3.141592653589793;

struct Surface { float distance; float material; };

mat3 rotation_x(float angle) {
    float c = cos(angle), s = sin(angle);
    return mat3(1,0,0, 0,c,s, 0,-s,c);
}

mat3 rotation_y(float angle) {
    float c = cos(angle), s = sin(angle);
    return mat3(c,0,-s, 0,1,0, s,0,c);
}

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float sphere_distance(vec3 p, vec3 center, float radius) {
    return length(p - center) - radius;
}

float capsule_distance(vec3 p, vec3 a, vec3 b, float radius) {
    vec3 segment = b - a;
    float along = clamp(dot(p - a, segment) / dot(segment, segment), 0.0, 1.0);
    return length(p - (a + along * segment)) - radius;
}

float torus_distance(vec3 p, vec2 radii) {
    vec2 q = vec2(length(p.xz) - radii.x, p.y);
    return length(q) - radii.y;
}

float capped_cylinder_distance(vec3 p, float radius, float half_height) {
    vec2 d = abs(vec2(length(p.xz), p.y)) - vec2(radius, half_height);
    return min(max(d.x, d.y), 0.0) + length(max(d, 0.0));
}

Surface nearer(Surface a, Surface b) { return a.distance < b.distance ? a : b; }

vec2 disturbance_center() {
    return vec2(frame.pointer_x * 1.42, frame.pointer_y * 0.72 - 0.08);
}

float disturbance_age() { return max(frame.time - frame.splash_time, 0.0); }

float dropped_sphere_y(float age) {
    if (age < 0.0) return 4.0;
    const float impact_time = 0.57;
    if (age < impact_time) return 1.43 - 2.70 * age * age;
    float density_ratio = frame.sphere_mass_g / 14.1;
    float target = density_ratio < 1.0
        ? 0.43 + 0.075 * (1.0 - density_ratio)
        : mix(0.36, -1.16, clamp((density_ratio - 1.0) / 5.5, 0.0, 1.0));
    float wet_age = age - impact_time;
    float frequency = 4.6 / sqrt(max(density_ratio, 0.18));
    return target + (0.55 - target) * exp(-wet_age * 1.18) * cos(wet_age * frequency);
}

float water_height(vec2 xz) {
    vec2 center = disturbance_center();
    float radius = length(xz - center);
    float age = disturbance_age();
    float impulse = exp(-age * 1.45);
    float ring = sin(radius * 19.0 - age * 10.5) * exp(-radius * 2.4) * impulse;
    float contact = frame.pointer_down * exp(-radius * radius * 65.0);
    float ambient = 0.006 * sin(xz.x * 7.0 + frame.time * 0.7)
        * cos(xz.y * 5.0 - frame.time * 0.5);
    return 0.42 + ambient + ring * 0.048 - contact * 0.035;
}

Surface glass_scene(vec3 p) {
    Surface result = Surface(100.0, 0.0);
    float radial = length(p.xz);
    float side = max(abs(radial - 1.34) - 0.025, abs(p.y + 0.34) - 1.14);
    result = nearer(result, Surface(side, 1.0));
    result = nearer(result, Surface(torus_distance(p - vec3(0, 0.82, 0), vec2(1.34, 0.035)), 1.0));
    float base = max(abs(p.y + 1.46) - 0.075, radial - 1.34);
    result = nearer(result, Surface(base, 1.0));

    float h = water_height(p.xz);
    // Only the liquid boundary is marched. Treating the whole volume as opaque
    // would hide the free surface and destroy the sense of refraction.
    float water_side = max(abs(radial - 1.275) - 0.012, max(-1.34 - p.y, p.y - h));
    float water_surface = max(abs(p.y - h) - 0.006, radial - 1.268);
    result = nearer(result, Surface(water_side, 2.0));
    result = nearer(result, Surface(water_surface, 6.0));

    float age = disturbance_age();
    float splash = exp(-age * 2.2) * smoothstep(1.15, 0.0, age);
    vec2 c = disturbance_center();
    if (splash > 0.002) {
        vec3 local = p - vec3(c.x, 0.44, c.y);
        float crown_radius = 0.16 + age * 0.16;
        float crown = abs(length(local.xz) - crown_radius) - 0.022;
        crown = max(crown, abs(local.y - splash * 0.23) - 0.18 * splash);
        result = nearer(result, Surface(crown, 2.0));
        for (int i = 0; i < 7; ++i) {
            float fi = float(i);
            float angle = fi * 2.399;
            vec3 droplet = vec3(c.x, 0.48, c.y)
                + vec3(cos(angle), 0.0, sin(angle)) * (0.08 + 0.026 * fi)
                + vec3(0, age * (0.70 + 0.055 * fi) - 1.65 * age * age, 0);
            result = nearer(result, Surface(sphere_distance(p, droplet, 0.022 + 0.005 * mod(fi, 2.0)), 2.0));
        }
    }
    return result;
}

vec3 glass_normal(vec3 p) {
    const float e = 0.002;
    vec2 o = vec2(e, 0);
    return normalize(vec3(
        glass_scene(p + o.xyy).distance - glass_scene(p - o.xyy).distance,
        glass_scene(p + o.yxy).distance - glass_scene(p - o.yxy).distance,
        glass_scene(p + o.yyx).distance - glass_scene(p - o.yyx).distance));
}

vec3 studio_background(vec2 screen, vec3 ray_direction) {
    float horizon = smoothstep(-0.48, -0.06, ray_direction.y);
    vec3 charcoal = mix(vec3(0.0008, 0.0011, 0.0014), vec3(0.006, 0.008, 0.010), horizon);
    float top_light = pow(max(ray_direction.y * 0.5 + 0.5, 0.0), 7.0);
    float vignette = smoothstep(1.35, 0.15, length(screen * vec2(0.72, 1.0)));
    return charcoal + vec3(0.012, 0.020, 0.024) * top_light + 0.0025 * vignette;
}

vec3 render_glass(vec2 screen) {
    vec3 ray_origin = vec3(0.0, 0.05, 4.6);
    vec3 ray_direction = normalize(vec3(screen.x, screen.y, -2.05));
    mat3 orbit = rotation_y(frame.yaw) * rotation_x(frame.pitch * 0.35);
    ray_origin = transpose(orbit) * ray_origin;
    ray_direction = transpose(orbit) * ray_direction;
    vec3 color = studio_background(screen, ray_direction);

    // Ground plane and a soft physically plausible contact shadow.
    if (ray_direction.y < -0.001) {
        float t_ground = (-1.54 - ray_origin.y) / ray_direction.y;
        if (t_ground > 0.0) {
            vec3 gp = ray_origin + ray_direction * t_ground;
            float grid = hash21(floor(gp.xz * 36.0));
            float shadow = exp(-dot(gp.xz, gp.xz) * 0.42);
            color = vec3(0.007 + grid * 0.003) * (1.0 - shadow * 0.64);
            color += vec3(0.018, 0.028, 0.032) * pow(max(0.0, 1.0 - length(gp.xz) * 0.42), 8.0);
        }
    }

    float travel = 0.0;
    float material = 0.0;
    bool hit = false;
    for (int step = 0; step < 150; ++step) {
        vec3 point = ray_origin + ray_direction * travel;
        Surface surface = glass_scene(point);
        if (surface.distance < 0.0014) {
            material = surface.material;
            hit = true;
            break;
        }
        travel += max(surface.distance * 0.62, 0.002);
        if (travel > 9.0) break;
    }
    if (hit) {
        vec3 point = ray_origin + ray_direction * travel;
        vec3 normal = glass_normal(point);
        vec3 view = -ray_direction;
        vec3 light = normalize(vec3(-0.62, 0.88, 0.48));
        float fresnel = pow(1.0 - max(dot(normal, view), 0.0), material < 1.5 ? 3.2 : 4.5);
        float specular = pow(max(dot(reflect(-light, normal), view), 0.0), material < 1.5 ? 150.0 : 90.0);
        if (material < 1.5) {
            vec3 reflected = studio_background(screen + normal.xy * 0.09, reflect(ray_direction, normal));
            color = mix(color * vec3(0.88, 0.96, 1.03), reflected + vec3(0.36) * specular, 0.12 + 0.72 * fresnel);
            color += vec3(0.55, 0.68, 0.73) * (0.08 + specular) * (0.35 + fresnel);
        } else if (material < 5.5) {
            float depth = clamp((0.43 - point.y) * 0.32, 0.0, 1.0);
            vec3 water_tint = mix(vec3(0.075, 0.115, 0.125), vec3(0.16, 0.31, 0.36), fresnel);
            color = mix(color * vec3(0.58, 0.79, 0.84), water_tint, 0.10 + depth * 0.10);
            color += vec3(0.75, 0.92, 0.96) * specular * 1.8;
            color += vec3(0.10, 0.33, 0.42) * fresnel * 0.34;
        } else {
            vec3 reflected = studio_background(screen + normal.xy * 0.12, reflect(ray_direction, normal));
            color = mix(color * vec3(0.62, 0.82, 0.88), reflected, 0.26 + fresnel * 0.52);
            color += vec3(0.48, 0.82, 0.90) * (0.10 + fresnel * 0.45);
            color += vec3(0.92, 0.98, 1.0) * specular * 2.2;
        }
    }

    // Caustic ellipse beneath the glass.
    float caustic = exp(-pow(length(screen - vec2(0.03, -0.69)) * 4.2, 2.0));
    color += vec3(0.08, 0.17, 0.19) * caustic * 0.22;

    float cup_mask = (1.0 - smoothstep(0.52, 0.57, abs(screen.x)))
        * smoothstep(-0.72, -0.62, screen.y)
        * (1.0 - smoothstep(0.37, 0.44, screen.y));
    color = mix(color, color * vec3(0.38, 0.50, 0.54), cup_mask * 0.58);

    // The glass wall is transparent, so composite the free-surface reflection
    // after the first boundary hit. Its projected ellipse carries the same
    // damped capillary impulse as the 3D height function.
    vec2 center = disturbance_center();
    float age = disturbance_age();
    float projected_x = center.x * 0.72;
    float ripple_radius = abs(screen.x - projected_x);
    float ripple = sin(ripple_radius * 28.0 - age * 10.5)
        * exp(-ripple_radius * 2.8) * exp(-age * 1.45);
    float sphere_impact_age = frame.time - frame.sphere_drop_time - 0.57;
    float sphere_wave = 0.0;
    for (int i = 0; i < 5; ++i) {
        if (float(i) >= frame.sphere_count) break;
        float sphere_x = (float(i) - (frame.sphere_count - 1.0) * 0.5) * 0.19;
        float distance_from_impact = abs(screen.x - sphere_x);
        sphere_wave += sin(distance_from_impact * 35.0 - sphere_impact_age * 12.0)
            * exp(-distance_from_impact * 4.0)
            * exp(-max(sphere_impact_age, 0.0) * 1.25)
            * step(0.0, sphere_impact_age)
            * clamp(frame.sphere_mass_g / 14.1, 0.25, 4.0);
    }
    ripple += sphere_wave * 0.55;
    float surface_y = 0.10 + ripple * 0.020;
    vec2 ellipse = vec2(screen.x / 0.55, (screen.y - surface_y) / 0.105);
    float ellipse_radius = length(ellipse);
    float surface_fill = 1.0 - smoothstep(0.91, 1.0, ellipse_radius);
    float meniscus = 1.0 - smoothstep(0.012, 0.030, abs(ellipse_radius - 0.96));
    float ring_pattern = 0.5 + 0.5 * sin(ripple_radius * 34.0 - age * 11.0);
    color = mix(color, color * vec3(0.66, 0.84, 0.88) + vec3(0.025, 0.075, 0.09), surface_fill * 0.20);
    color += vec3(0.32, 0.62, 0.68) * meniscus * 0.34;
    color += vec3(0.16, 0.44, 0.52) * surface_fill * ring_pattern
        * exp(-ripple_radius * 3.0) * exp(-age * 1.5) * 0.22;

    float splash_strength = exp(-age * 2.2) * (1.0 - smoothstep(0.70, 1.15, age));
    vec2 splash_q = vec2(screen.x - projected_x, screen.y - surface_y);
    float crown_radius = 0.055 + age * 0.16;
    float crown = 1.0 - smoothstep(
        0.010,
        0.023,
        abs(length(vec2(splash_q.x, splash_q.y * 1.45)) - crown_radius));
    crown *= smoothstep(-0.006, 0.035, splash_q.y);
    float droplets = 0.0;
    for (int i = 0; i < 6; ++i) {
        float fi = float(i);
        float side = fi - 2.5;
        vec2 drop_center = vec2(
            projected_x + side * (0.027 + age * 0.010),
            surface_y + age * (0.38 + 0.035 * mod(fi, 3.0)) - age * age * 0.34
                + 0.045 + abs(side) * 0.008);
        droplets += 1.0 - smoothstep(0.008, 0.017, length(screen - drop_center));
    }
    color += vec3(0.52, 0.82, 0.88) * (crown * 0.72 + droplets) * splash_strength;

    // Identically sized spheres make mass visible through buoyancy instead of
    // conflating mass with radius. Their displaced-water mass is 14.1 g.
    float sphere_age = frame.time - frame.sphere_drop_time;
    float world_y = dropped_sphere_y(sphere_age);
    for (int i = 0; i < 5; ++i) {
        if (float(i) >= frame.sphere_count) break;
        float sphere_x = (float(i) - (frame.sphere_count - 1.0) * 0.5) * 0.19;
        vec2 sphere_center = vec2(sphere_x, world_y * 0.47 - 0.02);
        vec2 local = (screen - sphere_center) / 0.074;
        float radius_squared = dot(local, local);
        if (radius_squared < 1.0) {
            vec3 normal = vec3(local, sqrt(max(1.0 - radius_squared, 0.0)));
            vec3 light = normalize(vec3(-0.65, 0.8, 0.9));
            float diffuse = max(dot(normal, light), 0.0);
            float specular = pow(max(dot(reflect(-light, normal), vec3(0,0,1)), 0.0), 56.0);
            vec3 brass = mix(vec3(0.22, 0.12, 0.025), vec3(0.92, 0.58, 0.12), diffuse);
            vec3 sphere_color = brass + vec3(0.9, 0.74, 0.42) * specular;
            float submerged = smoothstep(surface_y + 0.025, surface_y - 0.06, screen.y);
            sphere_color = mix(sphere_color, sphere_color * vec3(0.48, 0.72, 0.78), submerged * 0.58);
            float edge = smoothstep(1.0, 0.94, radius_squared);
            color = mix(color, sphere_color, edge);
        }
    }
    return color;
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
    vec3 oxygen, hydrogen_1, hydrogen_2;
    water_geometry(oxygen, hydrogen_1, hydrogen_2);
    Surface result = Surface(sphere_distance(point, oxygen, 0.55), 3.0);
    result = nearer(result, Surface(sphere_distance(point, hydrogen_1, 0.34), 4.0));
    result = nearer(result, Surface(sphere_distance(point, hydrogen_2, 0.34), 4.0));
    result = nearer(result, Surface(capsule_distance(point, oxygen, hydrogen_1, 0.115), 5.0));
    result = nearer(result, Surface(capsule_distance(point, oxygen, hydrogen_2, 0.115), 5.0));
    return result;
}

vec3 molecule_normal(vec3 p) {
    const float e = 0.0015;
    vec2 o = vec2(e, 0);
    return normalize(vec3(
        molecule_surface(p + o.xyy).distance - molecule_surface(p - o.xyy).distance,
        molecule_surface(p + o.yxy).distance - molecule_surface(p - o.yxy).distance,
        molecule_surface(p + o.yyx).distance - molecule_surface(p - o.yyx).distance));
}

vec3 render_molecule(vec2 screen) {
    vec3 ray_origin = vec3(0.0, 0.12, 5.1);
    vec3 ray_direction = normalize(vec3(screen, -1.95));
    mat3 model_rotation = rotation_y(frame.yaw + 0.42) * rotation_x(frame.pitch - 0.12);
    ray_origin = transpose(model_rotation) * ray_origin;
    ray_direction = transpose(model_rotation) * ray_direction;
    vec3 color = studio_background(screen, ray_direction) * vec3(0.72, 0.86, 0.92);

    // Coarse molecular packets recede as one molecule becomes explicit.
    for (int i = 0; i < 24; ++i) {
        float fi = float(i);
        vec2 cell = vec2(hash21(vec2(fi, 2.1)), hash21(vec2(fi, 8.7))) * 2.0 - 1.0;
        float radius = 0.005 + 0.010 * hash21(vec2(fi, 4.2));
        float haze = smoothstep(radius, 0.0, length(screen - cell * vec2(1.4, 0.82)));
        color += vec3(0.12, 0.24, 0.28) * haze * 0.35;
    }

    float travel = 0.0;
    float material = 0.0;
    bool hit = false;
    for (int step = 0; step < 128; ++step) {
        Surface surface = molecule_surface(ray_origin + ray_direction * travel);
        if (surface.distance < 0.0012) { material = surface.material; hit = true; break; }
        travel += surface.distance * 0.78;
        if (travel > 9.0) break;
    }
    if (hit) {
        vec3 point = ray_origin + ray_direction * travel;
        vec3 normal = molecule_normal(point);
        vec3 light = normalize(vec3(-0.55, 0.85, 0.62));
        vec3 view = -ray_direction;
        float diffuse = max(dot(normal, light), 0.0);
        float specular = pow(max(dot(normalize(light + view), normal), 0.0), 72.0);
        vec3 base = material < 3.5 ? vec3(0.76, 0.035, 0.045)
            : material < 4.5 ? vec3(0.90, 0.94, 0.97) : vec3(0.42, 0.52, 0.57);
        color = base * (0.18 + 0.82 * diffuse) + vec3(specular * 0.8);
        color += base * pow(1.0 - max(dot(normal, view), 0.0), 3.0) * 0.32;
    }
    return color;
}

float line_mask(vec2 p, vec2 a, vec2 b, float width) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return smoothstep(width, width * 0.35, length(pa - ba * h));
}

vec3 add_interface(vec3 color, vec2 screen, float molecular_mix) {
    vec3 ink = vec3(0.68, 0.89, 0.93);
    float rail = line_mask(screen, vec2(-1.42, -0.46), vec2(-1.42, 0.46), 0.004);
    for (int i = 0; i <= 12; ++i) {
        float y = mix(-0.46, 0.46, float(i) / 12.0);
        float tick = line_mask(screen, vec2(-1.44, y), vec2(-1.40 + (i == 6 ? 0.035 : 0.0), y), 0.003);
        rail = max(rail, tick);
    }
    float marker_y = mix(-0.43, 0.43, clamp(log2(frame.zoom / 0.48) / log2(12.0 / 0.48), 0.0, 1.0));
    float marker = 1.0 - smoothstep(0.015, 0.024, length(screen - vec2(-1.42, marker_y)));
    color += ink * (rail * 0.36 + marker * 0.95);

    // Minimal editorial rules: code-native geometry, no dashboard panels.
    float title_rule = line_mask(screen, vec2(-1.42, 0.82), vec2(-1.06, 0.82), 0.003);
    float info_rule = line_mask(screen, vec2(-1.42, -0.74), vec2(-1.06, -0.74), 0.003);
    color += ink * (title_rule * 0.55 + info_rule * 0.35);
    if (molecular_mix > 0.55) {
        float bracket = line_mask(screen, vec2(0.63, -0.28), vec2(1.10, -0.28), 0.003)
            + line_mask(screen, vec2(0.63, -0.30), vec2(0.63, -0.26), 0.003)
            + line_mask(screen, vec2(1.10, -0.30), vec2(1.10, -0.26), 0.003);
        color += ink * bracket * molecular_mix;
    }
    return color;
}

void main() {
    vec2 screen = vec2(uv.x * frame.viewport_aspect, -uv.y);
    float logarithmic_zoom = log2(max(frame.zoom, 0.48) / 0.48);
    float molecular_mix = smoothstep(3.15, 4.15, logarithmic_zoom);
    vec2 macro_screen = screen / mix(0.72, 1.42, smoothstep(0.0, 3.0, logarithmic_zoom));
    vec2 molecule_screen = screen / mix(0.72, 1.55, smoothstep(3.15, 4.65, logarithmic_zoom));
    vec3 macro_color = render_glass(macro_screen);
    vec3 molecule_color = render_molecule(molecule_screen);
    vec3 color = mix(macro_color, molecule_color, molecular_mix);
    color = add_interface(color, screen, molecular_mix);
    color *= 1.0 - 0.20 * smoothstep(0.72, 1.75, length(screen * vec2(0.68, 1.0)));
    color = pow(max(color, 0.0), vec3(0.84));
    output_color = vec4(color, 1.0);
}
