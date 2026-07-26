#include <metal_stdlib>
using namespace metal;

struct WormVertexOut {
    float4 position [[position]];
    float4 color;
    float2 local;
    float3 view_position;
    uint kind [[flat]];
};

inline float3 rotate_for_camera(float3 world, float yaw, float pitch) {
    float cy = cos(yaw);
    float sy = sin(yaw);
    float3 yawed = float3(
        cy * world.x + sy * world.z,
        world.y,
        -sy * world.x + cy * world.z);
    float cp = cos(pitch);
    float sp = sin(pitch);
    return float3(
        yawed.x,
        cp * yawed.y - sp * yawed.z,
        sp * yawed.y + cp * yawed.z);
}

inline float2 project_world(float3 view) {
    float2 center = float2(
        0.84f * view.x + 0.42f * view.z,
        view.y - 0.24f * view.x + 0.34f * view.z);
    return center / (1.16f + 0.24f * view.z);
}

vertex WormVertexOut worm_pipeline_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device float4* colors [[buffer(0)]],
    const device uint* kinds [[buffer(1)]],
    const device packed_float3* positions [[buffer(2)]],
    const device float* radii [[buffer(3)]])
{
    constexpr float2 corners[6] = {
        float2(-1.0, -1.0), float2( 1.0, -1.0), float2(-1.0,  1.0),
        float2(-1.0,  1.0), float2( 1.0, -1.0), float2( 1.0,  1.0)
    };
    WormVertexOut out;
    float2 local = corners[vertex_id];
    uint kind = kinds[instance_id];
    out.color = colors[instance_id];
    out.local = local;
    out.kind = kind;

    if (kind == 0u) {
        float3 camera = float3(positions[instance_id]);
        float3 world = float3(local.x, 0.0f, local.y);
        float3 view = rotate_for_camera(world, camera.x, camera.y) * camera.z;
        out.position = float4(project_world(view), 0.98f, 1.0f);
        out.view_position = view;
        return out;
    }

    float radius = radii[instance_id];
    if (radius <= 0.0f) {
        out.position = float4(2.0f, 2.0f, 1.0f, 1.0f);
        out.view_position = float3(0.0f);
        return out;
    }
    float3 view = float3(positions[instance_id]);
    float perspective = 1.0f / (1.16f + 0.24f * view.z);
    out.position = float4(project_world(view) + local * radius * perspective, 0.2f, 1.0f);
    out.view_position = view;
    return out;
}

fragment float4 worm_pipeline_fragment(WormVertexOut in [[stage_in]]) {
    if (in.kind == 0u) {
        float2 grid_uv = (in.local + 1.0f) * 10.0f;
        float2 cell = abs(fract(grid_uv) - 0.5f) / fwidth(grid_uv);
        float grid = 1.0f - min(min(cell.x, cell.y), 1.0f);
        float2 major_uv = (in.local + 1.0f) * 2.0f;
        float2 major_cell = abs(fract(major_uv) - 0.5f) / fwidth(major_uv);
        float major = 1.0f - min(min(major_cell.x, major_cell.y), 1.0f);
        float vignette = smoothstep(1.42f, 0.42f, length(in.local));
        float3 ground = mix(float3(0.025f, 0.055f, 0.043f), float3(0.07f, 0.16f, 0.105f), vignette);
        ground += grid * float3(0.035f, 0.13f, 0.085f);
        ground += major * float3(0.05f, 0.20f, 0.12f);
        return float4(ground, 1.0f);
    }

    float radial = dot(in.local, in.local);
    if (radial > 1.0f || in.color.a <= 0.0f) discard_fragment();

    float3 normal = normalize(float3(in.local, sqrt(max(1.0f - radial, 0.0f))));
    float3 light = normalize(float3(-0.42f, 0.76f, 0.55f));
    float diffuse = 0.30f + 0.70f * max(dot(normal, light), 0.0f);
    float rim = pow(1.0f - normal.z, 2.8f);
    float highlight = pow(max(dot(reflect(-light, normal), float3(0.0f, 0.0f, 1.0f)), 0.0f), 24.0f);
    float3 color = in.color.rgb * diffuse;
    color = mix(color, float3(0.64f, 1.0f, 0.82f), rim * (in.kind == 1u ? 0.38f : 0.12f));
    color += highlight * float3(1.0f, 0.94f, 0.72f);
    if (in.kind == 2u) {
        color += (1.0f - radial) * float3(0.20f, 0.035f, 0.0f);
    }
    return float4(color, 1.0f);
}
