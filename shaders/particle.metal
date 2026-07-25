#include <metal_stdlib>
using namespace metal;

struct ParticleVertexOut {
    float4 position [[position]];
    float4 color;
};

vertex ParticleVertexOut particle_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    const device float4 *colors [[buffer(0)]],
    const device packed_float3 *positions [[buffer(1)]],
    const device float *radii [[buffer(2)]])
{
    constexpr float2 corners[6] = {
        float2(-1.0, -1.0), float2( 1.0, -1.0), float2(-1.0,  1.0),
        float2(-1.0,  1.0), float2( 1.0, -1.0), float2( 1.0,  1.0)
    };
    float3 world = float3(positions[0]);
    float visual_radius = max(radii[0], 0.015);
    float2 center = float2(world.x, world.y * 1.6 - 0.8);

    ParticleVertexOut out;
    out.position = float4(center + corners[vertex_id] * visual_radius, 0.0, 1.0);
    out.color = colors[0];
    return out;
}

fragment float4 particle_pipeline_fragment(ParticleVertexOut in [[stage_in]]) {
    return in.color;
}
