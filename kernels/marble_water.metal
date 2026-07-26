#include <metal_stdlib>
using namespace metal;

inline void contain_marble(
    thread float3 &position,
    thread float3 &velocity,
    float half_extent_x,
    float half_extent_z,
    float radius)
{
    const float limit_x = half_extent_x - radius;
    const float limit_z = half_extent_z - radius;
    if (abs(position.x) > limit_x) {
        position.x = clamp(position.x, -limit_x, limit_x);
        velocity.x *= -0.45;
    }
    if (abs(position.z) > limit_z) {
        position.z = clamp(position.z, -limit_z, limit_z);
        velocity.z *= -0.45;
    }
}

inline uint active_water_width(float density, uint maximum_width)
{
    const uint minimum_width = min(64u, maximum_width);
    return min(
        maximum_width,
        minimum_width + uint(round(clamp(density, 0.0f, 1.0f) * float(maximum_width - minimum_width)))
    );
}

inline uint active_water_height(uint active_width, uint maximum_width, uint maximum_height)
{
    return max(1u, min(
        maximum_height,
        uint(round(float(active_width) * float(maximum_height) / float(max(maximum_width, 1u))))
    ));
}

inline float active_water_spacing(float spacing, uint active_width, uint maximum_width)
{
    return spacing * float(max(maximum_width, 2u) - 1u) / float(max(active_width, 2u) - 1u);
}

kernel void marble_reset_state(
    device packed_float3 *player_positions [[buffer(0)]],
    device packed_float3 *player_velocities [[buffer(1)]],
    device packed_float3 *enemy_positions [[buffer(2)]],
    device packed_float3 *enemy_velocities [[buffer(3)]],
    device packed_float3 *water_positions [[buffer(4)]],
    device packed_float3 *water_velocities [[buffer(5)]],
    constant float &reset_scene [[buffer(6)]],
    constant float &reset_water [[buffer(7)]],
    constant uint &enemy_count [[buffer(8)]],
    constant uint &width [[buffer(9)]],
    constant uint &height [[buffer(10)]],
    constant float &surface_height [[buffer(11)]],
    constant float &marble_radius [[buffer(12)]],
    uint index [[thread_position_in_grid]])
{
    if (reset_scene < 0.5 && reset_water < 0.5) return;

    if (index < width * height) {
        water_positions[index] = packed_float3(0.0);
        water_velocities[index] = packed_float3(0.0);
    }
    if (reset_scene < 0.5) return;

    if (index == 0) {
        player_positions[0] = packed_float3(0.0, surface_height + marble_radius, 0.15);
        player_velocities[0] = packed_float3(0.0);
    }
    if (index < enemy_count) {
        constexpr float3 initial_enemies[8] = {
            float3(-1.08, 0.0, -0.72), float3(-0.72, 0.0, 0.72),
            float3(-0.34, 0.0, -0.82), float3(0.18, 0.0, 0.78),
            float3(0.58, 0.0, -0.70), float3(0.94, 0.0, 0.58),
            float3(-1.02, 0.0, 0.12), float3(1.08, 0.0, -0.08)
        };
        float3 enemy_position = initial_enemies[index];
        enemy_position.y = surface_height + marble_radius;
        enemy_positions[index] = packed_float3(enemy_position);
        enemy_velocities[index] = packed_float3(0.0);
    }
}

kernel void marble_player_step(
    device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    device packed_float3 *impact_positions [[buffer(2)]],
    device float *impact_speeds [[buffer(3)]],
    constant float &input_x [[buffer(4)]],
    constant float &input_z [[buffer(5)]],
    constant float &grab_active [[buffer(6)]],
    constant float &grab_x [[buffer(7)]],
    constant float &grab_y [[buffer(8)]],
    constant float &grab_z [[buffer(9)]],
    constant float &gravity [[buffer(10)]],
    constant float &surface_height [[buffer(11)]],
    constant float &radius [[buffer(12)]],
    constant float &ground_drag [[buffer(13)]],
    constant float &drive_acceleration [[buffer(14)]],
    constant float &maximum_speed [[buffer(15)]],
    constant float &plane_scale [[buffer(16)]],
    constant float &half_extent_x [[buffer(17)]],
    constant float &half_extent_z [[buffer(18)]],
    constant float &dt [[buffer(19)]],
    uint index [[thread_position_in_grid]])
{
    if (index > 0) return;
    float3 position = float3(positions[0]);
    float3 velocity = float3(velocities[0]);
    const float contact_height = surface_height + radius;

    if (grab_active >= 0.5) {
        position = float3(grab_x, max(grab_y, contact_height), grab_z);
        velocity = float3(0.0);
        positions[0] = packed_float3(position);
        velocities[0] = packed_float3(velocity);
        impact_positions[0] = packed_float3(0.0);
        impact_speeds[0] = 0.0;
        return;
    }

    const bool airborne =
        position.y > contact_height + 0.0005 || abs(velocity.y) > 0.0005;
    if (airborne) {
        velocity.y += gravity * dt;
        velocity.xz *= 0.999;
        position += velocity * dt;
        contain_marble(
            position,
            velocity,
            half_extent_x * plane_scale,
            half_extent_z * plane_scale,
            radius
        );

        float landing_speed = 0.0;
        if (position.y <= contact_height) {
            landing_speed = max(-velocity.y, 0.0);
            position.y = contact_height;
            velocity.y = 0.0;
        }
        positions[0] = packed_float3(position);
        velocities[0] = packed_float3(velocity);
        impact_positions[0] =
            landing_speed > 0.0
                ? packed_float3(position.x, surface_height, position.z)
                : packed_float3(0.0);
        impact_speeds[0] = landing_speed * 4.0;
        return;
    }

    const float2 input = float2(input_x, input_z);
    const float input_length = length(input);
    if (input_length > 0.0) {
        const float2 direction = input / max(input_length, 1.0);
        velocity.xz += direction * drive_acceleration * dt;
    }
    velocity.xz *= ground_drag;
    const float horizontal_speed = length(velocity.xz);
    if (horizontal_speed > maximum_speed) {
        velocity.xz *= maximum_speed / horizontal_speed;
    }
    position.xz += velocity.xz * dt;
    position.y = contact_height;
    velocity.y = 0.0;
    contain_marble(
        position,
        velocity,
        half_extent_x * plane_scale,
        half_extent_z * plane_scale,
        radius
    );

    positions[0] = packed_float3(position);
    velocities[0] = packed_float3(velocity);
    impact_positions[0] = packed_float3(position.x, surface_height, position.z);
    impact_speeds[0] = length(velocity.xz);
}

