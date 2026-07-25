#include <metal_stdlib>
using namespace metal;

kernel void ground_contact_main(
    device packed_float3 *position [[buffer(0)]],
    device packed_float3 *velocity [[buffer(1)]],
    const device float *radius [[buffer(2)]],
    const device float *restitution [[buffer(3)]],
    const device float *friction [[buffer(4)]],
    constant float &ground_height [[buffer(5)]],
    uint index [[thread_position_in_grid]])
{
    float3 p = float3(position[index]);
    float3 v = float3(velocity[index]);
    float floor_y = ground_height + radius[index];
    if (p.y < floor_y) {
        p.y = floor_y;
        if (v.y < 0.0) {
            v.y = -v.y * restitution[index];
            float damping = clamp(1.0 - friction[index], 0.0, 1.0);
            v.xz *= damping;
        }
    }
    position[index] = packed_float3(p);
    velocity[index] = packed_float3(v);
}
