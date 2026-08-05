#include <cuda_runtime.h>

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

constexpr unsigned int PQO_BLOCK_WIDTH = 256;
constexpr unsigned int PQO_ATOM_FIELD_WIDTH = 100;
constexpr unsigned int PQO_CLUSTER_EDGE = 4;
constexpr unsigned int PQO_CLUSTERS_PER_AXIS = 25;

__device__ __forceinline__ unsigned int pqo_dispatch_count(
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    return dynamic_count == 0
        ? maximum_count
        : static_cast<unsigned int>(min(
              dynamic_count,
              static_cast<unsigned long long>(maximum_count)));
}

__device__ __forceinline__ float hydrogen_1s_density(
    const float radius,
    const float electron_count,
    const float bohr_radius)
{
    constexpr float inverse_pi = 0.3183098861837907f;
    const float inverse_a0 = 1.0f / bohr_radius;
    const float normalization = electron_count * inverse_pi
        * inverse_a0 * inverse_a0 * inverse_a0;
    return normalization * expf(-2.0f * radius * inverse_a0);
}

extern "C" __global__ void baseline_cuda_reset_observables(
    float* total_probability,
    float* radial_moment,
    unsigned int* active_lod_counts,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = pqo_dispatch_count(dynamic_count, maximum_count);
    if (index >= count) return;
    active_lod_counts[index] = 0;
    if (index == 0) {
        total_probability[0] = 0.0f;
        radial_moment[0] = 0.0f;
    }
}

extern "C" __global__ void baseline_cuda_sample_hydrogen_1s(
    const PqoFloat3* atom_position,
    const float* electron_count,
    const float* bohr_radius,
    const unsigned int* width,
    const float* half_extent,
    float* electron_density,
    float* total_probability,
    float* radial_moment,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    __shared__ float probability_sums[PQO_BLOCK_WIDTH];
    __shared__ float radial_moment_sums[PQO_BLOCK_WIDTH];

    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int local_index = threadIdx.x;
    const unsigned int count = pqo_dispatch_count(dynamic_count, maximum_count);
    const bool active = index < count;

    float probability = 0.0f;
    float radial_probability = 0.0f;
    if (active) {
        // This optimized baseline deliberately freezes the 100^3 specialization.
        // The value binding is retained so the graph records the physical shape.
        const unsigned int grid_width = min(width[0], PQO_ATOM_FIELD_WIDTH);
        const unsigned int plane = grid_width * grid_width;
        const unsigned int x = index % grid_width;
        const unsigned int y = (index / grid_width) % grid_width;
        const unsigned int z = index / plane;
        const float spacing = (2.0f * half_extent[0]) / static_cast<float>(grid_width);
        const PqoFloat3 center = atom_position[0];
        const float sample_x = -half_extent[0] + (static_cast<float>(x) + 0.5f) * spacing;
        const float sample_y = -half_extent[0] + (static_cast<float>(y) + 0.5f) * spacing;
        const float sample_z = -half_extent[0] + (static_cast<float>(z) + 0.5f) * spacing;
        const float dx = sample_x - center.x;
        const float dy = sample_y - center.y;
        const float dz = sample_z - center.z;
        const float radius = sqrtf(dx * dx + dy * dy + dz * dz);
        const float density = hydrogen_1s_density(
            radius,
            electron_count[0],
            bohr_radius[0]);
        const float voxel_volume = spacing * spacing * spacing;
        probability = density * voxel_volume;
        radial_probability = probability * radius;
        electron_density[index] = density;
    }

    probability_sums[local_index] = probability;
    radial_moment_sums[local_index] = radial_probability;
    __syncthreads();

    // One million samples become 3,907 block contributions, avoiding a global
    // atomic operation per voxel. The CUDA runtime supplies 256-thread blocks.
    for (unsigned int stride = PQO_BLOCK_WIDTH / 2; stride > 0; stride >>= 1) {
        if (local_index < stride) {
            probability_sums[local_index] += probability_sums[local_index + stride];
            radial_moment_sums[local_index] += radial_moment_sums[local_index + stride];
        }
        __syncthreads();
    }
    if (local_index == 0) {
        atomicAdd(total_probability, probability_sums[0]);
        atomicAdd(radial_moment, radial_moment_sums[0]);
    }
}

