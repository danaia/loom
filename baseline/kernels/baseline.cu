#include <cuda_runtime.h>
#include <cfloat>

struct float3_packed {
    float x;
    float y;
    float z;
};

struct float4_packed {
    float x;
    float y;
    float z;
    float w;
};

static constexpr unsigned int PARTICLE_CAPACITY = 32;

__device__ float wrap_axis(float value, float extent)
{
    const float span = extent * 2.0f;
    if (value > extent) return value - span;
    if (value < -extent) return value + span;
    return value;
}

__device__ float clampf(float value, float low, float high)
{
    return fminf(fmaxf(value, low), high);
}

__device__ float3_packed make_vec3(float x, float y, float z)
{
    return float3_packed{x, y, z};
}

extern "C" __global__ void baseline_apply_interaction(
    float3_packed *positions,
    float3_packed *velocities,
    float *active,
    float3_packed *targets,
    float *target_active,
    float *particle_types,
    float *selected,
    float *spawn_seen,
    float *click_seen,
    const float *click_x,
    const float *click_y,
    const float *click_z,
    const float *click_generation,
    const float *spawn_x,
    const float *spawn_y,
    const float *spawn_z,
    const float *spawn_generation,
    const float *spawn_slot,
    const float *spawn_type,
    float *select_seen,
    float *remove_seen,
    const float *select_command,
    const float *remove_command,
    const float *selection_radius,
    const float *reset,
    float *dragging,
    const float *pointer_down,
    const float *drag_x,
    const float *drag_y,
    const float *drag_z)
{
    if (blockIdx.x != 0 || threadIdx.x != 0) return;

    if (*reset > 0.5f) {
        for (unsigned int i = 0; i < PARTICLE_CAPACITY; ++i) {
            positions[i] = make_vec3(0.0f, 0.0f, 0.0f);
            velocities[i] = make_vec3(0.0f, 0.0f, 0.0f);
            targets[i] = make_vec3(0.0f, 0.0f, 0.0f);
            active[i] = i == 0 ? 1.0f : 0.0f;
            target_active[i] = 0.0f;
            particle_types[i] = 0.0f;
        }
        selected[0] = 0.0f;
        spawn_seen[0] = *spawn_generation;
        click_seen[0] = *click_generation;
        select_seen[0] = *select_command;
        remove_seen[0] = *remove_command;
        dragging[0] = 0.0f;
        return;
    }

    if (remove_seen[0] != *remove_command) {
        const unsigned int slot =
            static_cast<unsigned int>(fmaxf(*remove_command, 0.0f)) % PARTICLE_CAPACITY;
        active[slot] = 0.0f;
        positions[slot] = make_vec3(0.0f, 0.0f, 0.0f);
        velocities[slot] = make_vec3(0.0f, 0.0f, 0.0f);
        targets[slot] = make_vec3(0.0f, 0.0f, 0.0f);
        target_active[slot] = 0.0f;
        dragging[0] = 0.0f;
        remove_seen[0] = *remove_command;
    }

    if (select_seen[0] != *select_command) {
        selected[0] =
            static_cast<float>(static_cast<unsigned int>(fmaxf(*select_command, 0.0f)) % PARTICLE_CAPACITY);
        dragging[0] = 0.0f;
        select_seen[0] = *select_command;
    }

    if (spawn_seen[0] != *spawn_generation) {
        const unsigned int slot =
            static_cast<unsigned int>(clampf(*spawn_slot, 0.0f, static_cast<float>(PARTICLE_CAPACITY - 1)));
        positions[slot] = make_vec3(*spawn_x, *spawn_y, *spawn_z);
        velocities[slot] = make_vec3(0.0f, 0.0f, 0.0f);
        targets[slot] = make_vec3(0.0f, 0.0f, 0.0f);
        active[slot] = 1.0f;
        target_active[slot] = 0.0f;
        particle_types[slot] = clampf(*spawn_type, 0.0f, 2.0f);
        selected[0] = static_cast<float>(slot);
        spawn_seen[0] = *spawn_generation;
    }

    if (*pointer_down <= 0.5f) {
        dragging[0] = 0.0f;
    }

    if (click_seen[0] != *click_generation) {
        float closest_distance = FLT_MAX;
        int closest = -1;
        for (unsigned int i = 0; i < PARTICLE_CAPACITY; ++i) {
            if (active[i] <= 0.5f) continue;
            const float dx = positions[i].x - *click_x;
            const float dy = positions[i].y - *click_y;
            const float distance_to_click = sqrtf(dx * dx + dy * dy);
            if (distance_to_click < closest_distance) {
                closest_distance = distance_to_click;
                closest = static_cast<int>(i);
            }
        }

        if (closest >= 0 && closest_distance <= *selection_radius) {
            selected[0] = static_cast<float>(closest);
            dragging[0] = 1.0f;
            target_active[closest] = 0.0f;
        } else {
            dragging[0] = 0.0f;
            const unsigned int slot =
                static_cast<unsigned int>(clampf(selected[0], 0.0f, static_cast<float>(PARTICLE_CAPACITY - 1)));
            if (active[slot] > 0.5f) {
                targets[slot] = make_vec3(*click_x, *click_y, *click_z);
                target_active[slot] = 1.0f;
            }
        }
        click_seen[0] = *click_generation;
    }

    if (*pointer_down > 0.5f && dragging[0] > 0.5f) {
        const unsigned int slot =
            static_cast<unsigned int>(clampf(selected[0], 0.0f, static_cast<float>(PARTICLE_CAPACITY - 1)));
        positions[slot] = make_vec3(*drag_x, *drag_y, *drag_z);
        velocities[slot] = make_vec3(0.0f, 0.0f, 0.0f);
        target_active[slot] = 0.0f;
    }
}

