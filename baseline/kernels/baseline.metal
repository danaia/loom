#include <metal_stdlib>
using namespace metal;

constant uint PARTICLE_CAPACITY = 32;

inline float wrap_axis(float value, float extent)
{
    const float span = extent * 2.0;
    if (value > extent) return value - span;
    if (value < -extent) return value + span;
    return value;
}

kernel void baseline_apply_interaction(
    device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    device float *active [[buffer(2)]],
    device packed_float3 *targets [[buffer(3)]],
    device float *target_active [[buffer(4)]],
    device float *particle_types [[buffer(5)]],
    device float *selected [[buffer(6)]],
    device float *spawn_seen [[buffer(7)]],
    device float *click_seen [[buffer(8)]],
    constant float &click_x [[buffer(9)]],
    constant float &click_y [[buffer(10)]],
    constant float &click_z [[buffer(11)]],
    constant float &click_generation [[buffer(12)]],
    constant float &spawn_x [[buffer(13)]],
    constant float &spawn_y [[buffer(14)]],
    constant float &spawn_z [[buffer(15)]],
    constant float &spawn_generation [[buffer(16)]],
    constant float &spawn_slot [[buffer(17)]],
    constant float &spawn_type [[buffer(18)]],
    device float *select_seen [[buffer(19)]],
    device float *remove_seen [[buffer(20)]],
    constant float &select_command [[buffer(21)]],
    constant float &remove_command [[buffer(22)]],
    constant float &selection_radius [[buffer(23)]],
    constant float &reset [[buffer(24)]],
    device float *dragging [[buffer(25)]],
    constant float &pointer_down [[buffer(26)]],
    constant float &drag_x [[buffer(27)]],
    constant float &drag_y [[buffer(28)]],
    constant float &drag_z [[buffer(29)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;

    if (reset > 0.5) {
        for (uint i = 0; i < PARTICLE_CAPACITY; ++i) {
            positions[i] = packed_float3(0.0);
            velocities[i] = packed_float3(0.0);
            targets[i] = packed_float3(0.0);
            active[i] = i == 0 ? 1.0 : 0.0;
            target_active[i] = 0.0;
            particle_types[i] = 0.0;
        }
        selected[0] = 0.0;
        spawn_seen[0] = spawn_generation;
        click_seen[0] = click_generation;
        select_seen[0] = select_command;
        remove_seen[0] = remove_command;
        dragging[0] = 0.0;
        return;
    }

    if (remove_seen[0] != remove_command) {
        const uint slot = uint(max(remove_command, 0.0)) % PARTICLE_CAPACITY;
        active[slot] = 0.0;
        positions[slot] = packed_float3(0.0);
        velocities[slot] = packed_float3(0.0);
        targets[slot] = packed_float3(0.0);
        target_active[slot] = 0.0;
        dragging[0] = 0.0;
        remove_seen[0] = remove_command;
    }

    if (select_seen[0] != select_command) {
        selected[0] = float(uint(max(select_command, 0.0)) % PARTICLE_CAPACITY);
        dragging[0] = 0.0;
        select_seen[0] = select_command;
    }

    if (spawn_seen[0] != spawn_generation) {
        const uint slot = uint(clamp(spawn_slot, 0.0, float(PARTICLE_CAPACITY - 1)));
        positions[slot] = packed_float3(spawn_x, spawn_y, spawn_z);
        velocities[slot] = packed_float3(0.0);
        targets[slot] = packed_float3(0.0);
        active[slot] = 1.0;
        target_active[slot] = 0.0;
        particle_types[slot] = clamp(spawn_type, 0.0, 2.0);
        selected[0] = float(slot);
        spawn_seen[0] = spawn_generation;
    }

    if (pointer_down <= 0.5) {
        dragging[0] = 0.0;
    }

    if (click_seen[0] != click_generation) {
        const float2 click = float2(click_x, click_y);
        float closest_distance = INFINITY;
        int closest = -1;
        for (uint i = 0; i < PARTICLE_CAPACITY; ++i) {
            if (active[i] <= 0.5) continue;
            const float distance_to_click = distance(float2(positions[i].xy), click);
            if (distance_to_click < closest_distance) {
                closest_distance = distance_to_click;
                closest = int(i);
            }
        }

        if (closest >= 0 && closest_distance <= selection_radius) {
            selected[0] = float(closest);
            dragging[0] = 1.0;
            target_active[uint(closest)] = 0.0;
        } else {
            dragging[0] = 0.0;
            const uint slot = uint(clamp(selected[0], 0.0, float(PARTICLE_CAPACITY - 1)));
            if (active[slot] > 0.5) {
                targets[slot] = packed_float3(click_x, click_y, click_z);
                target_active[slot] = 1.0;
            }
        }
        click_seen[0] = click_generation;
    }

    if (pointer_down > 0.5 && dragging[0] > 0.5) {
        const uint slot = uint(clamp(selected[0], 0.0, float(PARTICLE_CAPACITY - 1)));
        positions[slot] = packed_float3(drag_x, drag_y, drag_z);
        velocities[slot] = packed_float3(0.0);
        target_active[slot] = 0.0;
    }
}

kernel void baseline_move_particle(
    device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    const device float *active [[buffer(2)]],
    device packed_float3 *targets [[buffer(3)]],
    device float *target_active [[buffer(4)]],
    constant float3 &gravity [[buffer(5)]],
    constant float &space_drag [[buffer(6)]],
    constant float &target_spring [[buffer(7)]],
    constant float &target_damping [[buffer(8)]],
    constant float &arrival_radius [[buffer(9)]],
    constant float &half_extent_x [[buffer(10)]],
    constant float &half_extent_y [[buffer(11)]],
    constant float &half_extent_z [[buffer(12)]],
    constant float &dt [[buffer(13)]],
    uint index [[thread_position_in_grid]])
{
    if (active[index] <= 0.5) return;

    float3 position = float3(positions[index]);
    float3 velocity = float3(velocities[index]);
    const float3 target = float3(targets[index]);

    if (target_active[index] > 0.5) {
        const float distance_to_target = distance(position, target);
        if (distance_to_target <= arrival_radius) {
            positions[index] = packed_float3(target);
            velocities[index] = packed_float3(0.0);
            target_active[index] = 0.0;
            return;
        }
        velocity += (gravity + (target - position) * target_spring - velocity * target_damping) * dt;
    } else {
        velocity += gravity * dt;
        velocity *= exp(-max(space_drag, 0.0) * dt);
    }

    position += velocity * dt;
    position.x = wrap_axis(position.x, half_extent_x);
    position.y = wrap_axis(position.y, half_extent_y);
    position.z = wrap_axis(position.z, half_extent_z);
    positions[index] = packed_float3(position);
    velocities[index] = packed_float3(velocity);
}

kernel void baseline_project_particles(
    const device packed_float3 *particle_positions [[buffer(0)]],
    const device float *particle_active [[buffer(1)]],
    const device packed_float3 *particle_targets [[buffer(2)]],
    const device float *particle_target_active [[buffer(3)]],
    const device float *particle_types [[buffer(4)]],
    const device float *selected [[buffer(5)]],
    device packed_float3 *render_positions [[buffer(6)]],
    device float *render_radii [[buffer(7)]],
    device float4 *render_colors [[buffer(8)]],
    device float *render_aspects [[buffer(9)]],
    constant float &radius [[buffer(10)]],
    constant float &target_radius [[buffer(11)]],
    constant float &aspect [[buffer(12)]],
    uint index [[thread_position_in_grid]])
{
    constexpr float camera_z = 3.0;
    constexpr float focal = 1.85;
    const uint particle_index = index % PARTICLE_CAPACITY;
    const bool is_target = index >= PARTICLE_CAPACITY;
    const bool visible = particle_active[particle_index] > 0.5
        && (!is_target || particle_target_active[particle_index] > 0.5);
    const float3 position = is_target
        ? float3(particle_targets[particle_index])
        : float3(particle_positions[particle_index]);
    const float depth = max(camera_z - position.z, 0.1);
    const float safe_aspect = max(aspect, 0.1);
    const bool is_selected = !is_target && uint(selected[0]) == particle_index;

    render_positions[index] = packed_float3(
        position.x * focal / (depth * safe_aspect),
        position.y * focal / depth,
        depth
    );
    render_radii[index] = visible
        ? (is_target ? target_radius : radius) * focal / depth
        : 0.0;
    render_aspects[index] = safe_aspect;
    const uint agent_type = uint(clamp(particle_types[particle_index], 0.0, 2.0));
    const float4 type_color = agent_type == 1
        ? float4(0.48, 0.92, 0.48, 1.0)
        : (agent_type == 2
            ? float4(0.78, 0.48, 1.0, 1.0)
            : float4(0.18, 0.72, 1.0, 1.0));
    render_colors[index] = is_target
        ? float4(1.0, 0.72, 0.18, 1.0)
        : (is_selected ? mix(type_color, float4(1.0), 0.35) : type_color);
}
