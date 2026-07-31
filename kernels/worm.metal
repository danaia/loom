#include <metal_stdlib>
using namespace metal;

#define WORM_SEGMENTS 24u
#define FOOD_CAPACITY 12u
#define RENDER_COUNT (1u + FOOD_CAPACITY + WORM_SEGMENTS)
#define PI 3.14159265358979323846f

inline float3 rotate_for_camera(float3 world, float yaw, float pitch) {
    float cy = cos(yaw);
    float sy = sin(yaw);
    float3 yawed = float3(
        cy * world.x + sy * world.z,
        world.y,
        -sy * world.x + cy * world.z);
    float cp = cos(pitch);
    float sp = sin(pitch);
    return float3(
        yawed.x,
        cp * yawed.y - sp * yawed.z,
        sp * yawed.y + cp * yawed.z);
}

kernel void worm_think_and_move(
    device packed_float3* position [[buffer(0)]],
    device packed_float3* heading [[buffer(1)]],
    device float* smell_strength [[buffer(2)]],
    device uint* meals [[buffer(3)]],
    const device packed_float3* food_position [[buffer(4)]],
    device uint* food_active [[buffer(5)]],
    device uint* tick [[buffer(6)]],
    constant float& fixed_dt [[buffer(7)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid != 0u) return;

    float3 head = float3(position[0]);
    float2 forward = normalize(float2(heading[0].x, heading[0].z));
    float best_distance = 1.0e9f;
    float2 desired = forward;
    bool scented = false;

    for (uint i = 0u; i < FOOD_CAPACITY; ++i) {
        if (food_active[i] == 0u) continue;
        float2 delta = float2(food_position[i].x - head.x, food_position[i].z - head.z);
        float distance_to_food = length(delta);
        if (distance_to_food < best_distance) {
            best_distance = distance_to_food;
            desired = normalize(delta + float2(1.0e-6f, 0.0f));
            scented = true;
        }
    }

    float time = float(tick[0]) * fixed_dt;
    if (!scented) {
        float wander = sin(time * 0.73f) * 0.65f + sin(time * 0.19f + 2.1f) * 0.35f;
        float angle = atan2(forward.y, forward.x) + wander * fixed_dt;
        desired = float2(cos(angle), sin(angle));
        smell_strength[0] *= 0.94f;
    } else {
        smell_strength[0] = clamp(1.25f / (0.18f + best_distance), 0.0f, 5.0f);
    }

    // A Pqo worm remembers its current heading and turns deliberately instead
    // of snapping to a target. Closer scents increase its confidence and speed.
    float turn_rate = scented ? clamp(0.07f + smell_strength[0] * 0.018f, 0.07f, 0.16f) : 0.035f;
    forward = normalize(mix(forward, desired, turn_rate));

    // Anticipate the plane edge and blend an avoidance intent into navigation.
    float edge = max(abs(head.x), abs(head.z));
    if (edge > 0.78f) {
        float2 home = normalize(-head.xz);
        forward = normalize(mix(forward, home, smoothstep(0.78f, 0.95f, edge)));
    }

    float2 lateral = float2(-forward.y, forward.x);
    float intelligence_wiggle = sin(time * 6.2f) * 0.11f;
    float speed = scented ? 0.31f + min(smell_strength[0], 2.0f) * 0.035f : 0.19f;
    head.xz += normalize(forward + lateral * intelligence_wiggle) * speed * fixed_dt;
    head.xz = clamp(head.xz, float2(-0.92f), float2(0.92f));
    head.y = 0.066f + sin(time * 7.0f) * 0.005f;
    position[0] = packed_float3(head);
    heading[0] = packed_float3(float3(forward.x, 0.0f, forward.y));

    // Sequential body relaxation preserves the connected, articulated animal.
    constexpr float spacing = 0.052f;
    for (uint i = 1u; i < WORM_SEGMENTS; ++i) {
        float3 previous = float3(position[i - 1u]);
        float3 current = float3(position[i]);
        float3 delta = previous - current;
        float distance_to_previous = max(length(delta.xz), 1.0e-5f);
        float correction = (distance_to_previous - spacing) * 0.72f;
        current.xz += normalize(delta.xz) * correction;
        float wave = sin(time * 7.0f - float(i) * 0.58f);
        current.y = 0.058f + wave * 0.007f * (1.0f - float(i) / float(WORM_SEGMENTS));
        position[i] = packed_float3(current);
    }

    // Eating is an authoritative state transition: the food disappears and
    // the worm's memory of meals increments.
    for (uint i = 0u; i < FOOD_CAPACITY; ++i) {
        if (food_active[i] == 0u) continue;
        float distance_to_food = distance(float2(food_position[i].x, food_position[i].z), head.xz);
        if (distance_to_food < 0.075f) {
            food_active[i] = 0u;
            meals[0] += 1u;
            smell_strength[0] = 0.0f;
        }
    }
    tick[0] += 1u;
}

kernel void worm_drop_food(
    device packed_float3* food_position [[buffer(0)]],
    device uint* food_active [[buffer(1)]],
    device uint* drop_cursor [[buffer(2)]],
    const device float* camera_yaw [[buffer(3)]],
    const device float* camera_pitch [[buffer(4)]],
    const device float* camera_zoom [[buffer(5)]],
    constant float& pick_x [[buffer(6)]],
    constant float& pick_y [[buffer(7)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid != 0u) return;

    // Invert the same camera/projection used by the renderer and intersect the
    // click ray with y=0. This keeps food attached to the plane at every orbit.
    float yaw = camera_yaw[0];
    float pitch = camera_pitch[0];
    float zoom = camera_zoom[0];
    float cy = cos(yaw);
    float sy = sin(yaw);
    float cp = cos(pitch);
    float sp = sin(pitch);

    float vx_x = zoom * cy;
    float vx_z = zoom * sy;
    float vy_x = zoom * sp * sy;
    float vy_z = -zoom * sp * cy;
    float vz_x = -zoom * cp * sy;
    float vz_z = zoom * cp * cy;

    float a1 = 0.84f * vx_x + (0.42f - 0.24f * pick_x) * vz_x;
    float b1 = 0.84f * vx_z + (0.42f - 0.24f * pick_x) * vz_z;
    float a2 = vy_x - 0.24f * vx_x + (0.34f - 0.24f * pick_y) * vz_x;
    float b2 = vy_z - 0.24f * vx_z + (0.34f - 0.24f * pick_y) * vz_z;
    float r1 = pick_x * 1.16f;
    float r2 = pick_y * 1.16f;
    float determinant = a1 * b2 - b1 * a2;
    if (abs(determinant) < 1.0e-5f) return;

    float x = (r1 * b2 - b1 * r2) / determinant;
    float z = (a1 * r2 - r1 * a2) / determinant;
    if (abs(x) > 1.0f || abs(z) > 1.0f) return;
    uint slot = drop_cursor[0] % FOOD_CAPACITY;
    food_position[slot] = packed_float3(x, 0.047f, z);
    food_active[slot] = 1u;
    drop_cursor[0] += 1u;
}

kernel void worm_orbit_camera(
    device float* camera_yaw [[buffer(0)]],
    device float* camera_pitch [[buffer(1)]],
    constant float& delta_yaw [[buffer(2)]],
    constant float& delta_pitch [[buffer(3)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid != 0u) return;
    float yaw = camera_yaw[0] + delta_yaw;
    camera_yaw[0] = atan2(sin(yaw), cos(yaw));
    camera_pitch[0] = clamp(camera_pitch[0] + delta_pitch, 0.18f, 1.18f);
}

kernel void worm_zoom_camera(
    device float* camera_zoom [[buffer(0)]],
    constant float& zoom_delta [[buffer(1)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid == 0u) {
        camera_zoom[0] = clamp(camera_zoom[0] * exp(zoom_delta), 0.52f, 1.42f);
    }
}

kernel void worm_prepare_render(
    const device packed_float3* worm_position [[buffer(0)]],
    const device float* smell_strength [[buffer(1)]],
    const device uint* meals [[buffer(2)]],
    const device packed_float3* food_position [[buffer(3)]],
    const device uint* food_active [[buffer(4)]],
    const device float* camera_yaw [[buffer(5)]],
    const device float* camera_pitch [[buffer(6)]],
    const device float* camera_zoom [[buffer(7)]],
    device packed_float3* render_position [[buffer(8)]],
    device float4* render_color [[buffer(9)]],
    device float* render_radius [[buffer(10)]],
    device uint* render_kind [[buffer(11)]],
    uint gid [[thread_position_in_grid]])
{
    float yaw = camera_yaw[0];
    float pitch = camera_pitch[0];
    float zoom = camera_zoom[0];

    if (gid == 0u) {
        render_position[gid] = packed_float3(yaw, pitch, zoom);
        render_color[gid] = float4(0.055f, 0.10f, 0.075f, 1.0f);
        render_radius[gid] = 1.0f;
        render_kind[gid] = 0u;
        return;
    }

    if (gid <= FOOD_CAPACITY) {
        uint food = gid - 1u;
        render_kind[gid] = 2u;
        if (food_active[food] == 0u) {
            render_position[gid] = packed_float3(0.0f);
            render_color[gid] = float4(0.0f);
            render_radius[gid] = 0.0f;
            return;
        }
        float3 view = rotate_for_camera(float3(food_position[food]), yaw, pitch) * zoom;
        render_position[gid] = packed_float3(view);
        render_color[gid] = float4(0.92f, 0.30f, 0.15f, 1.0f);
        render_radius[gid] = 0.047f * zoom;
        return;
    }

    uint segment = gid - 1u - FOOD_CAPACITY;
    float t = float(segment) / float(WORM_SEGMENTS - 1u);
    float3 view = rotate_for_camera(float3(worm_position[segment]), yaw, pitch) * zoom;
    float scent = clamp(smell_strength[0] * 0.18f, 0.0f, 0.55f);
    float3 body = mix(float3(0.12f, 0.82f, 0.62f), float3(0.47f, 0.21f, 0.92f), t);
    body = mix(body, float3(0.77f, 1.0f, 0.36f), scent * (1.0f - t));
    if (segment == 0u) {
        body = mix(body, float3(0.88f, 1.0f, 0.44f), 0.36f);
    }
    float meal_pulse = sin(float(meals[0]) * 2.3f) * 0.008f;
    render_position[gid] = packed_float3(view);
    render_color[gid] = float4(body, 1.0f);
    render_radius[gid] = (mix(0.073f, 0.037f, t) + meal_pulse * (1.0f - t)) * zoom;
    render_kind[gid] = 1u;
}
