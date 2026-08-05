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

__device__ __forceinline__ PqoFloat3 add(const PqoFloat3 a, const PqoFloat3 b)
{
    return {a.x + b.x, a.y + b.y, a.z + b.z};
}

__device__ __forceinline__ PqoFloat3 subtract(const PqoFloat3 a, const PqoFloat3 b)
{
    return {a.x - b.x, a.y - b.y, a.z - b.z};
}

__device__ __forceinline__ PqoFloat3 rotate_by_quaternion(
    const PqoFloat3 vector,
    const PqoFloat4 quaternion)
{
    const float norm_squared = quaternion.x * quaternion.x
        + quaternion.y * quaternion.y
        + quaternion.z * quaternion.z
        + quaternion.w * quaternion.w;
    const float inverse_norm = rsqrtf(fmaxf(norm_squared, 1.0e-20f));
    const float qx = quaternion.x * inverse_norm;
    const float qy = quaternion.y * inverse_norm;
    const float qz = quaternion.z * inverse_norm;
    const float qw = quaternion.w * inverse_norm;
    const float tx = 2.0f * (qy * vector.z - qz * vector.y);
    const float ty = 2.0f * (qz * vector.x - qx * vector.z);
    const float tz = 2.0f * (qx * vector.y - qy * vector.x);
    return {
        vector.x + qw * tx + (qy * tz - qz * ty),
        vector.y + qw * ty + (qz * tx - qx * tz),
        vector.z + qw * tz + (qx * ty - qy * tx),
    };
}

__device__ __forceinline__ float length(const PqoFloat3 value)
{
    return sqrtf(value.x * value.x + value.y * value.y + value.z * value.z);
}

__device__ __forceinline__ float dot(const PqoFloat3 a, const PqoFloat3 b)
{
    return a.x * b.x + a.y * b.y + a.z * b.z;
}

extern "C" __global__ void water_reconstruct_rigid_3site(
    const unsigned int* model,
    const PqoFloat3* molecule_position,
    const PqoFloat4* molecule_orientation,
    const float* oh_distance,
    const float* hoh_angle_degrees,
    const float* oxygen_charge_e,
    const float* hydrogen_charge_e,
    PqoFloat3* atom_position,
    float* atom_radius,
    unsigned int* atom_element,
    float* atom_charge_e,
    PqoFloat3* bond_start,
    PqoFloat3* bond_end,
    float* oh1_distance,
    float* oh2_distance,
    float* reconstructed_angle_degrees,
    float* geometry_error,
    float* net_charge_e,
    float* dipole_e_m,
    const unsigned long long dynamic_count,
    const unsigned int maximum_count)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index >= maximum_count || (dynamic_count != 0 && index >= dynamic_count)) return;
    if (index != 0 || model[0] != 1) return;

    constexpr float pi = 3.14159265358979323846f;
    const float half_angle = 0.5f * hoh_angle_degrees[0] * pi / 180.0f;
    const float local_x = oh_distance[0] * sinf(half_angle);
    const float local_z = oh_distance[0] * cosf(half_angle);
    const PqoFloat3 oxygen = molecule_position[0];
    const PqoFloat3 hydrogen_1 = add(
        oxygen,
        rotate_by_quaternion({local_x, 0.0f, local_z}, molecule_orientation[0]));
    const PqoFloat3 hydrogen_2 = add(
        oxygen,
        rotate_by_quaternion({-local_x, 0.0f, local_z}, molecule_orientation[0]));

    atom_position[0] = oxygen;
    atom_position[1] = hydrogen_1;
    atom_position[2] = hydrogen_2;
    // Deliberately visible presentation radii; these are not nuclear radii.
    atom_radius[0] = 6.0e-11f;
    atom_radius[1] = 3.7e-11f;
    atom_radius[2] = 3.7e-11f;
    atom_element[0] = 8;
    atom_element[1] = 1;
    atom_element[2] = 1;
    atom_charge_e[0] = oxygen_charge_e[0];
    atom_charge_e[1] = hydrogen_charge_e[0];
    atom_charge_e[2] = hydrogen_charge_e[0];
    bond_start[0] = oxygen;
    bond_start[1] = oxygen;
    bond_end[0] = hydrogen_1;
    bond_end[1] = hydrogen_2;

    const PqoFloat3 oh1 = subtract(hydrogen_1, oxygen);
    const PqoFloat3 oh2 = subtract(hydrogen_2, oxygen);
    const float distance_1 = length(oh1);
    const float distance_2 = length(oh2);
    const float inverse_lengths = 1.0f / fmaxf(distance_1 * distance_2, 1.0e-30f);
    const float cosine = fminf(1.0f, fmaxf(-1.0f, dot(oh1, oh2) * inverse_lengths));
    const float measured_angle = acosf(cosine) * 180.0f / pi;
    oh1_distance[0] = distance_1;
    oh2_distance[0] = distance_2;
    reconstructed_angle_degrees[0] = measured_angle;
    const float relative_distance_error = fmaxf(
        fabsf(distance_1 - oh_distance[0]),
        fabsf(distance_2 - oh_distance[0])) / oh_distance[0];
    const float relative_angle_error = fabsf(measured_angle - hoh_angle_degrees[0])
        / hoh_angle_degrees[0];
    geometry_error[0] = fmaxf(relative_distance_error, relative_angle_error);
    net_charge_e[0] = oxygen_charge_e[0] + 2.0f * hydrogen_charge_e[0];
    const PqoFloat3 dipole = {
        oxygen_charge_e[0] * oxygen.x
            + hydrogen_charge_e[0] * (hydrogen_1.x + hydrogen_2.x),
        oxygen_charge_e[0] * oxygen.y
            + hydrogen_charge_e[0] * (hydrogen_1.y + hydrogen_2.y),
        oxygen_charge_e[0] * oxygen.z
            + hydrogen_charge_e[0] * (hydrogen_1.z + hydrogen_2.z),
    };
    dipole_e_m[0] = length(dipole);
}
