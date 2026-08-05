#include <cuda_runtime.h>

// Pqo f32x3/f32x4 values are tightly packed. Keep these structs at the ABI
// boundary instead of relying on host compiler vector alignment.
struct PqoFloat3 {
    float x;
    float y;
    float z;
};

struct PqoFloat4 {
    float x;
    float y;
    float z;
    float w;
};

static_assert(sizeof(PqoFloat3) == 12, "Pqo f32x3 must occupy 12 bytes");
static_assert(sizeof(PqoFloat4) == 16, "Pqo f32x4 must occupy 16 bytes");

__device__ __forceinline__ unsigned int pqo_dispatch_count(
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    // Fixed-length dispatches pass zero. A dynamic dispatch passes a device
    // pointer through this ABI slot and is intentionally handled by generated
    // Pqo kernels until the external-CUDA ABI exposes a dereference helper.
    return dynamic_count == 0
        ? maximum_count
        : static_cast<unsigned int>(min(
              dynamic_count,
              static_cast<unsigned long long>(maximum_count)));
}

extern "C" __global__ void baseline_cuda_clear_lod_counts(
    unsigned int* active_lod_counts,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = pqo_dispatch_count(dynamic_count, maximum_count);
    if (index >= count) return;
    active_lod_counts[index] = 0;
}

extern "C" __global__ void baseline_cuda_classify_for_presentation(
    const PqoFloat3* particle_position,
    const float* particle_radius,
    const PqoFloat4* particle_color,
    const PqoFloat3* camera_position,
    const float* cull_distance,
    const float* lod0_distance,
    const float* lod1_distance,
    const float* lod2_distance,
    PqoFloat3* presentation_position,
    float* presentation_radius,
    PqoFloat4* presentation_color,
    unsigned int* presentation_lod,
    unsigned int* presentation_visible,
    unsigned int* active_lod_counts,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = pqo_dispatch_count(dynamic_count, maximum_count);
    if (index >= count) return;

    const PqoFloat3 position = particle_position[index];
    const float dx = position.x - camera_position[0].x;
    const float dy = position.y - camera_position[0].y;
    const float dz = position.z - camera_position[0].z;
    const float distance_squared = dx * dx + dy * dy + dz * dz;
    const float cull_distance_squared = cull_distance[0] * cull_distance[0];
    const bool visible = distance_squared <= cull_distance_squared;

    unsigned int lod = 3;
    if (visible) {
        if (distance_squared <= lod0_distance[0] * lod0_distance[0]) {
            lod = 0;
        } else if (distance_squared <= lod1_distance[0] * lod1_distance[0]) {
            lod = 1;
        } else if (distance_squared <= lod2_distance[0] * lod2_distance[0]) {
            lod = 2;
        }
    }

    presentation_position[index] = position;
    presentation_radius[index] = visible ? particle_radius[index] : 0.0f;
    presentation_color[index] = particle_color[index];
    presentation_lod[index] = lod;
    presentation_visible[index] = visible ? 1u : 0u;
    if (visible) atomicAdd(&active_lod_counts[lod], 1u);
}
