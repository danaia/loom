#include <metal_stdlib>
using namespace metal;

struct CrystalVertexOut {
    float4 position [[position]];
    float4 color;
    float2 local;
};

vertex CrystalVertexOut crystal_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device float4* colors [[buffer(0)]],
    const device packed_float3* positions [[buffer(1)]],
    const device float* radii [[buffer(2)]])
{
    constexpr float2 corners[6] = {
        float2(-1.0, -1.0), float2( 1.0, -1.0), float2(-1.0,  1.0),
        float2(-1.0,  1.0), float2( 1.0, -1.0), float2( 1.0,  1.0)
    };
    float radius = radii[instance_id];
    CrystalVertexOut out;
    if (radius <= 0.0f) {
        out.position = float4(2.0f, 2.0f, 1.0f, 1.0f);
        out.color = float4(0.0f);
        out.local = corners[vertex_id];
        return out;
    }

    float3 world = float3(positions[instance_id]);
    // Fixed isometric camera: the simulation remains fully 3D while this first
    // renderer avoids camera state outside the Loom graph.
    float2 center = float2(
        0.84f * world.x + 0.42f * world.z,
        world.y - 0.24f * world.x + 0.34f * world.z);
    float perspective = 1.0f / (1.12f + 0.24f * world.z);
    float2 local = corners[vertex_id];

    out.position = float4((center + local * radius) * perspective, world.z, 1.0f);
    out.color = colors[instance_id];
    out.local = local;
    return out;
}

fragment float4 crystal_pipeline_fragment(CrystalVertexOut in [[stage_in]]) {
    float r2 = dot(in.local, in.local);
    if (r2 > 1.0f || in.color.a <= 0.0f) {
        discard_fragment();
    }
    float highlight = 0.72f + 0.28f * sqrt(max(1.0f - r2, 0.0f));
    return float4(in.color.rgb * highlight, 1.0f);
}
