#include <metal_stdlib>
using namespace metal;

constant uint EMPTY_COMPONENT = 0xffffffffu;
constant uint METRIC_SOLID = 0u;
constant uint METRIC_INTERFACE = 1u;
constant uint METRIC_SURFACE = 2u;
constant uint METRIC_DAMAGED = 3u;
constant uint METRIC_DETACHED = 4u;
constant uint METRIC_COMPONENT_ROOTS = 5u;
constant uint METRIC_SOLUTE_Q10 = 6u;
constant uint METRIC_TEMPERATURE_Q10 = 7u;
constant uint METRIC_SLICE_COUNT = 8u;
constant uint METRIC_PHASE_Q10 = 9u;

inline uint index_3d(uint3 p, uint width) {
    return (p.z * width + p.y) * width + p.x;
}

inline uint3 coordinate_3d(uint index, uint width) {
    uint plane = width * width;
    return uint3(index % width, (index / width) % width, index / plane);
}

inline uint neighbor_index(uint3 p, int axis, int direction, uint width) {
    int3 q = int3(p);
    q[axis] = clamp(q[axis] + direction, 0, int(width) - 1);
    return index_3d(uint3(q), width);
}

kernel void crystal_initialize(
    device float* phase [[buffer(0)]],
    device float* phase_next [[buffer(1)]],
    device float* solute [[buffer(2)]],
    device float* solute_next [[buffer(3)]],
    device float* temperature [[buffer(4)]],
    device float* temperature_next [[buffer(5)]],
    device float* damage [[buffer(6)]],
    device uint* component [[buffer(7)]],
    device packed_float3* position [[buffer(8)]],
    device packed_float3* velocity [[buffer(9)]],
    const device uint* tick [[buffer(10)]],
    constant uint& width [[buffer(11)]],
    uint gid [[thread_position_in_grid]])
{
    if (tick[0] != 0u) {
        return;
    }

    uint3 cell = coordinate_3d(gid, width);
    float3 center = (float3(cell) + 0.5f) - float(width) * 0.5f;
    float seeded = length(center) <= 2.35f ? 1.0f : 0.0f;
    float3 q = center / float(width);
    constexpr float reservoir_angle = 0.39f;
    float reservoir_cs = cos(reservoir_angle);
    float reservoir_sn = sin(reservoir_angle);
    float3 crystal_q = float3(
        reservoir_cs * q.x + reservoir_sn * q.z,
        q.y,
        -reservoir_sn * q.x + reservoir_cs * q.z);
    float3 absolute_q = abs(crystal_q);
    float axial_extent = max(absolute_q.x, max(absolute_q.y, absolute_q.z)) / 0.205f;
    float diagonal_extent =
        (absolute_q.x + absolute_q.y + absolute_q.z) / 0.345f;
    float wulff_extent = max(axial_extent, diagonal_extent);
    float reservoir = 1.0f - smoothstep(0.90f, 1.04f, wulff_extent);
    float cell_scale = 1.55f / float(width);
    float3 world = center * cell_scale;

    phase[gid] = seeded;
    phase_next[gid] = seeded;
    solute[gid] = seeded > 0.5f ? 0.42f : mix(0.27f, 1.0f, reservoir);
    solute_next[gid] = solute[gid];
    temperature[gid] = seeded > 0.5f ? 0.18f : 0.02f;
    temperature_next[gid] = temperature[gid];
    damage[gid] = 0.0f;
    component[gid] = seeded > 0.5f ? gid : EMPTY_COMPONENT;
    position[gid] = packed_float3(world);
    velocity[gid] = packed_float3(0.0f);
}

