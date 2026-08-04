#include <cuda_runtime.h>

extern "C" __global__ void crystal_clear_hierarchy(
    unsigned int* cluster_population,
    unsigned int* cluster_lod,
    unsigned int* cluster_visible,
    unsigned int* active_lod_counts,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = dynamic_count == 0 ? maximum_count :
        static_cast<unsigned int>(min(dynamic_count, static_cast<unsigned long long>(maximum_count)));
    if (index >= count) return;
    cluster_population[index] = 0;
    cluster_lod[index] = 3;
    cluster_visible[index] = 0;
    if (index < 4) active_lod_counts[index] = 0;
}

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

extern "C" __global__ void crystal_finalize_hierarchy(
    unsigned int* cluster_population,
    unsigned int* cluster_lod,
    unsigned int* cluster_visible,
    unsigned int* active_lod_counts,
    const unsigned int* tick,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = dynamic_count == 0 ? maximum_count :
        static_cast<unsigned int>(min(dynamic_count, static_cast<unsigned long long>(maximum_count)));
    if (index >= count) return;
    if (index == 0) active_lod_counts[3] = count;
    constexpr unsigned int clusters_per_axis = 25;
    const unsigned int cluster_x = index % clusters_per_axis;
    const unsigned int cluster_y = (index / clusters_per_axis) % clusters_per_axis;
    const unsigned int cluster_z = index / (clusters_per_axis * clusters_per_axis);
    const float3 q = make_float3(
        (static_cast<float>(cluster_x * 4) + 2.0f - 50.0f) / 100.0f,
        (static_cast<float>(cluster_y * 4) + 2.0f - 50.0f) / 100.0f,
        (static_cast<float>(cluster_z * 4) + 2.0f - 50.0f) / 100.0f);
    constexpr float cs = 0.92490906f;
    constexpr float sn = 0.38018842f;
    const float3 crystal_q = make_float3(cs * q.x + sn * q.z, q.y, -sn * q.x + cs * q.z);
    const float3 absolute_q = make_float3(fabsf(crystal_q.x), fabsf(crystal_q.y), fabsf(crystal_q.z));
    const float axial = fmaxf(absolute_q.x, fmaxf(absolute_q.y, absolute_q.z)) / 0.205f;
    const float diagonal = (absolute_q.x + absolute_q.y + absolute_q.z) / 0.345f;
    const float extent = fmaxf(axial, diagonal);
    const float growth = fminf(1.0f, 0.045f + static_cast<float>(tick[0]) * 0.0025f);
    const unsigned int population = extent <= growth * 0.88f ? 64u : extent <= growth ? 16u : 0u;
    cluster_population[index] = population;
    if (population == 0) {
        cluster_lod[index] = 3;
        cluster_visible[index] = 0;
        return;
    }
    const unsigned int lod = population >= 48 ? 0 : population >= 16 ? 1 : 2;
    cluster_lod[index] = lod;
    cluster_visible[index] = 1;
    atomicAdd(&active_lod_counts[lod], 1u);
}
