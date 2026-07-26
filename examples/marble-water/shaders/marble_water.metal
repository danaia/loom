#include <metal_stdlib>
using namespace metal;

struct MarbleVertexOut {
    float4 position [[position]];
    float2 local;
    float4 color;
    float depth;
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
    out.position = float4(
        projected + corners[vertex_id] * visual_radius,
        clamp(depth / 5.0, 0.0, 1.0),
        1.0
    );
    out.local = corners[vertex_id];
    out.color = colors[instance_id];
    out.depth = depth;
    return out;
}

fragment float4 marble_water_pipeline_fragment(MarbleVertexOut in [[stage_in]])
{
    const float radius_squared = dot(in.local, in.local);
    if (radius_squared > 1.0) discard_fragment();
    const float z = sqrt(max(1.0 - radius_squared, 0.0));
    const float3 normal = normalize(float3(in.local.x, -in.local.y, z));
    const float3 light = normalize(float3(-0.35, 0.65, 0.80));
    const float diffuse = 0.30 + 0.70 * max(dot(normal, light), 0.0);
    const float highlight = pow(
        max(dot(reflect(-light, normal), float3(0.0, 0.0, 1.0)), 0.0),
        22.0
    );
    const float edge_alpha = smoothstep(1.0, 0.78, radius_squared);
    return float4(
        in.color.rgb * diffuse + highlight * 0.45,
        in.color.a * edge_alpha
    );
}
