#include <metal_stdlib>
using namespace metal;

struct NeonVertexOut {
    float4 position [[position]];
    float2 local;
    float hue;
};

vertex NeonVertexOut neon_flock_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device packed_float2 *positions [[buffer(0)]],
    const device packed_float2 *trail_positions [[buffer(1)]])
{
    constexpr float2 corners[6] = {
        float2(0.0, -1.0), float2(1.0, -1.0), float2(0.0, 1.0),
        float2(0.0,  1.0), float2(1.0, -1.0), float2(1.0, 1.0)
    };

    const float2 head = float2(positions[instance_id]);
    float2 tail = float2(trail_positions[instance_id]);
    float2 segment = head - tail;
    float segment_length = length(segment);
    if (segment_length < 0.002) {
        const float seed = fract(sin((float(instance_id) + 1.0) * 12.9898) * 43758.5453);
        const float angle = seed * 6.28318530718;
        segment = float2(cos(angle), sin(angle)) * 0.002;
        tail = head - segment;
        segment_length = 0.002;
    }

    const float2 direction = segment / segment_length;
    const float2 normal = float2(-direction.y, direction.x);
    const float along = corners[vertex_id].x;
    const float across = corners[vertex_id].y;
    const float width = mix(0.007, 0.015, along);
    const float2 world = mix(tail, head, along) + normal * across * width;

    NeonVertexOut out;
    out.position = float4(world, 0.0, 1.0);
    out.local = float2(along, across);
    out.hue = fract(float(instance_id) * 0.61803398875);
    return out;
}

fragment float4 neon_flock_pipeline_fragment(NeonVertexOut in [[stage_in]])
{
    const float3 palette = 0.58 + 0.42 * cos(
        6.28318530718 * (in.hue + float3(0.00, 0.23, 0.48)));
    const float edge = smoothstep(1.0, 0.0, abs(in.local.y));
    const float core = pow(edge, 4.0);
    const float tail_fade = smoothstep(0.0, 0.32, in.local.x);
    const float head = smoothstep(0.62, 1.0, in.local.x);
    const float glow = edge * tail_fade * (0.10 + 0.30 * head);
    const float hot_core = core * tail_fade * (0.14 + 0.62 * head);
    const float3 color =
        palette * (glow + hot_core) + float3(0.55, 0.72, 0.90) * hot_core * head * 0.45;
    const float alpha = clamp(glow + hot_core, 0.0, 1.0);
    return float4(color, alpha);
}
