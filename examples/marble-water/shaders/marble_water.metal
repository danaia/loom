#include <metal_stdlib>
using namespace metal;

struct MarbleVertexOut {
    float4 position [[position]];
    float2 local;
    float4 color;
    float depth;
    float ui;
    float activity;
    float density;
    float plane;
    float amplification;
    float reset;
};

vertex MarbleVertexOut marble_water_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device float4 *colors [[buffer(0)]],
    const device packed_float3 *positions [[buffer(1)]],
    const device float *radii [[buffer(2)]],
    const device float *plane_scales [[buffer(3)]])
{
    constexpr float2 corners[6] = {
        float2(-1.0, -1.0), float2( 1.0, -1.0), float2(-1.0,  1.0),
        float2(-1.0,  1.0), float2( 1.0, -1.0), float2( 1.0,  1.0)
    };
    const float radius = radii[instance_id];
    const float plane_scale = plane_scales[instance_id];
    if (radius < 0.0) {
        MarbleVertexOut out;
        out.position = float4(
            float2(-0.79, 0.76) + corners[vertex_id] * float2(0.180, 0.170),
            0.0,
            1.0
        );
        out.local = corners[vertex_id];
        out.color = colors[instance_id];
        out.depth = 0.0;
        out.ui = 1.0;
        out.density = clamp(float3(positions[instance_id]).x, 0.0, 1.0);
        out.activity = clamp(float3(positions[instance_id]).y, 0.0, 1.0);
        out.plane = clamp((plane_scale - 1.0) * 0.5, 0.0, 1.0);
        out.amplification = clamp(colors[instance_id].b, 0.0, 1.0);
        out.reset = clamp(float3(positions[instance_id]).z, 0.0, 1.0);
        return out;
    }

    const float camera_scale = 1.0 + (max(plane_scale, 1.0) - 1.0) * 0.72;
    const float3 camera = float3(0.0, 2.10, 3.55) * camera_scale;
    const float3 target = float3(0.0, 0.02, 0.0);
    const float3 forward = normalize(target - camera);
    const float3 right = normalize(cross(forward, float3(0.0, 1.0, 0.0)));
    const float3 camera_up = cross(right, forward);
    const float3 relative = float3(positions[instance_id]) - camera;
    const float depth = max(dot(relative, forward), 0.05);
    const float focal = 1.85;
    const float2 projected = float2(
        dot(relative, right) * focal / (depth * 1.333333),
        dot(relative, camera_up) * focal / depth
    );
    const float marble_scale = radius > 0.05 ? camera_scale : 1.0;
    const float visual_radius = radius * marble_scale * focal / depth;

    MarbleVertexOut out;
    out.position = float4(projected + corners[vertex_id] * visual_radius,
                          clamp(depth / 5.0, 0.0, 1.0), 1.0);
    out.local = corners[vertex_id];
    out.color = colors[instance_id];
    out.depth = depth;
    out.ui = 0.0;
    out.activity = 0.0;
    out.density = 0.0;
    out.plane = 0.0;
    out.amplification = 0.0;
    out.reset = 0.0;
    return out;
}

inline float rounded_box_sdf(float2 point, float2 half_size, float radius)
{
    const float2 offset = abs(point) - half_size + radius;
    return min(max(offset.x, offset.y), 0.0)
        + length(max(offset, 0.0))
        - radius;
}

inline float line_mask(float2 point, float2 start, float2 end, float width)
{
    const float2 segment = end - start;
    const float projection = clamp(
        dot(point - start, segment) / max(dot(segment, segment), 1e-5),
        0.0,
        1.0
    );
    const float distance = length(point - (start + segment * projection));
    return 1.0 - smoothstep(width, width + 0.012, distance);
}

inline float key_mask(float2 point, float2 center)
{
    return 1.0 - smoothstep(
        0.0,
        0.025,
        rounded_box_sdf(point - center, float2(0.205, 0.215), 0.055)
    );
}

inline float key_border(float2 point, float2 center)
{
    const float distance = abs(
        rounded_box_sdf(point - center, float2(0.205, 0.215), 0.055)
    );
    return 1.0 - smoothstep(0.018, 0.038, distance);
}

inline float glyph_w(float2 point, float2 center)
{
    const float2 p = point - center;
    return max(
        max(line_mask(p, float2(-0.105, 0.085), float2(-0.055, -0.090), 0.021),
            line_mask(p, float2(-0.055, -0.090), float2(0.000, 0.020), 0.021)),
        max(line_mask(p, float2(0.000, 0.020), float2(0.055, -0.090), 0.021),
            line_mask(p, float2(0.055, -0.090), float2(0.105, 0.085), 0.021))
    );
}

