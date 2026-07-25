#include <metal_stdlib>
using namespace metal;

kernel void integrate_main(
    device packed_float3 *position [[buffer(0)]],
    device packed_float3 *velocity [[buffer(1)]],
    constant packed_float3 &gravity [[buffer(2)]],
    constant float &dt [[buffer(3)]],
    uint index [[thread_position_in_grid]])
{
    float3 v = float3(velocity[index]) + float3(gravity) * dt;
    float3 p = float3(position[index]) + v * dt;
    velocity[index] = packed_float3(v);
    position[index] = packed_float3(p);
}
