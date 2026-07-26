#include <metal_stdlib>
using namespace metal;

inline float wrap_axis(float value, float extent)
{
    const float span = extent * 2.0;
    if (value > extent) return value - span;
    if (value < -extent) return value + span;
    return value;
}

kernel void baseline_simulate_particle(
    device packed_float3 *positions [[buffer(0)]],
    device packed_float3 *velocities [[buffer(1)]],
    constant float3 &gravity [[buffer(2)]],
    constant float &grab_active [[buffer(3)]],
    constant float &grab_x [[buffer(4)]],
    constant float &grab_y [[buffer(5)]],
    constant float &grab_z [[buffer(6)]],
    constant float &space_drag [[buffer(7)]],
    constant float &reset [[buffer(8)]],
    constant float &grab_spring [[buffer(9)]],
    constant float &grab_damping [[buffer(10)]],
    constant float &half_extent_x [[buffer(11)]],
    constant float &half_extent_y [[buffer(12)]],
    constant float &half_extent_z [[buffer(13)]],
    constant float &dt [[buffer(14)]],
    uint index [[thread_position_in_grid]])
{
    float3 position = float3(positions[index]);
    float3 velocity = float3(velocities[index]);

    if (reset > 0.5) {
        positions[index] = packed_float3(0.0);
        velocities[index] = packed_float3(0.0);
        return;
    }

    float3 acceleration = gravity;
    if (grab_active > 0.5) {
        const float3 target = float3(grab_x, grab_y, grab_z);
        acceleration +=
            (target - position) * grab_spring - velocity * grab_damping;
    } else {
        // Zero gravity preserves inertial motion. Optional drag is exposed by
        // the baseline UI but defaults to the vacuum value of zero.
        velocity *= exp(-max(space_drag, 0.0) * dt);
    }

    velocity += acceleration * dt;
    position += velocity * dt;

    // The viewer represents unbounded space as a wrapping volume, keeping the
    // lone particle available without adding collision forces.
    position.x = wrap_axis(position.x, half_extent_x);
    position.y = wrap_axis(position.y, half_extent_y);
    position.z = wrap_axis(position.z, half_extent_z);

    positions[index] = packed_float3(position);
    velocities[index] = packed_float3(velocity);
}

kernel void baseline_project_particle(
    const device packed_float3 *particle_positions [[buffer(0)]],
    device packed_float3 *render_positions [[buffer(1)]],
    device float *render_radii [[buffer(2)]],
    device float4 *render_colors [[buffer(3)]],
    device float *render_aspects [[buffer(4)]],
    constant float &radius [[buffer(5)]],
    constant float &aspect [[buffer(6)]],
    constant float &grabbed [[buffer(7)]],
    uint index [[thread_position_in_grid]])
{
    constexpr float camera_z = 3.0;
    constexpr float focal = 1.85;
    const float3 position = float3(particle_positions[index]);
    const float depth = max(camera_z - position.z, 0.1);
    const float safe_aspect = max(aspect, 0.1);

    render_positions[index] = packed_float3(
        position.x * focal / (depth * safe_aspect),
        position.y * focal / depth,
        depth
    );
    render_radii[index] = radius * focal / depth;
    render_aspects[index] = safe_aspect;
    render_colors[index] = mix(
        float4(0.18, 0.72, 1.0, 1.0),
        float4(0.46, 0.90, 1.0, 1.0),
        step(0.5, grabbed)
    );
}