inline float glyph_a(float2 point, float2 center)
{
    const float2 p = point - center;
    return max(
        max(line_mask(p, float2(-0.100, -0.090), float2(0.000, 0.095), 0.021),
            line_mask(p, float2(0.000, 0.095), float2(0.100, -0.090), 0.021)),
        line_mask(p, float2(-0.058, -0.005), float2(0.058, -0.005), 0.021)
    );
}

inline float glyph_s(float2 point, float2 center)
{
    const float2 p = point - center;
    float mask = line_mask(p, float2(-0.080, 0.090), float2(0.090, 0.090), 0.021);
    mask = max(mask, line_mask(p, float2(-0.090, 0.080), float2(-0.090, 0.005), 0.021));
    mask = max(mask, line_mask(p, float2(-0.080, 0.000), float2(0.080, 0.000), 0.021));
    mask = max(mask, line_mask(p, float2(0.090, -0.005), float2(0.090, -0.080), 0.021));
    return max(mask, line_mask(p, float2(-0.090, -0.090), float2(0.080, -0.090), 0.021));
}

inline float glyph_d(float2 point, float2 center)
{
    const float2 p = point - center;
    float mask = line_mask(p, float2(-0.085, -0.095), float2(-0.085, 0.095), 0.021);
    mask = max(mask, line_mask(p, float2(-0.075, 0.090), float2(0.045, 0.090), 0.021));
    mask = max(mask, line_mask(p, float2(0.045, 0.090), float2(0.095, 0.045), 0.021));
    mask = max(mask, line_mask(p, float2(0.095, 0.045), float2(0.095, -0.045), 0.021));
    mask = max(mask, line_mask(p, float2(0.095, -0.045), float2(0.045, -0.090), 0.021));
    return max(mask, line_mask(p, float2(0.045, -0.090), float2(-0.075, -0.090), 0.021));
}

inline float small_letter_mask(float2 point, float2 center, uint letter)
{
    const float2 p = (point - center) / 0.14;
    float mask = 0.0;
    if (letter == 0u) { // F
        mask = line_mask(p, float2(-0.34, -0.50), float2(-0.34, 0.50), 0.085);
        mask = max(mask, line_mask(p, float2(-0.34, 0.50), float2(0.34, 0.50), 0.085));
        mask = max(mask, line_mask(p, float2(-0.34, 0.02), float2(0.24, 0.02), 0.085));
    } else if (letter == 1u) { // P
        mask = line_mask(p, float2(-0.34, -0.50), float2(-0.34, 0.50), 0.085);
        mask = max(mask, line_mask(p, float2(-0.34, 0.50), float2(0.22, 0.50), 0.085));
        mask = max(mask, line_mask(p, float2(-0.34, 0.02), float2(0.22, 0.02), 0.085));
        mask = max(mask, line_mask(p, float2(0.28, 0.44), float2(0.28, 0.08), 0.085));
    } else if (letter == 2u) { // S
        mask = line_mask(p, float2(-0.28, 0.50), float2(0.32, 0.50), 0.085);
        mask = max(mask, line_mask(p, float2(-0.34, 0.44), float2(-0.34, 0.06), 0.085));
        mask = max(mask, line_mask(p, float2(-0.28, 0.00), float2(0.28, 0.00), 0.085));
        mask = max(mask, line_mask(p, float2(0.34, -0.06), float2(0.34, -0.44), 0.085));
        mask = max(mask, line_mask(p, float2(-0.32, -0.50), float2(0.28, -0.50), 0.085));
    } else if (letter == 3u) { // M
        mask = line_mask(p, float2(-0.34, -0.50), float2(-0.34, 0.50), 0.085);
        mask = max(mask, line_mask(p, float2(-0.34, 0.50), float2(0.00, 0.08), 0.085));
        mask = max(mask, line_mask(p, float2(0.00, 0.08), float2(0.34, 0.50), 0.085));
        mask = max(mask, line_mask(p, float2(0.34, 0.50), float2(0.34, -0.50), 0.085));
    } else { // B
        mask = line_mask(p, float2(-0.34, -0.50), float2(-0.34, 0.50), 0.085);
        mask = max(mask, line_mask(p, float2(-0.34, 0.50), float2(0.20, 0.50), 0.085));
        mask = max(mask, line_mask(p, float2(-0.34, 0.00), float2(0.22, 0.00), 0.085));
        mask = max(mask, line_mask(p, float2(-0.34, -0.50), float2(0.20, -0.50), 0.085));
        mask = max(mask, line_mask(p, float2(0.28, 0.43), float2(0.28, 0.08), 0.085));
        mask = max(mask, line_mask(p, float2(0.28, -0.08), float2(0.28, -0.43), 0.085));
    }
    return mask;
}

