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

inline float sample_water_height(
    const device packed_float3 *water_positions,
    float2 point,
    uint width,
    uint height,
    float spacing,
    float density,
    float plane_scale)
{
    const uint active_width = active_water_width(density, width);
    const uint active_height = active_water_height(active_width, width, height);
    const float active_spacing =
        active_water_spacing(spacing, active_width, width) * plane_scale;
    const float2 grid_point = float2(
        point.x / active_spacing + (float(active_width) - 1.0) * 0.5,
        point.y / active_spacing + (float(active_height) - 1.0) * 0.5
    );
    const float2 bounded = clamp(
        grid_point,
        float2(0.0),
        float2(float(active_width - 1u), float(active_height - 1u))
    );
    const uint2 lower = uint2(floor(bounded));
    const uint2 upper = min(
        lower + uint2(1u),
        uint2(active_width - 1u, active_height - 1u)
    );
    const float2 blend = fract(bounded);
    const float h00 =
        float3(water_positions[lower.y * active_width + lower.x]).y;
    const float h10 =
        float3(water_positions[lower.y * active_width + upper.x]).y;
    const float h01 =
        float3(water_positions[upper.y * active_width + lower.x]).y;
    const float h11 =
        float3(water_positions[upper.y * active_width + upper.x]).y;
    return mix(mix(h00, h10, blend.x), mix(h01, h11, blend.x), blend.y);
}