kernel void crystal_evolve_fields(
    const device float* phase [[buffer(0)]],
    const device float* solute [[buffer(1)]],
    const device float* temperature [[buffer(2)]],
    const device float* damage [[buffer(3)]],
    device float* phase_next [[buffer(4)]],
    device float* solute_next [[buffer(5)]],
    device float* temperature_next [[buffer(6)]],
    const device uint* tick [[buffer(7)]],
    constant uint& width [[buffer(8)]],
    constant float& growth_rate [[buffer(9)]],
    constant float& anisotropy_strength [[buffer(10)]],
    constant float& solute_diffusion [[buffer(11)]],
    constant float& thermal_diffusion [[buffer(12)]],
    uint gid [[thread_position_in_grid]])
{
    float p = phase[gid];
    float c = solute[gid];
    float t = temperature[gid];
    if ((tick[0] & 1u) != 0u) {
        phase_next[gid] = p;
        solute_next[gid] = c;
        temperature_next[gid] = t;
        return;
    }

    uint3 cell = coordinate_3d(gid, width);
    uint xm = neighbor_index(cell, 0, -1, width);
    uint xp = neighbor_index(cell, 0,  1, width);
    uint ym = neighbor_index(cell, 1, -1, width);
    uint yp = neighbor_index(cell, 1,  1, width);
    uint zm = neighbor_index(cell, 2, -1, width);
    uint zp = neighbor_index(cell, 2,  1, width);

    float phase_sum =
        phase[xm] + phase[xp] + phase[ym] + phase[yp] + phase[zm] + phase[zp];
    float solute_laplacian =
        solute[xm] + solute[xp] + solute[ym] + solute[yp] + solute[zm] + solute[zp] - 6.0f * c;
    float thermal_laplacian =
        temperature[xm] + temperature[xp] + temperature[ym] + temperature[yp] +
        temperature[zm] + temperature[zp] - 6.0f * t;

    float3 gradient = float3(
        phase[xp] - phase[xm],
        phase[yp] - phase[ym],
        phase[zp] - phase[zm]);
    float gradient_length = length(gradient);
    float3 normal = gradient_length > 1.0e-5f
        ? gradient / gradient_length
        : normalize(float3(cell) + 0.5f - float(width) * 0.5f + float3(1.0e-4f));

    // Rotate the surface normal into a fixed crystal frame. The fourth powers
    // impose cubic symmetry while the rotation keeps it off the world axes.
    constexpr float angle = 0.39f;
    float cs = cos(angle);
    float sn = sin(angle);
    float3 local_normal = float3(
        cs * normal.x + sn * normal.z,
        normal.y,
        -sn * normal.x + cs * normal.z);
    float cubic = dot(local_normal * local_normal, local_normal * local_normal);
    float anisotropy = mix(1.0f - anisotropy_strength, 1.0f + anisotropy_strength, cubic);

    float equilibrium = 0.31f + 0.16f * t;
    float supersaturation = max(c - equilibrium, 0.0f);
    float interface_contact = smoothstep(0.04f, 1.1f, phase_sum);
    float curvature = phase_sum - 6.0f * p;
    float drive = growth_rate * supersaturation * anisotropy * interface_contact;
    drive += 0.006f * min(curvature, 0.0f);
    drive *= 1.0f - clamp(damage[gid], 0.0f, 1.0f);

    float next_p = p;
    if (p < 0.999f && phase_sum > 0.04f) {
        next_p = clamp(p + max(drive, 0.0f), 0.0f, 1.0f);
    }
    float solidified = max(next_p - p, 0.0f);
    float next_c = c + solute_diffusion * solute_laplacian - solidified * 0.62f;
    float next_t = t + thermal_diffusion * thermal_laplacian + solidified * 0.13f;

    phase_next[gid] = next_p;
    solute_next[gid] = clamp(next_c, 0.0f, 1.25f);
    temperature_next[gid] = clamp(next_t * 0.9992f, 0.0f, 1.0f);
}

kernel void crystal_commit_fields(
    device float* phase [[buffer(0)]],
    const device float* phase_next [[buffer(1)]],
    device float* solute [[buffer(2)]],
    const device float* solute_next [[buffer(3)]],
    device float* temperature [[buffer(4)]],
    const device float* temperature_next [[buffer(5)]],
    uint gid [[thread_position_in_grid]])
{
    phase[gid] = phase_next[gid];
    solute[gid] = solute_next[gid];
    temperature[gid] = temperature_next[gid];
}

inline float2 project_crystal(float3 world) {
    float2 center = float2(
        0.84f * world.x + 0.42f * world.z,
        world.y - 0.24f * world.x + 0.34f * world.z);
    float perspective = 1.0f / (1.12f + 0.24f * world.z);
    return center * perspective;
}

inline float distance_to_segment(float2 point, float2 start, float2 end) {
    float2 span = end - start;
    float denominator = max(dot(span, span), 1.0e-7f);
    float along = clamp(dot(point - start, span) / denominator, 0.0f, 1.0f);
    return length(point - (start + span * along));
}

