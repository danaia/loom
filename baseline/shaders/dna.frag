#version 460

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 output_color;

// This compact ABI is shared with the native Vulkan shell. In this scene the
// legacy slots are deliberately interpreted as named sandbox controls.
layout(push_constant) uniform Frame {
    float time;
    float scale_level;
    float thermal_amplitude;
    float bend;
    float separation;
    float show_orbital;
    float show_bases;
    float base_pair_count;
    float yaw;
    float pitch;
    float zoom;
    float smart_lod;
    float lod_bias;
    float motion;
} frame;

const float PI = 3.141592653589793;
const float B_DNA_RISE_NM = 0.34;
const float B_DNA_TWIST = 2.0 * PI / 10.5;
const int MAX_BASE_PAIRS = 24;

mat2 rotate2(float angle) {
    float c = cos(angle), s = sin(angle);
    return mat2(c, -s, s, c);
}

vec3 model_point(vec3 point) {
    point.yz = rotate2(frame.pitch) * point.yz;
    point.xz = rotate2(frame.yaw) * point.xz;
    return point;
}

float sphere_distance(vec3 point, vec3 center, float radius) {
    return length(point - center) - radius;
}

float capsule_distance(vec3 point, vec3 start, vec3 end, float radius) {
    vec3 segment = end - start;
    float fraction = clamp(dot(point - start, segment) / max(dot(segment, segment), 1e-5), 0.0, 1.0);
    return length(point - (start + fraction * segment)) - radius;
}

int primary_base(int index) {
    // Drew-Dickerson: CGCGAATTCGCG, repeated only when the exploratory length
    // control exceeds the validated 12-base-pair reference.
    int i = index % 12;
    if (i == 4 || i == 5) return 0; // adenine
    if (i == 0 || i == 2 || i == 8 || i == 10) return 1; // cytosine
    if (i == 1 || i == 3 || i == 9 || i == 11) return 2; // guanine
    return 3; // thymine
}

int complement_base(int base) {
    return base == 0 ? 3 : base == 3 ? 0 : base == 1 ? 2 : 1;
}

vec3 base_color(int base) {
    if (base == 0) return vec3(0.20, 0.86, 0.52); // A
    if (base == 1) return vec3(0.18, 0.62, 1.00); // C
    if (base == 2) return vec3(1.00, 0.64, 0.14); // G
    return vec3(0.95, 0.28, 0.48);                // T
}

vec3 centerline(float index, float count, float phase) {
    float z = (index - 0.5 * (count - 1.0)) * B_DNA_RISE_NM;
    float half_length = max(0.5 * count * B_DNA_RISE_NM, 0.5);
    float normalized = z / half_length;
    float thermal = frame.thermal_amplitude * 0.10;
    float animated = frame.time * frame.motion;
    return vec3(
        frame.bend * 0.72 * (normalized * normalized - 0.34)
            + thermal * sin(1.7 * z + animated + phase),
        thermal * 0.7 * cos(1.3 * z - animated * 0.73 + phase),
        z);
}

void nucleotide_sites(int index, float count, out vec3 backbone_a, out vec3 backbone_b,
                      out vec3 base_a, out vec3 base_b, out vec3 center) {
    float fi = float(index);
    float phase = fi * B_DNA_TWIST;
    center = centerline(fi, count, phase);
    float opening = frame.separation * exp(-0.16 * pow(fi - 0.5 * (count - 1.0), 2.0));
    float radius = 1.0 + opening * 0.72;
    vec2 radial = vec2(cos(phase), sin(phase));
    backbone_a = center + vec3(radial * radius, 0.0);
    backbone_b = center - vec3(radial * radius, 0.0);
    base_a = center + vec3(radial * (0.53 + opening * 0.48), 0.0);
    base_b = center - vec3(radial * (0.53 + opening * 0.48), 0.0);
}

vec2 dna_distance(vec3 world_point) {
    vec3 point = model_point(world_point);
    int count = int(clamp(round(frame.base_pair_count), 2.0, float(MAX_BASE_PAIRS)));
    int level = int(clamp(round(frame.scale_level), 1.0, 4.0));
    float best = 1e6;
    float material = 0.0;

    for (int index = 0; index < MAX_BASE_PAIRS; ++index) {
        if (index >= count) break;
        vec3 backbone_a, backbone_b, base_a, base_b, center;
        nucleotide_sites(index, float(count), backbone_a, backbone_b, base_a, base_b, center);

        if (level <= 3) {
            float radius = level == 1 ? 0.13 : 0.16;
            float da = sphere_distance(point, backbone_a, radius);
            float db = sphere_distance(point, backbone_b, radius);
            if (da < best) { best = da; material = 1.0; }
            if (db < best) { best = db; material = 2.0; }
        }

        if (level <= 3 && frame.show_bases > 0.5) {
            float base_radius = level == 1 ? 0.17 : 0.21;
            if (level <= 2) {
                float da = sphere_distance(point, base_a, base_radius);
                float db = sphere_distance(point, base_b, base_radius);
                if (da < best) { best = da; material = 10.0 + float(primary_base(index)); }
                if (db < best) { best = db; material = 10.0 + float(complement_base(primary_base(index))); }
            }
            float pair = capsule_distance(point, base_a, base_b, level == 3 ? 0.052 : level == 1 ? 0.045 : 0.075);
            if (pair < best) { best = pair; material = 5.0; }
        }

        if (index + 1 < count) {
            vec3 next_a, next_b, next_base_a, next_base_b, next_center;
            nucleotide_sites(index + 1, float(count), next_a, next_b, next_base_a, next_base_b, next_center);
            if (level <= 3) {
                float tube = level == 3 ? 0.12 : 0.075;
                float da = capsule_distance(point, backbone_a, next_a, tube);
                float db = capsule_distance(point, backbone_b, next_b, tube);
                if (da < best) { best = da; material = 1.0; }
                if (db < best) { best = db; material = 2.0; }
            } else {
                float rod = capsule_distance(point, center, next_center, 0.48);
                if (rod < best) { best = rod; material = 6.0; }
            }
        }
    }
    return vec2(best, material);
}

