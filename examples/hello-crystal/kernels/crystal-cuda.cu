#include <cuda_runtime.h>

// The Pqo CUDA ABI appends `dynamic_count` and `maximum_count` to the declared
// kernel bindings. This simulation has a fixed one-million-cell domain, but
// honoring both fields keeps the kernel valid for the generic CUDA runner.
extern "C" __global__ void crystal_step(
    float* phase,
    float* solute,
    float* temperature,
    unsigned int* tick,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = dynamic_count == 0
        ? maximum_count
        : static_cast<unsigned int>(min(dynamic_count, static_cast<unsigned long long>(maximum_count)));
    if (index >= count) {
        return;
    }

    // The 100^3 field uses the same cell-centre coordinate system as the
    // Metal crystal. A growing rotated Wulff shape acts as the equilibrium
    // crystal; concentration and temperature relax around its interface.
    constexpr unsigned int width = 100;
    constexpr unsigned int plane = width * width;
    const unsigned int x = index % width;
    const unsigned int y = (index / width) % width;
    const unsigned int z = index / plane;
    const float3 center = make_float3(
        static_cast<float>(x) + 0.5f - 50.0f,
        static_cast<float>(y) + 0.5f - 50.0f,
        static_cast<float>(z) + 0.5f - 50.0f);
    const float3 q = make_float3(center.x / 100.0f, center.y / 100.0f, center.z / 100.0f);
    constexpr float cs = 0.92490906f; // cos(0.39)
    constexpr float sn = 0.38018842f; // sin(0.39)
    const float3 crystal_q = make_float3(
        cs * q.x + sn * q.z,
        q.y,
        -sn * q.x + cs * q.z);
    const float3 absolute_q = make_float3(
        fabsf(crystal_q.x), fabsf(crystal_q.y), fabsf(crystal_q.z));
    const float axial = fmaxf(absolute_q.x, fmaxf(absolute_q.y, absolute_q.z)) / 0.205f;
    const float diagonal = (absolute_q.x + absolute_q.y + absolute_q.z) / 0.345f;
    const float extent = fmaxf(axial, diagonal);

    const unsigned int frame = tick[0];
    const float growth = fminf(1.0f, 0.045f + static_cast<float>(frame) * 0.0025f);
    const float target_phase = extent <= growth ? 1.0f : 0.0f;
    const float current = phase[index];
    const float next_phase = current + (target_phase - current) * 0.11f;
    const float solidified = fmaxf(next_phase - current, 0.0f);

    phase[index] = next_phase;
    solute[index] = fminf(1.25f, fmaxf(0.0f, solute[index] + (1.0f - solute[index]) * 0.012f - solidified * 0.62f));
    temperature[index] = fminf(1.0f, fmaxf(0.0f, temperature[index] * 0.998f + solidified * 0.13f));
    if (index == 0) {
        tick[0] = frame + 1;
    }
}
