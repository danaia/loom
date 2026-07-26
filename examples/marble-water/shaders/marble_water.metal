#include <metal_stdlib>
using namespace metal;

struct MarbleVertexOut {
    float4 position [[position]];
    float2 local;
    float4 color;
    float depth;
    float radius;
    float waterline_view;
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
    const float3 projected_position = float3(positions[instance_id]);
    const float depth = max(projected_position.z, 0.05);

    MarbleVertexOut out;
    out.position = float4(
        projected_position.xy + corners[vertex_id] * radius,
        clamp(depth / 5.0, 0.0, 1.0),
        1.0
    );
    out.local = corners[vertex_id];
    out.color = colors[instance_id];
    out.depth = depth;
    out.radius = radius;
    out.waterline_view = plane_scales[instance_id];
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
    float3 shaded_color =
        in.color.rgb * diffuse + highlight * 0.45;
    const bool is_marble = in.color.a >= 1.0;
    const bool is_immersed = is_marble && in.color.a > 1.001;
    const float waterline =
        is_immersed ? clamp(in.color.a - 2.0, -1.0, 1.0) : -1.0;
    float alpha = (is_marble ? 1.0 : in.color.a) * edge_alpha;
    // Reconstruct the visible sphere point's world-space height. Intersecting
    // that curved surface with the horizontal water plane produces the
    // projected elliptical contact arc a real floating sphere has.
    const float view_elevation =
        clamp(in.waterline_view, -0.999, 0.999);
    const float camera_up_y =
        sqrt(max(1.0 - view_elevation * view_elevation, 0.0));
    const float sphere_surface_y =
        in.local.y * camera_up_y + z * view_elevation;
    const bool below_water =
        is_immersed && sphere_surface_y < waterline + 0.025;
    if (below_water) {
        const float submerged_amount = clamp(
            (waterline - sphere_surface_y)
                / max(waterline + 1.0, 0.08),
            0.0,
            1.0
        );
        shaded_color = mix(
            shaded_color,
            shaded_color * float3(0.18, 0.52, 0.72),
            0.48 + submerged_amount * 0.22
        );
        alpha *= mix(0.72, 0.50, submerged_amount);
        const float waterline_highlight =
            1.0 - smoothstep(
                0.0,
                0.055,
                abs(sphere_surface_y - waterline)
            );
        shaded_color +=
            float3(0.32, 0.78, 0.96) * waterline_highlight * 0.32;
    }
    return float4(
        shaded_color,
        alpha
    );
}
