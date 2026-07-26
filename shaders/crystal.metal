#include <metal_stdlib>
using namespace metal;

struct CrystalVertexOut {
    float4 position [[position]];
    float4 color;
    float2 local;
    float3 normal;
    float3 world;
};

vertex CrystalVertexOut crystal_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device float4* colors [[buffer(0)]],
    const device packed_float3* positions [[buffer(1)]],
    const device float* radii [[buffer(2)]],
    const device packed_float3* normals [[buffer(3)]])
{
    constexpr float2 corners[6] = {
        float2(-1.0, -1.0), float2( 1.0, -1.0), float2(-1.0,  1.0),
        float2(-1.0,  1.0), float2( 1.0, -1.0), float2( 1.0,  1.0)
    };
    CrystalVertexOut out;
    float radius = radii[instance_id];
    float2 local = corners[vertex_id];
    if (radius <= 0.0f) {
        out.position = float4(2.0f, 2.0f, 1.0f, 1.0f);
        out.color = float4(0.0f);
        out.local = local;
        out.normal = float3(0.0f, 0.0f, 1.0f);
        out.world = float3(0.0f);
        return out;
    }

    float3 world = float3(positions[instance_id]);
    float2 center = float2(
        0.84f * world.x + 0.42f * world.z,
        world.y - 0.24f * world.x + 0.34f * world.z);
    float perspective = 1.0f / (1.12f + 0.24f * world.z);

    out.position = float4((center + local * radius) * perspective, world.z, 1.0f);
    out.color = colors[instance_id];
    out.local = local;
    out.normal = normalize(float3(normals[instance_id]));
    out.world = world;
    return out;
}

fragment float4 crystal_pipeline_fragment(CrystalVertexOut in [[stage_in]]) {
    if (abs(in.local.x) + abs(in.local.y) > 1.72f || in.color.a <= 0.0f) {
        discard_fragment();
    }

    float3 light = normalize(float3(-0.42f, 0.78f, 0.56f));
    float3 view = normalize(float3(0.18f, 0.12f, 1.0f));
    float3 micro = normalize(in.normal + float3(in.local.x, in.local.y, 0.0f) * 0.16f);
    float diffuse = 0.30f + 0.70f * max(dot(micro, light), 0.0f);
    float fresnel = pow(1.0f - abs(dot(micro, view)), 3.0f);
    float glint = pow(max(dot(reflect(-light, micro), view), 0.0f), 28.0f);
    float internal_band = 0.5f + 0.5f * sin(
        in.world.x * 74.0f + in.world.y * 41.0f - in.world.z * 57.0f);
    float bevel = smoothstep(0.58f, 1.0f, max(abs(in.local.x), abs(in.local.y)));

    float3 deep = in.color.rgb * (0.62f + 0.20f * internal_band);
    float3 rim = float3(0.76f, 0.96f, 1.0f);
    float3 color = deep * diffuse;
    color = mix(color, rim, 0.42f * fresnel + 0.18f * bevel);
    color += glint * float3(1.0f, 0.98f, 0.88f);
    return float4(color, 1.0f);
}