kernel void marble_enemy_step(
    device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    const device packed_float3 *player_positions [[buffer(2)]],
    device packed_float3 *impact_positions [[buffer(3)]],
    device float *impact_speeds [[buffer(4)]],
    constant uint &enemy_count [[buffer(5)]],
    constant float &surface_height [[buffer(6)]],
    constant float &radius [[buffer(7)]],
    constant float &chase_acceleration [[buffer(8)]],
    constant float &maximum_speed [[buffer(9)]],
    constant float &plane_scale [[buffer(10)]],
    constant float &half_extent_x [[buffer(11)]],
    constant float &half_extent_z [[buffer(12)]],
    constant float &dt [[buffer(13)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= enemy_count) return;
    float3 position = float3(positions[index]);
    float3 velocity = float3(velocities[index]);
    const float3 player = float3(player_positions[0]);
    const float2 offset = player.xz - position.xz;
    const float distance = length(offset);
    if (distance > 0.001) {
        const float2 direction = offset / distance;
        const float orbit_direction = (index & 1u) == 0u ? -1.0 : 1.0;
        const float2 orbit = float2(-direction.y, direction.x) * orbit_direction;
        velocity.xz += (direction + orbit * 0.22) * chase_acceleration * dt;
    }
    velocity.xz *= 0.995;
    const float horizontal_speed = length(velocity.xz);
    if (horizontal_speed > maximum_speed) velocity.xz *= maximum_speed / horizontal_speed;
    position.xz += velocity.xz * dt;
    position.y = surface_height + radius;
    velocity.y = 0.0;
    contain_marble(
        position,
        velocity,
        half_extent_x * plane_scale,
        half_extent_z * plane_scale,
        radius
    );

    positions[index] = packed_float3(position);
    velocities[index] = packed_float3(velocity);
    impact_positions[index] = packed_float3(position.x, surface_height, position.z);
    impact_speeds[index] = length(velocity.xz);
}

inline float impact_impulse(
    float2 point,
    float3 impact_position,
    float impact_speed,
    float radius)
{
    if (impact_speed <= 0.0) return 0.0;
    const float2 delta = point - impact_position.xz;
    const float normalized_distance = dot(delta, delta) / max(radius * radius, 1e-6);
    return impact_speed * exp(-normalized_distance * 3.0);
}

kernel void marble_water_force(
    const device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    const device packed_float3 *player_impact_positions [[buffer(2)]],
    const device float *player_impact_speeds [[buffer(3)]],
    const device packed_float3 *enemy_impact_positions [[buffer(4)]],
    const device float *enemy_impact_speeds [[buffer(5)]],
    constant uint &width [[buffer(6)]],
    constant uint &height [[buffer(7)]],
    constant uint &enemy_count [[buffer(8)]],
    constant float &spacing [[buffer(9)]],
    constant float &density [[buffer(10)]],
    constant float &plane_scale [[buffer(11)]],
    constant float &spring [[buffer(12)]],
    constant float &coupling [[buffer(13)]],
    constant float &damping [[buffer(14)]],
    constant float &impact_radius [[buffer(15)]],
    constant float &impact_gain [[buffer(16)]],
    constant float &dt [[buffer(17)]],
    uint index [[thread_position_in_grid]])
{
    const uint active_width = active_water_width(density, width);
    const uint active_height = active_water_height(active_width, width, height);
    const uint active_count = active_width * active_height;
    if (index >= active_count) {
        velocities[index] = packed_float3(0.0);
        return;
    }
    const uint x = index % active_width;
    const uint z = index / active_width;
    const float3 position = float3(positions[index]);
    float3 velocity = float3(velocities[index]);

    const float active_spacing =
        active_water_spacing(spacing, active_width, width) * plane_scale;
    const float rest_x = (float(x) - (float(active_width) - 1.0) * 0.5) * active_spacing;
    const float rest_z = (float(z) - (float(active_height) - 1.0) * 0.5) * active_spacing;
    const float2 rest_point = float2(rest_x, rest_z);

    float neighbor_sum = 0.0;
    uint neighbor_count = 0;
    if (x > 0) { neighbor_sum += float3(positions[index - 1]).y; neighbor_count++; }
    if (x + 1 < active_width) { neighbor_sum += float3(positions[index + 1]).y; neighbor_count++; }
    if (z > 0) { neighbor_sum += float3(positions[index - active_width]).y; neighbor_count++; }
    if (z + 1 < active_height) { neighbor_sum += float3(positions[index + active_width]).y; neighbor_count++; }
    const float neighbor_average = neighbor_sum / max(float(neighbor_count), 1.0);
    const float acceleration = -spring * position.y
        + coupling * (neighbor_average - position.y)
        - damping * velocity.y;
    velocity.y += acceleration * dt;

    float impulse = impact_impulse(rest_point, float3(player_impact_positions[0]),
                                   player_impact_speeds[0], impact_radius);
    for (uint enemy = 0; enemy < enemy_count; ++enemy) {
        impulse += impact_impulse(rest_point, float3(enemy_impact_positions[enemy]),
                                  enemy_impact_speeds[enemy], impact_radius);
    }
    velocity.y += impulse * impact_gain * dt;
    velocity.xz = 0.0;
    velocities[index] = packed_float3(velocity);
}

kernel void marble_compose_scene(
    device packed_float3 *render_positions [[buffer(0)]],
    device float *render_radii [[buffer(1)]],
    device float4 *render_colors [[buffer(2)]],
    device float *render_scales [[buffer(3)]],
    const device packed_float3 *player_positions [[buffer(4)]],
    const device packed_float3 *enemy_positions [[buffer(5)]],
    const device packed_float3 *water_positions [[buffer(6)]],
    constant uint &enemy_count [[buffer(7)]],
    constant uint &width [[buffer(8)]],
    constant uint &height [[buffer(9)]],
    constant float &spacing [[buffer(10)]],
    constant float &density [[buffer(11)]],
    constant float &plane_scale [[buffer(12)]],
    constant float &reset_scene [[buffer(13)]],
    constant float &hud_fps [[buffer(14)]],
    constant float &hud_gpu_mb [[buffer(15)]],
    constant float &marble_radius [[buffer(16)]],
    uint index [[thread_position_in_grid]])
{
    const uint active_width = active_water_width(density, width);
    const uint active_height = active_water_height(active_width, width, height);
    const uint active_count = active_width * active_height;
    const float active_spacing =
        active_water_spacing(spacing, active_width, width) * plane_scale;
    const uint water_capacity = width * height;
    render_scales[index] = plane_scale;
    if (index < water_capacity) {
        const uint water_index = index;
        if (water_index >= active_count) {
            render_positions[index] = packed_float3(0.0);
            render_radii[index] = 0.0;
            render_colors[index] = float4(0.0);
            return;
        }
        const uint x = water_index % active_width;
        const uint z = water_index / active_width;
        const float rest_x = (float(x) - (float(active_width) - 1.0) * 0.5) * active_spacing;
        const float rest_z = (float(z) - (float(active_height) - 1.0) * 0.5) * active_spacing;
        const float water_height = float3(water_positions[water_index]).y;
        render_positions[index] = packed_float3(rest_x, water_height, rest_z);
        render_radii[index] = clamp(active_spacing * 0.28, 0.003, 0.013)
            + min(abs(water_height) * 0.10, 0.008);
        const float crest = clamp(water_height * 8.0 + 0.35, 0.0, 1.0);
        render_colors[index] = float4(mix(float3(0.02, 0.25, 0.58), float3(0.18, 0.86, 1.0), crest), 0.82);
    } else if (index == water_capacity) {
        render_positions[index] = player_positions[0];
        render_radii[index] = marble_radius;
        render_colors[index] = float4(1.0, 0.86, 0.08, 1.0);
    } else if (index <= water_capacity + enemy_count) {
        render_positions[index] = enemy_positions[index - water_capacity - 1];
        render_radii[index] = marble_radius * 0.92;
        render_colors[index] = float4(0.96, 0.08, 0.12, 1.0);
    } else {
        float activity = 0.0;
        for (uint water_index = 0; water_index < active_count; ++water_index) {
            activity = max(activity, abs(float3(water_positions[water_index]).y));
        }
        render_positions[index] = packed_float3(
            clamp(density, 0.0, 1.0),
            clamp(activity * 12.0, 0.0, 1.0),
            clamp(reset_scene, 0.0, 1.0)
        );
        render_radii[index] = -1.0;
        render_colors[index] = float4(hud_fps, hud_gpu_mb, 0.0, 1.0);
    }
}