inline float digit_mask(float2 point, float2 center, uint digit)
{
    constexpr ushort segment_masks[10] = {
        0x3f, 0x06, 0x5b, 0x4f, 0x66,
        0x6d, 0x7d, 0x07, 0x7f, 0x6f
    };
    const float2 p = (point - center) / 0.16;
    const ushort segments = segment_masks[min(digit, 9u)];
    float mask = 0.0;
    if ((segments & 0x01) != 0) mask = max(mask, line_mask(p, float2(-0.28, 0.50), float2(0.28, 0.50), 0.080));
    if ((segments & 0x02) != 0) mask = max(mask, line_mask(p, float2(0.32, 0.45), float2(0.32, 0.05), 0.080));
    if ((segments & 0x04) != 0) mask = max(mask, line_mask(p, float2(0.32, -0.05), float2(0.32, -0.45), 0.080));
    if ((segments & 0x08) != 0) mask = max(mask, line_mask(p, float2(-0.28, -0.50), float2(0.28, -0.50), 0.080));
    if ((segments & 0x10) != 0) mask = max(mask, line_mask(p, float2(-0.32, -0.05), float2(-0.32, -0.45), 0.080));
    if ((segments & 0x20) != 0) mask = max(mask, line_mask(p, float2(-0.32, 0.45), float2(-0.32, 0.05), 0.080));
    if ((segments & 0x40) != 0) mask = max(mask, line_mask(p, float2(-0.28, 0.00), float2(0.28, 0.00), 0.080));
    return mask;
}

