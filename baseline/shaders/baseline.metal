#include <metal_stdlib>
using namespace metal;

struct BaselineVertexOut {
    float4 position [[position]];
    float2 local;
    float4 color;
};

vertex BaselineVertexOut baseline_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device float4 *colors [[buffer(0)]],
    const device packed_float3 *positions [[buffer(1)]],
    const device float *radii [[buffer(2)]],
    const device float *aspects [[buffer(3)]])
{
    constexpr float2 corners[6] = {
        float2(-1.0, -1.0), float2( 1.0, -1.0), float2(-1.0,  1.0),
        float2(-1.0,  1.0), float2( 1.0, -1.0), float2( 1.0,  1.0)
    };
    const float3 center = float3(positions[instance_id]);
    const float2 local = corners[vertex_id];
    const float radius = radii[instance_id];
    const float aspect = max(aspects[instance_id], 0.1);

    BaselineVertexOut out;
    out.position = float4(
        center.xy + float2(local.x * radius / aspect, local.y * radius),
        clamp(center.z / 6.0, 0.0, 1.0),
        1.0
    );
    out.local = local;
    out.color = colors[instance_id];
    return out;
}

fragment float4 baseline_pipeline_fragment(BaselineVertexOut in [[stage_in]])
{
    const float radius_squared = dot(in.local, in.local);
    if (radius_squared > 1.0) discard_fragment();

    const float z = sqrt(max(1.0 - radius_squared, 0.0));
    const float3 normal = normalize(float3(in.local.x, -in.local.y, z));
    const float3 light = normalize(float3(-0.45, 0.65, 0.75));
    const float diffuse = 0.24 + 0.76 * max(dot(normal, light), 0.0);
    const float highlight = pow(
        max(dot(reflect(-light, normal), float3(0.0, 0.0, 1.0)), 0.0),
        28.0
    );
    const float edge = smoothstep(1.0, 0.72, radius_squared);
    const float3 shaded = in.color.rgb * diffuse + highlight * 0.55;
    return float4(shaded, edge);
}