extern "C" __global__ void baseline_move_particle(
    float3_packed *positions,
    float3_packed *velocities,
    const float *active,
    float3_packed *targets,
    float *target_active,
    const float3_packed *gravity,
    const float *space_drag,
    const float *target_spring,
    const float *target_damping,
    const float *arrival_radius,
    const float *half_extent_x,
    const float *half_extent_y,
    const float *half_extent_z,
    const float *dt)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index >= PARTICLE_CAPACITY || active[index] <= 0.5f) return;

    float3_packed position = positions[index];
    float3_packed velocity = velocities[index];
    const float3_packed target = targets[index];

    if (target_active[index] > 0.5f) {
        const float dx = target.x - position.x;
        const float dy = target.y - position.y;
        const float dz = target.z - position.z;
        const float distance_to_target = sqrtf(dx * dx + dy * dy + dz * dz);
        if (distance_to_target <= *arrival_radius) {
            positions[index] = target;
            velocities[index] = make_vec3(0.0f, 0.0f, 0.0f);
            target_active[index] = 0.0f;
            return;
        }
        velocity.x += (gravity->x + dx * *target_spring - velocity.x * *target_damping) * *dt;
        velocity.y += (gravity->y + dy * *target_spring - velocity.y * *target_damping) * *dt;
        velocity.z += (gravity->z + dz * *target_spring - velocity.z * *target_damping) * *dt;
    } else {
        const float damping = expf(-fmaxf(*space_drag, 0.0f) * *dt);
        velocity.x = (velocity.x + gravity->x * *dt) * damping;
        velocity.y = (velocity.y + gravity->y * *dt) * damping;
        velocity.z = (velocity.z + gravity->z * *dt) * damping;
    }

    position.x = wrap_axis(position.x + velocity.x * *dt, *half_extent_x);
    position.y = wrap_axis(position.y + velocity.y * *dt, *half_extent_y);
    position.z = wrap_axis(position.z + velocity.z * *dt, *half_extent_z);
    positions[index] = position;
    velocities[index] = velocity;
}

extern "C" __global__ void baseline_project_particles(
    const float3_packed *particle_positions,
    const float *particle_active,
    const float3_packed *particle_targets,
    const float *particle_target_active,
    const float *particle_types,
    const float *selected,
    float3_packed *render_positions,
    float *render_radii,
    float4_packed *render_colors,
    float *render_aspects,
    const float *radius,
    const float *target_radius,
    const float *aspect)
{
    const unsigned int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index >= PARTICLE_CAPACITY * 2) return;

    constexpr float camera_z = 3.0f;
    constexpr float focal = 1.85f;
    const unsigned int particle_index = index % PARTICLE_CAPACITY;
    const bool is_target = index >= PARTICLE_CAPACITY;
    const bool visible = particle_active[particle_index] > 0.5f
        && (!is_target || particle_target_active[particle_index] > 0.5f);
    const float3_packed position =
        is_target ? particle_targets[particle_index] : particle_positions[particle_index];
    const float depth = fmaxf(camera_z - position.z, 0.1f);
    const float safe_aspect = fmaxf(*aspect, 0.1f);
    const bool is_selected = !is_target && static_cast<unsigned int>(selected[0]) == particle_index;

    render_positions[index] = make_vec3(
        position.x * focal / (depth * safe_aspect),
        position.y * focal / depth,
        depth
    );
    render_radii[index] = visible
        ? (is_target ? *target_radius : *radius) * focal / depth
        : 0.0f;
    render_aspects[index] = safe_aspect;

    const unsigned int agent_type =
        static_cast<unsigned int>(clampf(particle_types[particle_index], 0.0f, 2.0f));
    float4_packed type_color = {0.18f, 0.72f, 1.0f, 1.0f};
    if (agent_type == 1) {
        type_color = {0.48f, 0.92f, 0.48f, 1.0f};
    } else if (agent_type == 2) {
        type_color = {0.78f, 0.48f, 1.0f, 1.0f};
    }

    render_colors[index] = is_target
        ? float4_packed{1.0f, 0.72f, 0.18f, 1.0f}
        : (is_selected
            ? float4_packed{
                type_color.x * 0.65f + 0.35f,
                type_color.y * 0.65f + 0.35f,
                type_color.z * 0.65f + 0.35f,
                1.0f}
            : type_color);
}