fragment float4 marble_water_pipeline_fragment(MarbleVertexOut in [[stage_in]])
{
    if (in.ui > 0.5) {
        const float panel = 1.0 - smoothstep(
            0.0,
            0.028,
            rounded_box_sdf(in.local, float2(0.96, 0.93), 0.12)
        );
        if (panel <= 0.0) discard_fragment();

        float3 color = float3(0.018, 0.040, 0.075);
        float alpha = 0.87 * panel;

        constexpr float2 particle_start = float2(-0.650, 0.700);
        constexpr float2 particle_end = float2(0.180, 0.700);
        const float particle_track = line_mask(
            in.local,
            particle_start,
            particle_end,
            0.030
        );
        color = mix(color, float3(0.10, 0.22, 0.34), particle_track);

        const float2 particle_thumb = mix(particle_start, particle_end, in.density);
        const float particle_fill = line_mask(
            in.local,
            particle_start,
            particle_thumb,
            0.035
        );
        const float particle_thumb_mask = 1.0 - smoothstep(
            0.085,
            0.115,
            length(in.local - particle_thumb)
        );
        const float3 activity_color = mix(
            float3(0.08, 0.58, 0.86),
            float3(0.20, 0.92, 1.0),
            in.activity
        );
        color = mix(
            color,
            activity_color,
            max(particle_fill, particle_thumb_mask)
        );

        constexpr float2 plane_start = float2(-0.650, 0.340);
        constexpr float2 plane_end = float2(0.180, 0.340);
        const float plane_track = line_mask(
            in.local,
            plane_start,
            plane_end,
            0.030
        );
        color = mix(color, float3(0.18, 0.14, 0.32), plane_track);

        const float2 plane_thumb = mix(plane_start, plane_end, in.plane);
        const float plane_fill = line_mask(
            in.local,
            plane_start,
            plane_thumb,
            0.035
        );
        const float plane_thumb_mask = 1.0 - smoothstep(
            0.085,
            0.115,
            length(in.local - plane_thumb)
        );
        color = mix(
            color,
            float3(0.56, 0.46, 1.0),
            max(plane_fill, plane_thumb_mask)
        );

        constexpr float2 amplification_start = float2(-0.650, -0.020);
        constexpr float2 amplification_end = float2(0.180, -0.020);
        const float amplification_track = line_mask(
            in.local,
            amplification_start,
            amplification_end,
            0.030
        );
        color = mix(color, float3(0.30, 0.17, 0.08), amplification_track);

        const float2 amplification_thumb = mix(
            amplification_start,
            amplification_end,
            in.amplification
        );
        const float amplification_fill = line_mask(
            in.local,
            amplification_start,
            amplification_thumb,
            0.035
        );
        const float amplification_thumb_mask = 1.0 - smoothstep(
            0.085,
            0.115,
            length(in.local - amplification_thumb)
        );
        color = mix(
            color,
            float3(1.0, 0.52, 0.12),
            max(amplification_fill, amplification_thumb_mask)
        );

        constexpr float2 reset_center = float2(0.670, 0.700);
        const float reset_button = 1.0 - smoothstep(
            0.190,
            0.225,
            length(in.local - reset_center)
        );
        color = mix(
            color,
            mix(float3(0.07, 0.14, 0.22), float3(0.14, 0.58, 0.76), in.reset),
            reset_button
        );
        const float2 reset_point = in.local - reset_center;
        const float reset_ring = 1.0 - smoothstep(
            0.020,
            0.040,
            abs(length(reset_point) - 0.105)
        );
        float reset_arrow = line_mask(
            reset_point,
            float2(0.025, 0.095),
            float2(0.105, 0.095),
            0.022
        );
        reset_arrow = max(reset_arrow, line_mask(
            reset_point,
            float2(0.105, 0.095),
            float2(0.075, 0.045),
            0.022
        ));
        color = mix(
            color,
            float3(0.76, 0.93, 1.0),
            max(reset_ring * reset_button, reset_arrow * reset_button)
        );

        const float separator = 1.0 - smoothstep(
            0.012,
            0.030,
            abs(in.local.y + 0.230)
        );
        color = mix(color, float3(0.13, 0.24, 0.37), separator * 0.65);

        constexpr float2 w_center = float2(-0.380, -0.410);
        constexpr float2 a_center = float2(-0.650, -0.750);
        constexpr float2 s_center = float2(-0.380, -0.750);
        constexpr float2 d_center = float2(-0.110, -0.750);
        float keys = key_mask(in.local, w_center);
        keys = max(keys, key_mask(in.local, a_center));
        keys = max(keys, key_mask(in.local, s_center));
        keys = max(keys, key_mask(in.local, d_center));
        color = mix(color, float3(0.055, 0.115, 0.185), keys * 0.94);

        float borders = key_border(in.local, w_center);
        borders = max(borders, key_border(in.local, a_center));
        borders = max(borders, key_border(in.local, s_center));
        borders = max(borders, key_border(in.local, d_center));
        color = mix(color, float3(0.18, 0.47, 0.67), borders * 0.82);

        float glyphs = glyph_w(in.local, w_center);
        glyphs = max(glyphs, glyph_a(in.local, a_center));
        glyphs = max(glyphs, glyph_s(in.local, s_center));
        glyphs = max(glyphs, glyph_d(in.local, d_center));
        color = mix(color, float3(0.82, 0.94, 1.0), glyphs);

        const uint fps = uint(clamp(round(in.color.r), 0.0, 999.0));
        const uint gpu_mb = uint(clamp(round(in.color.g), 0.0, 999.0));
        float labels = small_letter_mask(in.local, float2(0.235, -0.405), 0u);
        labels = max(labels, small_letter_mask(in.local, float2(0.315, -0.405), 1u));
        labels = max(labels, small_letter_mask(in.local, float2(0.395, -0.405), 2u));
        labels = max(labels, small_letter_mask(in.local, float2(0.275, -0.745), 3u));
        labels = max(labels, small_letter_mask(in.local, float2(0.380, -0.745), 4u));
        color = mix(color, float3(0.35, 0.58, 0.72), labels);

        float fps_digits = digit_mask(in.local, float2(0.515, -0.405), (fps / 100u) % 10u);
        fps_digits = max(fps_digits, digit_mask(in.local, float2(0.665, -0.405), (fps / 10u) % 10u));
        fps_digits = max(fps_digits, digit_mask(in.local, float2(0.815, -0.405), fps % 10u));
        float memory_digits = digit_mask(in.local, float2(0.515, -0.745), (gpu_mb / 100u) % 10u);
        memory_digits = max(memory_digits, digit_mask(in.local, float2(0.665, -0.745), (gpu_mb / 10u) % 10u));
        memory_digits = max(memory_digits, digit_mask(in.local, float2(0.815, -0.745), gpu_mb % 10u));
        color = mix(color, float3(0.74, 0.94, 1.0), max(fps_digits, memory_digits));
        return float4(color, alpha);
    }

    const float radius_squared = dot(in.local, in.local);
    if (radius_squared > 1.0) discard_fragment();
    const float z = sqrt(max(1.0 - radius_squared, 0.0));
    const float3 normal = normalize(float3(in.local.x, -in.local.y, z));
    const float3 light = normalize(float3(-0.35, 0.65, 0.80));
    const float diffuse = 0.30 + 0.70 * max(dot(normal, light), 0.0);
    const float highlight = pow(max(dot(reflect(-light, normal), float3(0.0, 0.0, 1.0)), 0.0), 22.0);
    const float edge_alpha = smoothstep(1.0, 0.78, radius_squared);
    return float4(in.color.rgb * diffuse + highlight * 0.45,
                  in.color.a * edge_alpha);
}