extern "C" __global__ void baseline_cuda_classify_density_clusters(
    const float* electron_density,
    const float* electron_count,
    const float* bohr_radius,
    const unsigned int* width,
    const float* half_extent,
    const float* lod0_density_ratio,
    const float* lod1_density_ratio,
    const float* lod2_density_ratio,
    const float* visible_density_ratio,
    float* cluster_max_density,
    float* cluster_probability,
    unsigned int* cluster_lod,
    unsigned int* cluster_visible,
    unsigned int* active_lod_counts,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = pqo_dispatch_count(dynamic_count, maximum_count);
    if (index >= count) return;

    const unsigned int grid_width = min(width[0], PQO_ATOM_FIELD_WIDTH);
    const unsigned int cluster_x = index % PQO_CLUSTERS_PER_AXIS;
    const unsigned int cluster_y = (index / PQO_CLUSTERS_PER_AXIS) % PQO_CLUSTERS_PER_AXIS;
    const unsigned int cluster_z = index / (PQO_CLUSTERS_PER_AXIS * PQO_CLUSTERS_PER_AXIS);
    float maximum_density = 0.0f;
    float density_sum = 0.0f;

    #pragma unroll
    for (unsigned int dz = 0; dz < PQO_CLUSTER_EDGE; ++dz) {
        #pragma unroll
        for (unsigned int dy = 0; dy < PQO_CLUSTER_EDGE; ++dy) {
            #pragma unroll
            for (unsigned int dx = 0; dx < PQO_CLUSTER_EDGE; ++dx) {
                const unsigned int x = cluster_x * PQO_CLUSTER_EDGE + dx;
                const unsigned int y = cluster_y * PQO_CLUSTER_EDGE + dy;
                const unsigned int z = cluster_z * PQO_CLUSTER_EDGE + dz;
                const unsigned int voxel = x + grid_width * (y + grid_width * z);
                const float density = electron_density[voxel];
                maximum_density = fmaxf(maximum_density, density);
                density_sum += density;
            }
        }
    }

    const float spacing = (2.0f * half_extent[0]) / static_cast<float>(grid_width);
    const float voxel_volume = spacing * spacing * spacing;
    const float peak_density = hydrogen_1s_density(
        0.0f,
        electron_count[0],
        bohr_radius[0]);
    const float relative_density = maximum_density / peak_density;
    const bool visible = relative_density >= visible_density_ratio[0];
    unsigned int lod = 3;
    if (relative_density >= lod0_density_ratio[0]) {
        lod = 0;
    } else if (relative_density >= lod1_density_ratio[0]) {
        lod = 1;
    } else if (relative_density >= lod2_density_ratio[0]) {
        lod = 2;
    }

    cluster_max_density[index] = maximum_density;
    cluster_probability[index] = density_sum * voxel_volume;
    cluster_lod[index] = lod;
    cluster_visible[index] = visible ? 1u : 0u;
    if (visible) atomicAdd(&active_lod_counts[lod], 1u);
}

extern "C" __global__ void baseline_cuda_project_atom(
    const PqoFloat3* atom_position,
    const float* bohr_radius,
    const PqoFloat4* atom_color,
    const float* probability_radius_bohr,
    PqoFloat3* presentation_position,
    float* presentation_radius,
    PqoFloat4* presentation_color,
    unsigned int* presentation_lod,
    unsigned int* presentation_visible,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned int count = pqo_dispatch_count(dynamic_count, maximum_count);
    if (index >= count) return;
    presentation_position[index] = atom_position[index];
    presentation_radius[index] = probability_radius_bohr[0] * bohr_radius[index];
    presentation_color[index] = atom_color[index];
    presentation_lod[index] = 0;
    presentation_visible[index] = 1;
}