vec3 dna_material(float material) {
    if (material >= 10.0) return base_color(int(material - 10.0));
    if (material < 1.5) return vec3(0.16, 0.78, 1.0);
    if (material < 2.5) return vec3(0.72, 0.30, 1.0);
    if (material < 5.5) return vec3(0.90, 0.92, 0.98);
    return vec3(0.16, 0.58, 0.78);
}

vec3 dna_normal(vec3 point) {
    const float epsilon = 0.003;
    return normalize(vec3(
        dna_distance(point + vec3(epsilon, 0, 0)).x - dna_distance(point - vec3(epsilon, 0, 0)).x,
        dna_distance(point + vec3(0, epsilon, 0)).x - dna_distance(point - vec3(0, epsilon, 0)).x,
        dna_distance(point + vec3(0, 0, epsilon)).x - dna_distance(point - vec3(0, 0, epsilon)).x));
}

vec3 background_color(vec2 screen) {
    return mix(vec3(0.0015, 0.003, 0.009), vec3(0.009, 0.026, 0.052),
               max(0.0, 1.0 - length(screen) * 0.55));
}

void render_hydrogen(vec2 screen, vec3 background) {
    vec3 ray_origin = vec3(0.0, 0.0, 18.0);
    vec3 ray_direction = normalize(vec3(screen, -1.8));
    float projection = dot(ray_origin, ray_direction);
    float discriminant = projection * projection - (dot(ray_origin, ray_origin) - 144.0);
    if (discriminant < 0.0 || frame.show_orbital < 0.5) {
        output_color = vec4(background, 1.0);
        return;
    }
    float root = sqrt(discriminant);
    float near_t = max(-projection - root, 0.0);
    float far_t = -projection + root;
    const int samples = 160;
    float step_length = (far_t - near_t) / float(samples);
    float transmittance = 1.0;
    vec3 radiance = vec3(0.0);
    for (int sample_index = 0; sample_index < samples; ++sample_index) {
        vec3 position_bohr = ray_origin + ray_direction * (near_t + (float(sample_index) + 0.5) * step_length);
        float radius_bohr = length(position_bohr);
        float density = exp(-2.0 * radius_bohr) / PI * (1.0 - smoothstep(9.0, 12.0, radius_bohr));
        float opacity = 1.0 - exp(-density * step_length * 11.0);
        vec3 cloud = mix(vec3(0.18, 0.06, 0.72), vec3(0.18, 0.86, 1.0),
                         pow(max(density * PI, 0.0), 0.16));
        radiance += transmittance * opacity * cloud;
        transmittance *= 1.0 - opacity;
        if (transmittance < 0.002) break;
    }
    output_color = vec4(vec3(1.0) - exp(-radiance * 1.35) + transmittance * background, 1.0);
}

void main() {
    vec2 screen = vec2(uv.x * 1.48, -uv.y) / max(frame.zoom, 0.1);
    vec3 background = background_color(screen);
    if (frame.scale_level < 0.5) {
        render_hydrogen(screen, background);
        return;
    }

    int count = int(clamp(round(frame.base_pair_count), 2.0, float(MAX_BASE_PAIRS)));
    float half_length = 0.5 * float(count) * B_DNA_RISE_NM;
    vec3 ray_origin = vec3(0.0, 0.0, 7.2 + half_length * 0.35);
    vec3 ray_direction = normalize(vec3(screen, -1.65));
    float distance_along_ray = 0.0;
    bool hit = false;
    vec2 sample_value = vec2(0.0);
    vec3 point = vec3(0.0);
    int step_limit = frame.smart_lod > 0.5 ? int(clamp(78.0 + frame.lod_bias * 12.0, 54.0, 112.0)) : 112;
    for (int step = 0; step < 112; ++step) {
        if (step >= step_limit) break;
        point = ray_origin + ray_direction * distance_along_ray;
        sample_value = dna_distance(point);
        if (sample_value.x < 0.003) { hit = true; break; }
        distance_along_ray += max(sample_value.x * 0.72, 0.007);
        if (distance_along_ray > 18.0) break;
    }
    if (!hit) {
        output_color = vec4(background, 1.0);
        return;
    }

    vec3 normal = dna_normal(point);
    vec3 light = normalize(vec3(-0.45, 0.72, 0.58));
    float diffuse = 0.22 + 0.78 * max(dot(normal, light), 0.0);
    float rim = pow(1.0 - abs(dot(normal, -ray_direction)), 3.0);
    vec3 color = dna_material(sample_value.y) * diffuse + vec3(0.28, 0.62, 0.82) * rim * 0.38;
    output_color = vec4(pow(color, vec3(0.86)), 1.0);
}