inline float submerged_volume_fraction(
    float center_height,
    float water_surface,
    float radius)
{
    const float cap_height = clamp(
        water_surface - (center_height - radius),
        0.0,
        radius * 2.0
    );
    return cap_height * cap_height * (3.0 * radius - cap_height)
        / max(4.0 * radius * radius * radius, 1e-6);
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
        player_positions[0] = packed_float3(0.0, surface_height, 0.15);
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
        enemy_position.y = surface_height;
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
    const device packed_float3 *water_positions [[buffer(20)]],
    constant uint &water_width [[buffer(21)]],
    constant uint &water_height [[buffer(22)]],
    constant float &water_spacing [[buffer(23)]],
    constant float &water_density [[buffer(24)]],
    constant float &density_ratio [[buffer(25)]],
    constant float &vertical_drag [[buffer(26)]],
    uint index [[thread_position_in_grid]])
{
    if (index > 0) return;
    float3 position = float3(positions[0]);
    float3 velocity = float3(velocities[0]);
    float local_water_surface = surface_height + sample_water_height(
        water_positions,
        position.xz,
        water_width,
        water_height,
        water_spacing,
        water_density,
        plane_scale
    );

    if (grab_active >= 0.5) {
        position = float3(grab_x, max(grab_y, local_water_surface), grab_z);
        velocity = float3(0.0);
        positions[0] = packed_float3(position);
        velocities[0] = packed_float3(velocity);
        impact_positions[0] = packed_float3(0.0);
        impact_speeds[0] = 0.0;
        return;
    }

    const bool was_above_water =
        position.y - radius >= local_water_surface;
    const float submerged_fraction = submerged_volume_fraction(
        position.y,
        local_water_surface,
        radius
    );
    // Archimedes buoyancy: at density_ratio=0.5, half of the sphere's
    // displaced volume exactly balances its weight.
    const float buoyancy_acceleration =
        -gravity * submerged_fraction / max(density_ratio, 0.05);
    velocity.y += (
        gravity
        + buoyancy_acceleration
        - velocity.y * vertical_drag * submerged_fraction
    ) * dt;
    const float2 input = float2(input_x, input_z);
    const float input_length = length(input);
    if (input_length > 0.0) {
        const float2 direction = input / max(input_length, 1.0);
        velocity.xz += direction * drive_acceleration * dt;
    }
    velocity.xz *= mix(0.9995, ground_drag, submerged_fraction);
    const float horizontal_speed = length(velocity.xz);
    if (horizontal_speed > maximum_speed) {
        velocity.xz *= maximum_speed / horizontal_speed;
    }
    position += velocity * dt;
    contain_marble(
        position,
        velocity,
        half_extent_x * plane_scale,
        half_extent_z * plane_scale,
        radius
    );
    local_water_surface = surface_height + sample_water_height(
        water_positions,
        position.xz,
        water_width,
        water_height,
        water_spacing,
        water_density,
        plane_scale
    );
    const bool entered_water =
        was_above_water && position.y - radius < local_water_surface;
    const float entry_speed =
        entered_water ? max(-velocity.y, 0.0) : 0.0;

    positions[0] = packed_float3(position);
    velocities[0] = packed_float3(velocity);
    impact_positions[0] =
        entry_speed > 0.0
            ? packed_float3(position.x, local_water_surface, position.z)
            : packed_float3(0.0);
    // Negative values are one-frame water-entry impulses. Continuous wakes
    // are derived directly from the body's velocity in the water pass.
    impact_speeds[0] = -entry_speed;
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
    constant float &gravity [[buffer(14)]],
    const device packed_float3 *water_positions [[buffer(15)]],
    constant uint &water_width [[buffer(16)]],
    constant uint &water_height [[buffer(17)]],
    constant float &water_spacing [[buffer(18)]],
    constant float &water_density [[buffer(19)]],
    constant float &density_ratio [[buffer(20)]],
    constant float &vertical_drag [[buffer(21)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= enemy_count) return;
    float3 position = float3(positions[index]);
    float3 velocity = float3(velocities[index]);
    const float3 player = float3(player_positions[0]);
    float local_water_surface = surface_height + sample_water_height(
        water_positions,
        position.xz,
        water_width,
        water_height,
        water_spacing,
        water_density,
        plane_scale
    );
    const bool was_above_water =
        position.y - radius >= local_water_surface;
    const float submerged_fraction = submerged_volume_fraction(
        position.y,
        local_water_surface,
        radius
    );
    const float buoyancy_acceleration =
        -gravity * submerged_fraction / max(density_ratio, 0.05);
    velocity.y += (
        gravity
        + buoyancy_acceleration
        - velocity.y * vertical_drag * submerged_fraction
    ) * dt;
    const float2 offset = player.xz - position.xz;
    const float distance = length(offset);
    if (distance > 0.001) {
        const float2 direction = offset / distance;
        const float orbit_direction = (index & 1u) == 0u ? -1.0 : 1.0;
        const float2 orbit = float2(-direction.y, direction.x) * orbit_direction;
        velocity.xz += (direction + orbit * 0.22) * chase_acceleration * dt;
    }
    velocity.xz *= mix(0.9995, 0.995, submerged_fraction);
    const float horizontal_speed = length(velocity.xz);
    if (horizontal_speed > maximum_speed) velocity.xz *= maximum_speed / horizontal_speed;
    position += velocity * dt;
    contain_marble(
        position,
        velocity,
        half_extent_x * plane_scale,
        half_extent_z * plane_scale,
        radius
    );
    local_water_surface = surface_height + sample_water_height(
        water_positions,
        position.xz,
        water_width,
        water_height,
        water_spacing,
        water_density,
        plane_scale
    );
    const bool entered_water =
        was_above_water && position.y - radius < local_water_surface;
    const float entry_speed =
        entered_water ? max(-velocity.y, 0.0) : 0.0;

    positions[index] = packed_float3(position);
    velocities[index] = packed_float3(velocity);
    impact_positions[index] =
        entry_speed > 0.0
            ? packed_float3(position.x, local_water_surface, position.z)
            : packed_float3(0.0);
    impact_speeds[index] = -entry_speed;
}

kernel void marble_resolve_interactions(
    device packed_float3 *player_positions [[buffer(0)]],
    device packed_float3 *player_velocities [[buffer(1)]],
    device packed_float3 *enemy_positions [[buffer(2)]],
    device packed_float3 *enemy_velocities [[buffer(3)]],
    device packed_float3 *player_impact_positions [[buffer(4)]],
    device float *player_impact_speeds [[buffer(5)]],
    device packed_float3 *enemy_impact_positions [[buffer(6)]],
    device float *enemy_impact_speeds [[buffer(7)]],
    constant uint &enemy_count [[buffer(8)]],
    constant float &surface_height [[buffer(9)]],
    constant float &radius [[buffer(10)]],
    constant float &restitution [[buffer(11)]],
    constant float &collision_wave_gain [[buffer(12)]],
    constant float &mass_kg [[buffer(13)]],
    uint index [[thread_position_in_grid]])
{
    if (index > 0) return;
    constexpr uint maximum_marbles = 9u;
    float3 positions[maximum_marbles];
    float3 velocities[maximum_marbles];
    const uint marble_count = min(enemy_count + 1u, maximum_marbles);
    positions[0] = float3(player_positions[0]);
    velocities[0] = float3(player_velocities[0]);
    for (uint enemy = 0; enemy < enemy_count; ++enemy) {
        positions[enemy + 1u] = float3(enemy_positions[enemy]);
        velocities[enemy + 1u] = float3(enemy_velocities[enemy]);
    }

    float collision_energy[maximum_marbles] = {
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
    };
    const float minimum_separation = radius * 2.0;
    for (uint first = 0; first < marble_count; ++first) {
        for (uint second = first + 1u; second < marble_count; ++second) {
            const float vertical_separation =
                abs(positions[second].y - positions[first].y);
            if (vertical_separation >= minimum_separation) continue;
            const float horizontal_separation = sqrt(max(
                minimum_separation * minimum_separation
                    - vertical_separation * vertical_separation,
                0.0
            ));
            float2 offset = positions[second].xz - positions[first].xz;
            float distance = length(offset);
            if (distance >= horizontal_separation) continue;

            float2 normal;
            if (distance > 1e-5) {
                normal = offset / distance;
            } else {
                const float angle =
                    float(first * 7u + second * 13u) * 0.61803398875;
                normal = float2(cos(angle), sin(angle));
                distance = 0.0;
            }
            const float overlap = horizontal_separation - distance;
            positions[first].xz -= normal * overlap * 0.5;
            positions[second].xz += normal * overlap * 0.5;

            const float closing_speed =
                dot(velocities[second].xz - velocities[first].xz, normal);
            if (closing_speed < 0.0) {
                const float inverse_mass = 1.0 / max(mass_kg, 1e-4);
                const float impulse_momentum =
                    -(1.0 + restitution) * closing_speed
                    / (inverse_mass + inverse_mass);
                const float delta_speed = impulse_momentum * inverse_mass;
                velocities[first].xz -= normal * delta_speed;
                velocities[second].xz += normal * delta_speed;
                collision_energy[first] =
                    max(
                        collision_energy[first],
                        mass_kg * delta_speed * collision_wave_gain
                    );
                collision_energy[second] =
                    max(
                        collision_energy[second],
                        mass_kg * delta_speed * collision_wave_gain
                    );
            }
        }
    }

    player_positions[0] = packed_float3(positions[0]);
    player_velocities[0] = packed_float3(velocities[0]);
    if (collision_energy[0] > 0.0 && player_impact_speeds[0] >= 0.0) {
        player_impact_positions[0] =
            packed_float3(positions[0].x, surface_height, positions[0].z);
        player_impact_speeds[0] =
            max(player_impact_speeds[0], collision_energy[0]);
    }
    for (uint enemy = 0; enemy < enemy_count; ++enemy) {
        const uint marble = enemy + 1u;
        enemy_positions[enemy] = packed_float3(positions[marble]);
        enemy_velocities[enemy] = packed_float3(velocities[marble]);
        if (collision_energy[marble] > 0.0) {
            enemy_impact_positions[enemy] = packed_float3(
                positions[marble].x,
                surface_height,
                positions[marble].z
            );
            enemy_impact_speeds[enemy] = max(
                enemy_impact_speeds[enemy],
                collision_energy[marble]
            );
        }
    }
}

inline float collision_impulse(
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

inline float directional_body_wake(
    float2 point,
    float3 body_position,
    float3 body_velocity,
    float radius,
    float submerged_fraction)
{
    const float speed = length(body_velocity.xz);
    if (speed < 0.003 || submerged_fraction <= 0.0) return 0.0;

    const float2 direction = body_velocity.xz / speed;
    const float2 lateral_axis = float2(-direction.y, direction.x);
    const float2 normalized_offset =
        (point - body_position.xz) / max(radius, 1e-4);
    const float along = dot(normalized_offset, direction);
    const float lateral = dot(normalized_offset, lateral_axis);
    const float envelope = exp(
        -1.35 * along * along - 2.4 * lateral * lateral
    );

    // The odd profile is volume-balanced: a compressed bow crest is paired
    // with a trailing trough. The wave solver carries both away as a wake.
    return speed
        * clamp(along * 1.8, -1.0, 1.0)
        * envelope
        * submerged_fraction;
}

inline float floating_body_acceleration(
    float2 point,
    float water_height,
    float3 body_position,
    float3 body_velocity,
    float surface_height,
    float radius,
    float density_ratio,
    float body_coupling)
{
    const float distance_from_body =
        length(point - body_position.xz) / max(radius, 1e-4);
    if (distance_from_body >= 2.4) return 0.0;

    const float local_surface = surface_height + water_height;
    const float submerged_fraction = submerged_volume_fraction(
        body_position.y,
        local_surface,
        radius
    );
    if (submerged_fraction <= 0.0) return 0.0;

    // A floating sphere excludes a depression-sized volume beneath its
    // waterline and pushes that volume into a surrounding shoulder. Keeping
    // this as a spring target lets the surface relax and radiate naturally.
    const float crater =
        exp(-3.0 * distance_from_body * distance_from_body);
    const float rim_offset = distance_from_body - 1.15;
    const float displaced_rim = 0.22 * exp(-9.0 * rim_offset * rim_offset);
    const float displacement_scale =
        radius * 0.10 * submerged_fraction / max(density_ratio, 0.05);
    const float target_height =
        displacement_scale * (displaced_rim - crater);
    const float footprint = clamp(crater + displaced_rim, 0.0, 1.0);
    const float heave_transfer =
        -body_velocity.y * crater * submerged_fraction * 2.2;
    return (target_height - water_height)
            * body_coupling * footprint
        + heave_transfer;
}

inline float drop_impulse(
    float2 point,
    float3 impact_position,
    float impact_speed,
    float radius)
{
    if (impact_speed >= 0.0) return 0.0;
    const float distance_from_impact =
        length(point - impact_position.xz) / max(radius, 1e-4);
    if (distance_from_impact >= 2.4) return 0.0;

    // A pebble first displaces water down at the contact point and up into a
    // narrow rim. The nearly volume-balanced profile becomes the first
    // crest/trough pair that the shallow-water solver propagates outward.
    const float crater =
        exp(-3.0 * distance_from_impact * distance_from_impact);
    const float rim_offset = distance_from_impact - 1.15;
    const float displaced_rim = 0.22 * exp(-9.0 * rim_offset * rim_offset);
    return (-impact_speed) * (displaced_rim - crater);
}

kernel void marble_water_force(
    const device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    const device packed_float3 *player_impact_positions [[buffer(2)]],
    const device float *player_impact_speeds [[buffer(3)]],
    const device packed_float3 *enemy_impact_positions [[buffer(4)]],
    const device float *enemy_impact_speeds [[buffer(5)]],
    const device packed_float3 *player_positions [[buffer(6)]],
    const device packed_float3 *player_velocities [[buffer(7)]],
    const device packed_float3 *enemy_positions [[buffer(8)]],
    const device packed_float3 *enemy_velocities [[buffer(9)]],
    constant uint &width [[buffer(10)]],
    constant uint &height [[buffer(11)]],
    constant uint &enemy_count [[buffer(12)]],
    constant float &spacing [[buffer(13)]],
    constant float &density [[buffer(14)]],
    constant float &plane_scale [[buffer(15)]],
    constant float &amplification [[buffer(16)]],
    constant float &wave_speed [[buffer(17)]],
    constant float &damping [[buffer(18)]],
    constant float &impact_radius [[buffer(19)]],
    constant float &impact_gain [[buffer(20)]],
    constant float &wake_gain [[buffer(21)]],
    constant float &surface_height [[buffer(22)]],
    constant float &marble_radius [[buffer(23)]],
    constant float &density_ratio [[buffer(24)]],
    constant float &body_coupling [[buffer(25)]],
    constant float &dt [[buffer(26)]],
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
    const float amplification_amount = clamp(amplification, 0.0, 1.0);
    const float ripple_gain = 1.0 + 2.5 * pow(amplification_amount, 1.25);
    const float ripple_radius =
        impact_radius * (1.0 + 0.25 * amplification_amount);
    const float ripple_wave_speed =
        wave_speed * (1.0 + 0.12 * amplification_amount);
    const float ripple_damping =
        damping * (1.0 - 0.35 * amplification_amount);

    // Nine-point, spacing-aware Laplacian for the 2D shallow-water wave
    // equation. The old neighbor average omitted 1 / spacing^2, making wave
    // speed depend on particle density and effectively trapping disturbances
    // beside the marble.
    const float center = position.y;
    const float left =
        x > 0 ? float3(positions[index - 1]).y : center;
    const float right =
        x + 1 < active_width ? float3(positions[index + 1]).y : center;
    const float near =
        z > 0 ? float3(positions[index - active_width]).y : center;
    const float far =
        z + 1 < active_height ? float3(positions[index + active_width]).y : center;
    const float near_left =
        x > 0 && z > 0
            ? float3(positions[index - active_width - 1]).y
            : center;
    const float near_right =
        x + 1 < active_width && z > 0
            ? float3(positions[index - active_width + 1]).y
            : center;
    const float far_left =
        x > 0 && z + 1 < active_height
            ? float3(positions[index + active_width - 1]).y
            : center;
    const float far_right =
        x + 1 < active_width && z + 1 < active_height
            ? float3(positions[index + active_width + 1]).y
            : center;
    const float inverse_spacing_squared =
        1.0 / max(active_spacing * active_spacing, 1e-6);
    const float laplacian = (
        4.0 * (left + right + near + far)
        + near_left + near_right + far_left + far_right
        - 20.0 * center
    ) * (inverse_spacing_squared / 6.0);

    // Absorb outgoing waves over the outer particle band instead of reflecting
    // a hard square echo back through the pool.
    const uint edge_x = min(x, active_width - 1u - x);
    const uint edge_z = min(z, active_height - 1u - z);
    const float edge_distance = float(min(edge_x, edge_z));
    const float edge_absorption =
        4.0 * (1.0 - smoothstep(0.0, 10.0, edge_distance));
    const float acceleration =
        ripple_wave_speed * ripple_wave_speed * laplacian
        - (ripple_damping + edge_absorption) * velocity.y;
    velocity.y += acceleration * dt;

    const float player_impact_speed = player_impact_speeds[0];
    const float landing_impulse = drop_impulse(
        rest_point,
        float3(player_impact_positions[0]),
        player_impact_speed,
        ripple_radius
    );
    // Landing is an instantaneous momentum transfer, so it is deliberately
    // not multiplied by dt. This produces one coherent expanding ring.
    velocity.y += landing_impulse * impact_gain * ripple_gain;

    float collision_wake = collision_impulse(
        rest_point,
        float3(player_impact_positions[0]),
        player_impact_speed,
        ripple_radius
    );
    for (uint enemy = 0; enemy < enemy_count; ++enemy) {
        collision_wake += collision_impulse(
            rest_point,
            float3(enemy_impact_positions[enemy]),
            enemy_impact_speeds[enemy],
            ripple_radius
        );
    }

    const float3 player_position = float3(player_positions[0]);
    const float3 player_velocity = float3(player_velocities[0]);
    const float player_submerged = submerged_volume_fraction(
        player_position.y,
        surface_height + position.y,
        marble_radius
    );
    float motion_wake = directional_body_wake(
        rest_point,
        player_position,
        player_velocity,
        marble_radius,
        player_submerged
    );
    float body_acceleration = floating_body_acceleration(
        rest_point,
        position.y,
        player_position,
        player_velocity,
        surface_height,
        marble_radius,
        density_ratio,
        body_coupling
    );
    for (uint enemy = 0; enemy < enemy_count; ++enemy) {
        const float3 enemy_position = float3(enemy_positions[enemy]);
        const float3 enemy_velocity = float3(enemy_velocities[enemy]);
        const float enemy_submerged = submerged_volume_fraction(
            enemy_position.y,
            surface_height + position.y,
            marble_radius
        );
        motion_wake += directional_body_wake(
            rest_point,
            enemy_position,
            enemy_velocity,
            marble_radius,
            enemy_submerged
        );
        body_acceleration += floating_body_acceleration(
            rest_point,
            position.y,
            enemy_position,
            enemy_velocity,
            surface_height,
            marble_radius,
            density_ratio,
            body_coupling
        );
    }
    velocity.y += (
        (collision_wake + motion_wake) * wake_gain * ripple_gain
        + body_acceleration
    ) * dt;
    velocity.y = clamp(velocity.y, -1.5, 1.5);
    velocity.xz = 0.0;
    velocities[index] = packed_float3(velocity);
}

kernel void marble_wave_response(
    const device packed_float3 *player_positions [[buffer(0)]],
    device packed_float3 *player_velocities [[buffer(1)]],
    const device packed_float3 *enemy_positions [[buffer(2)]],
    device packed_float3 *enemy_velocities [[buffer(3)]],
    const device packed_float3 *water_positions [[buffer(4)]],
    constant uint &enemy_count [[buffer(5)]],
    constant uint &width [[buffer(6)]],
    constant uint &height [[buffer(7)]],
    constant float &spacing [[buffer(8)]],
    constant float &density [[buffer(9)]],
    constant float &plane_scale [[buffer(10)]],
    constant float &response_acceleration [[buffer(11)]],
    constant float &dt [[buffer(12)]],
    uint index [[thread_position_in_grid]])
{
    if (index > 0) return;
    const uint active_width = active_water_width(density, width);
    const float sample_offset =
        active_water_spacing(spacing, active_width, width) * plane_scale;

    float3 player_velocity = float3(player_velocities[0]);
    const float2 player_point = float3(player_positions[0]).xz;
    const float2 player_gradient = float2(
        sample_water_height(
            water_positions, player_point + float2(sample_offset, 0.0),
            width, height, spacing, density, plane_scale
        ) - sample_water_height(
            water_positions, player_point - float2(sample_offset, 0.0),
            width, height, spacing, density, plane_scale
        ),
        sample_water_height(
            water_positions, player_point + float2(0.0, sample_offset),
            width, height, spacing, density, plane_scale
        ) - sample_water_height(
            water_positions, player_point - float2(0.0, sample_offset),
            width, height, spacing, density, plane_scale
        )
    ) / max(2.0 * sample_offset, 1e-4);
    player_velocity.xz -=
        player_gradient * response_acceleration * dt;
    player_velocities[0] = packed_float3(player_velocity);

    for (uint enemy = 0; enemy < enemy_count; ++enemy) {
        float3 enemy_velocity = float3(enemy_velocities[enemy]);
        const float2 enemy_point = float3(enemy_positions[enemy]).xz;
        const float2 enemy_gradient = float2(
            sample_water_height(
                water_positions, enemy_point + float2(sample_offset, 0.0),
                width, height, spacing, density, plane_scale
            ) - sample_water_height(
                water_positions, enemy_point - float2(sample_offset, 0.0),
                width, height, spacing, density, plane_scale
            ),
            sample_water_height(
                water_positions, enemy_point + float2(0.0, sample_offset),
                width, height, spacing, density, plane_scale
            ) - sample_water_height(
                water_positions, enemy_point - float2(0.0, sample_offset),
                width, height, spacing, density, plane_scale
            )
        ) / max(2.0 * sample_offset, 1e-4);
        enemy_velocity.xz -=
            enemy_gradient * response_acceleration * dt;
        enemy_velocities[enemy] = packed_float3(enemy_velocity);
    }
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
    constant float &marble_radius [[buffer(13)]],
    constant float &surface_height [[buffer(14)]],
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
        const float wave_energy = clamp(abs(water_height) * 42.0, 0.0, 1.0);
        render_radii[index] = clamp(active_spacing * 0.28, 0.003, 0.013)
            + min(abs(water_height) * 0.24, 0.014);
        const float crest = clamp(water_height * 28.0 + 0.34, 0.0, 1.0);
        const float3 height_color = mix(
            float3(0.01, 0.12, 0.40),
            float3(0.16, 0.88, 1.0),
            crest
        );
        render_colors[index] = float4(
            mix(height_color, float3(0.72, 0.98, 1.0), wave_energy * 0.38),
            0.84
        );
    } else if (index == water_capacity) {
        // The simulated body center already follows the local surface through
        // gravity and buoyancy, so rendering it directly avoids double-bobbing.
        render_positions[index] = player_positions[0];
        render_radii[index] = marble_radius;
        const float3 player_position = float3(player_positions[0]);
        const float player_waterline = clamp(
            (
                surface_height
                + sample_water_height(
                    water_positions,
                    player_position.xz,
                    width,
                    height,
                    spacing,
                    density,
                    plane_scale
                )
                - player_position.y
            ) / max(marble_radius, 1e-4),
            -1.0,
            1.0
        );
        const float player_immersion_code =
            player_waterline > -0.999 ? 2.0 + player_waterline : 1.0;
        render_colors[index] =
            float4(1.0, 0.86, 0.08, player_immersion_code);
    } else if (index <= water_capacity + enemy_count) {
        const float3 enemy_position =
            float3(enemy_positions[index - water_capacity - 1]);
        render_positions[index] = packed_float3(enemy_position);
        render_radii[index] = marble_radius * 0.92;
        const float enemy_waterline = clamp(
            (
                surface_height
                + sample_water_height(
                    water_positions,
                    enemy_position.xz,
                    width,
                    height,
                    spacing,
                    density,
                    plane_scale
                )
                - enemy_position.y
            ) / max(marble_radius * 0.92, 1e-4),
            -1.0,
            1.0
        );
        const float enemy_immersion_code =
            enemy_waterline > -0.999 ? 2.0 + enemy_waterline : 1.0;
        render_colors[index] =
            float4(0.96, 0.08, 0.12, enemy_immersion_code);
    }
}
