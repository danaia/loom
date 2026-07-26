#include <metal_stdlib>
using namespace metal;

kernel void neon_flock_neighborhood_main(
    device packed_float2 *positions [[buffer(0)]],
    device packed_float2 *velocities [[buffer(1)]],
    device packed_float2 *trail_positions [[buffer(2)]],
    device packed_float2 *accelerations [[buffer(3)]],
    constant uint &population [[buffer(4)]],
    constant float &initial_radius [[buffer(5)]],
    constant float &initial_speed [[buffer(6)]],
    constant float &perception_radius [[buffer(7)]],
    constant float &separation_radius [[buffer(8)]],
    constant float &cohesion_gain [[buffer(9)]],
    constant float &alignment_gain [[buffer(10)]],
    constant float &separation_strength [[buffer(11)]],
    constant float &wander_acceleration [[buffer(12)]],
    constant float &startup_acceleration [[buffer(13)]],
    constant float &swirl_gain [[buffer(14)]],
    constant float &maximum_acceleration [[buffer(15)]],
    constant float &maximum_speed [[buffer(16)]],
    constant float &half_extent [[buffer(17)]],
    constant float &boundary_margin [[buffer(18)]],
    constant float &boundary_gain [[buffer(19)]],
    constant float &dt [[buffer(20)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= population) {
        return;
    }

    float2 position = float2(positions[index]);
    float2 velocity = float2(velocities[index]);
    const bool uninitialized =
        dot(position, position) < 1e-12 && dot(velocity, velocity) < 1e-12;
    if (uninitialized) {
        const float fraction = (float(index) + 0.5) / float(population);
        const float angle = float(index) * 2.39996322973;
        const float radius = initial_radius * sqrt(fraction);
        position = float2(cos(angle), sin(angle)) * radius;
        const float2 tangent = normalize(float2(-position.y, position.x));
        velocity = tangent * initial_speed;
        positions[index] = packed_float2(position);
        velocities[index] = packed_float2(velocity);
        trail_positions[index] = packed_float2(position);
        accelerations[index] = packed_float2(0.0);
        return;
    }
    const float perception_squared = perception_radius * perception_radius;
    const float separation_squared = separation_radius * separation_radius;

    float2 center_sum = 0.0;
    float2 velocity_sum = 0.0;
    float2 separation_sum = 0.0;
    uint neighbors = 0;

    for (uint neighbor = 0; neighbor < population; ++neighbor) {
        if (neighbor == index) {
            continue;
        }
        const float2 offset = float2(positions[neighbor]) - position;
        const float distance_squared = dot(offset, offset);
        if (distance_squared > 1e-8 && distance_squared < perception_squared) {
            center_sum += float2(positions[neighbor]);
            velocity_sum += float2(velocities[neighbor]);
            neighbors += 1;
            if (distance_squared < separation_squared) {
                separation_sum -= offset / max(distance_squared, 1e-5);
            }
        }
    }

    float2 steering = 0.0;
    if (neighbors > 0) {
        const float inverse_neighbors = 1.0 / float(neighbors);
        const float2 center = center_sum * inverse_neighbors;
        const float2 average_velocity = velocity_sum * inverse_neighbors;
        steering += (center - position) * cohesion_gain;
        steering += (average_velocity - velocity) * alignment_gain;
        steering += separation_sum * inverse_neighbors * separation_strength;
    }

    // A deterministic low-amplitude current prevents a frozen symmetric state
    // without introducing hidden CPU initialization or random state.
    const float seed = fract(sin((float(index) + 1.0) * 12.9898) * 43758.5453);
    const float angle = seed * 6.28318530718;
    const float2 wander_direction = float2(cos(angle), sin(angle));
    const bool starting = dot(position, position) < 4e-6 && dot(velocity, velocity) < 0.01;
    steering += wander_direction * (starting ? startup_acceleration : wander_acceleration);
    steering += float2(-position.y, position.x) * swirl_gain;

    const float inner_extent = half_extent - boundary_margin;
    if (position.x > inner_extent) {
        steering.x -= (position.x - inner_extent) * boundary_gain;
    } else if (position.x < -inner_extent) {
        steering.x -= (position.x + inner_extent) * boundary_gain;
    }
    if (position.y > inner_extent) {
        steering.y -= (position.y - inner_extent) * boundary_gain;
    } else if (position.y < -inner_extent) {
        steering.y -= (position.y + inner_extent) * boundary_gain;
    }

    const float acceleration_length = length(steering);
    if (acceleration_length > maximum_acceleration) {
        steering *= maximum_acceleration / acceleration_length;
    }

    const float2 predicted_velocity = velocity + steering * dt;
    const float predicted_speed = length(predicted_velocity);
    if (predicted_speed > maximum_speed) {
        const float2 limited_velocity = predicted_velocity * (maximum_speed / predicted_speed);
        steering = (limited_velocity - velocity) / max(dt, 1e-6);
    }

    accelerations[index] = packed_float2(steering);
}