kernel void crystal_slice_material(
    const device float* phase [[buffer(0)]],
    const device packed_float3* position [[buffer(1)]],
    device float* damage [[buffer(2)]],
    device packed_float3* velocity [[buffer(3)]],
    device atomic_uint* slice_count [[buffer(4)]],
    constant float& start_x [[buffer(5)]],
    constant float& start_y [[buffer(6)]],
    constant float& end_x [[buffer(7)]],
    constant float& end_y [[buffer(8)]],
    constant float& radius [[buffer(9)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid == 0u) {
        atomic_fetch_add_explicit(&slice_count[0], 1u, memory_order_relaxed);
    }
    if (phase[gid] < 0.55f || damage[gid] >= 0.98f) {
        return;
    }

    float2 start = float2(start_x, start_y);
    float2 end = float2(end_x, end_y);
    float2 screen = project_crystal(float3(position[gid]));
    if (distance_to_segment(screen, start, end) <= radius) {
        float2 tangent = normalize((end - start) + float2(1.0e-5f, 0.0f));
        float2 normal = float2(-tangent.y, tangent.x);
        float side = dot(screen - start, normal) >= 0.0f ? 1.0f : -1.0f;
        damage[gid] = 1.0f;
        velocity[gid] = packed_float3(float3(normal.x * side, 0.08f, normal.y * side) * 0.11f);
    }
}

kernel void crystal_initialize_components(
    const device float* phase [[buffer(0)]],
    const device float* damage [[buffer(1)]],
    device uint* component [[buffer(2)]],
    uint gid [[thread_position_in_grid]])
{
    if (phase[gid] >= 0.55f && damage[gid] < 0.98f) {
        if (component[gid] == EMPTY_COMPONENT) {
            component[gid] = gid;
        }
    } else {
        component[gid] = EMPTY_COMPONENT;
    }
}

kernel void crystal_relax_components(
    const device float* phase [[buffer(0)]],
    const device float* damage [[buffer(1)]],
    device atomic_uint* component [[buffer(2)]],
    constant uint& width [[buffer(3)]],
    uint gid [[thread_position_in_grid]])
{
    if (phase[gid] < 0.55f || damage[gid] >= 0.98f) {
        return;
    }
    uint3 cell = coordinate_3d(gid, width);
    uint label = atomic_load_explicit(&component[gid], memory_order_relaxed);
    for (int axis = 0; axis < 3; ++axis) {
        for (int direction = -1; direction <= 1; direction += 2) {
            uint neighbor = neighbor_index(cell, axis, direction, width);
            if (phase[neighbor] >= 0.55f && damage[neighbor] < 0.98f) {
                label = min(
                    label,
                    atomic_load_explicit(&component[neighbor], memory_order_relaxed));
            }
        }
    }
    atomic_fetch_min_explicit(&component[gid], label, memory_order_relaxed);
}

kernel void crystal_integrate_fragments(
    const device float* phase [[buffer(0)]],
    const device float* damage [[buffer(1)]],
    const device uint* component [[buffer(2)]],
    device packed_float3* position [[buffer(3)]],
    device packed_float3* velocity [[buffer(4)]],
    const device uint* slice_count [[buffer(5)]],
    constant uint& seed_index [[buffer(6)]],
    constant float& fixed_dt [[buffer(7)]],
    uint gid [[thread_position_in_grid]])
{
    if (slice_count[0] == 0u || phase[gid] < 0.55f || damage[gid] >= 0.98f) {
        return;
    }
    uint main_component = component[seed_index];
    if (component[gid] == EMPTY_COMPONENT || component[gid] == main_component) {
        velocity[gid] = packed_float3(0.0f);
        return;
    }

    float3 v = float3(velocity[gid]);
    float3 p = float3(position[gid]);
    v.y -= 0.34f * fixed_dt;
    p += v * fixed_dt;
    if (p.y < -0.77f) {
        p.y = -0.77f;
        v.y = abs(v.y) * 0.28f;
        v.xz *= 0.91f;
    }
    position[gid] = packed_float3(p);
    velocity[gid] = packed_float3(v);
}

kernel void crystal_prepare_render(
    const device float* phase [[buffer(0)]],
    const device float* damage [[buffer(1)]],
    const device uint* component [[buffer(2)]],
    const device packed_float3* position [[buffer(3)]],
    device packed_float3* render_position [[buffer(4)]],
    device packed_float3* render_normal [[buffer(5)]],
    device float4* color [[buffer(6)]],
    device float* radius [[buffer(7)]],
    constant uint& width [[buffer(8)]],
    constant uint& seed_index [[buffer(9)]],
    uint gid [[thread_position_in_grid]])
{
    float p = phase[gid];
    if (p < 0.55f || damage[gid] >= 0.98f) {
        render_position[gid] = packed_float3(0.0f);
        render_normal[gid] = packed_float3(0.0f);
        color[gid] = float4(0.0f);
        radius[gid] = 0.0f;
        return;
    }

    uint3 cell = coordinate_3d(gid, width);
    float3 outward = float3(0.0f);
    bool cut_edge = false;
    for (int axis = 0; axis < 3; ++axis) {
        for (int direction = -1; direction <= 1; direction += 2) {
            uint neighbor = neighbor_index(cell, axis, direction, width);
            bool exposed = phase[neighbor] < 0.55f || damage[neighbor] >= 0.98f;
            if (exposed) {
                outward[axis] += float(direction);
                cut_edge = cut_edge || damage[neighbor] >= 0.98f;
            }
        }
    }
    float3 surface_normal = normalize(outward);
    if (length_squared(outward) < 0.5f) {
        render_position[gid] = packed_float3(0.0f);
        render_normal[gid] = packed_float3(0.0f);
        color[gid] = float4(0.0f);
        radius[gid] = 0.0f;
        return;
    }

    float3 world = float3(position[gid]);
    bool detached = component[gid] != component[seed_index];
    float3 base = detached
        ? float3(0.20f, 0.67f, 0.94f)
        : float3(0.35f, 0.82f, 1.0f);
    base = cut_edge ? mix(base, float3(0.82f, 0.96f, 1.0f), 0.72f) : base;
    float depth_light = 0.58f + 0.42f * clamp(world.z + 0.55f, 0.0f, 1.0f);

    render_position[gid] = position[gid];
    render_normal[gid] = packed_float3(surface_normal);
    color[gid] = float4(base * depth_light, cut_edge ? 1.0f : 0.88f);
    radius[gid] = 1.46f / float(width);
}

kernel void crystal_clear_metrics(
    device uint* metrics [[buffer(0)]],
    uint gid [[thread_position_in_grid]])
{
    metrics[gid] = 0u;
}

kernel void crystal_reduce_metrics(
    const device float* phase [[buffer(0)]],
    const device float* solute [[buffer(1)]],
    const device float* temperature [[buffer(2)]],
    const device float* damage [[buffer(3)]],
    const device uint* component [[buffer(4)]],
    const device float* radius [[buffer(5)]],
    const device uint* slice_count [[buffer(6)]],
    device atomic_uint* metrics [[buffer(7)]],
    constant uint& seed_index [[buffer(8)]],
    uint gid [[thread_position_in_grid]])
{
    float p = phase[gid];
    bool solid = p >= 0.55f && damage[gid] < 0.98f;
    if (solid) {
        atomic_fetch_add_explicit(&metrics[METRIC_SOLID], 1u, memory_order_relaxed);
        if (p < 0.98f) {
            atomic_fetch_add_explicit(&metrics[METRIC_INTERFACE], 1u, memory_order_relaxed);
        }
        if (radius[gid] > 0.0f) {
            atomic_fetch_add_explicit(&metrics[METRIC_SURFACE], 1u, memory_order_relaxed);
        }
        if (component[gid] != component[seed_index]) {
            atomic_fetch_add_explicit(&metrics[METRIC_DETACHED], 1u, memory_order_relaxed);
        }
        if (component[gid] == gid) {
            atomic_fetch_add_explicit(&metrics[METRIC_COMPONENT_ROOTS], 1u, memory_order_relaxed);
        }
    }
    if (damage[gid] > 0.02f) {
        atomic_fetch_add_explicit(&metrics[METRIC_DAMAGED], 1u, memory_order_relaxed);
    }
    atomic_fetch_add_explicit(
        &metrics[METRIC_SOLUTE_Q10], uint(clamp(solute[gid], 0.0f, 4.0f) * 1024.0f),
        memory_order_relaxed);
    atomic_fetch_add_explicit(
        &metrics[METRIC_TEMPERATURE_Q10],
        uint(clamp(temperature[gid], 0.0f, 4.0f) * 1024.0f),
        memory_order_relaxed);
    if (gid == 0u) {
        atomic_store_explicit(&metrics[METRIC_SLICE_COUNT], slice_count[0], memory_order_relaxed);
    }
    atomic_fetch_add_explicit(
        &metrics[METRIC_PHASE_Q10], uint(clamp(p, 0.0f, 1.0f) * 1024.0f),
        memory_order_relaxed);
}

kernel void crystal_advance_tick(
    device uint* tick [[buffer(0)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid == 0u) {
        tick[0] += 1u;
    }
}
