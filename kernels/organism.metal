#include <metal_stdlib>
using namespace metal;

constant uint LOOM_Q16 = 65536;
constant uint LOOM_DECISION_MAX = 4095;

inline uint field_index(float2 position, uint width) {
    const float2 normalized = clamp(position * 0.45f + 0.5f, 0.0f, 0.999999f);
    const uint2 cell = uint2(normalized * float(width));
    return cell.y * width + cell.x;
}

inline uint decision_bin(float value, float maximum) {
    return uint(round(clamp(value / maximum, 0.0f, 1.0f) * float(LOOM_DECISION_MAX)));
}

kernel void organism_sample(
    const device packed_float3* position [[buffer(0)]],
    const device float* energy [[buffer(1)]],
    const device float* activator [[buffer(2)]],
    const device float* inhibitor [[buffer(3)]],
    const device float* nutrient [[buffer(4)]],
    const device float* density [[buffer(5)]],
    device uint* activator_bin [[buffer(6)]],
    device uint* inhibitor_bin [[buffer(7)]],
    device uint* nutrient_bin [[buffer(8)]],
    device uint* density_bin [[buffer(9)]],
    device uint* energy_bin [[buffer(10)]],
    constant uint& width [[buffer(11)]],
    uint index [[thread_position_in_grid]])
{
    const uint field = field_index(float3(position[index]).xy, width);
    activator_bin[index] = decision_bin(activator[field], 16.0f);
    inhibitor_bin[index] = decision_bin(inhibitor[field], 16.0f);
    nutrient_bin[index] = decision_bin(nutrient[field], 1.0f);
    density_bin[index] = decision_bin(density[field], 16.0f);
    energy_bin[index] = decision_bin(energy[index], 8.0f);
}

kernel void organism_decide(
    const device uint* stable_id [[buffer(0)]],
    const device uint* fate [[buffer(1)]],
    const device uint* phase [[buffer(2)]],
    const device uint* health [[buffer(3)]],
    const device uint* age [[buffer(4)]],
    const device uint* fate_confidence [[buffer(5)]],
    const device uint* activator_bin [[buffer(6)]],
    const device uint* inhibitor_bin [[buffer(7)]],
    const device uint* nutrient_bin [[buffer(8)]],
    const device uint* density_bin [[buffer(9)]],
    const device uint* energy_bin [[buffer(10)]],
    device uint* requested_fate [[buffer(11)]],
    device uint* requested_phase [[buffer(12)]],
    device uint* requested_health [[buffer(13)]],
    device uint* divide_intent [[buffer(14)]],
    device uint* death_intent [[buffer(15)]],
    device uint* activator_deposit [[buffer(16)]],
    device uint* inhibitor_deposit [[buffer(17)]],
    uint index [[thread_position_in_grid]])
{
    const uint current_fate = fate[index];
    const uint current_phase = phase[index];
    const uint current_health = health[index];
    uint next_phase = current_phase;
    if (current_phase == 0 && age[index] >= 60) next_phase = 1;
    else if (current_phase == 1 && fate_confidence[index] >= 60) next_phase = 2;
    else if (current_phase == 2 && fate_confidence[index] >= 120) next_phase = 3;

    uint next_fate = current_fate;
    if (current_fate == 1 && next_phase >= 2) {
        next_fate = density_bin[index] < 1024 ? 2 : 3;
    } else if (current_fate == 2 && density_bin[index] > 1792) {
        next_fate = 3;
    } else if (current_fate == 3 && density_bin[index] < 768) {
        next_fate = 2;
    }

    requested_fate[index] = stable_id[index] == 1 ? 0 : next_fate;
    requested_phase[index] = next_phase;
    requested_health[index] = current_health;
    const bool can_divide =
        next_phase >= 1 &&
        current_health == 0 &&
        age[index] >= 240 &&
        energy_bin[index] >= 1536 &&
        nutrient_bin[index] >= 2048 &&
        inhibitor_bin[index] < 3072;
    divide_intent[index] = can_divide ? 1 : 0;
    death_intent[index] = current_health == 2 || energy_bin[index] == 0 ? 1 : 0;
    activator_deposit[index] = stable_id[index] == 1 ? LOOM_Q16 : LOOM_Q16 / 16;
    inhibitor_deposit[index] = LOOM_Q16 / 128;
}

inline bool fate_allowed(uint from, uint to) {
    if (from == to) return true;
    if (from == 1 && (to == 2 || to == 3)) return true;
    return (from == 2 && to == 3) || (from == 3 && to == 2);
}

inline bool phase_allowed(uint from, uint to, uint health, bool repair) {
    if (from == to) return true;
    if (from + 1 == to && from < 3) return true;
    return from == 3 && to == 1 && health == 1 && repair;
}

inline bool health_allowed(uint from, uint to) {
    if (from == to) return true;
    return (from == 0 && to == 1) ||
           (from == 1 && (to == 0 || to == 2));
}

kernel void organism_resolve_state(
    device uint* fate [[buffer(0)]],
    device uint* phase [[buffer(1)]],
    device uint* health [[buffer(2)]],
    device uint* previous_fate [[buffer(3)]],
    device uint* fate_confidence [[buffer(4)]],
    device uint* time_in_fate [[buffer(5)]],
    device uint* age [[buffer(6)]],
    device float* energy [[buffer(7)]],
    device float4* color [[buffer(8)]],
    const device uint* requested_fate [[buffer(9)]],
    const device uint* requested_phase [[buffer(10)]],
    const device uint* requested_health [[buffer(11)]],
    const device uint* nutrient_bin [[buffer(12)]],
    const device uint* activator_deposit [[buffer(13)]],
    const device uint* inhibitor_deposit [[buffer(14)]],
    uint index [[thread_position_in_grid]])
{
    const uint old_fate = fate[index];
    const uint next_fate = requested_fate[index];
    previous_fate[index] = old_fate;
    if (fate_allowed(old_fate, next_fate)) {
        fate[index] = next_fate;
    }
    if (phase_allowed(phase[index], requested_phase[index], health[index], false)) {
        phase[index] = requested_phase[index];
    }
    if (health_allowed(health[index], requested_health[index])) {
        health[index] = requested_health[index];
    }
    if (fate[index] == old_fate) {
        fate_confidence[index] = min(fate_confidence[index] + 1, 1000000u);
        time_in_fate[index] += 1;
    } else {
        fate_confidence[index] = 0;
        time_in_fate[index] = 0;
    }
    age[index] += 1;
    const float absorbed = float(nutrient_bin[index]) / float(LOOM_DECISION_MAX) * 0.003f;
    const float signaling =
        float(activator_deposit[index] + inhibitor_deposit[index]) /
        float(LOOM_Q16) * 0.0001f;
    energy[index] = max(0.0f, energy[index] + absorbed - 0.001f - signaling);
    const float4 colors[4] = {
        float4(1.0, 0.25, 0.25, 1.0),
        float4(0.8, 0.8, 0.9, 1.0),
        float4(0.2, 0.8, 1.0, 1.0),
        float4(0.35, 1.0, 0.45, 1.0)
    };
    color[index] = colors[min(fate[index], 3u)];
}

kernel void organism_clear_deposits(
    device uint* activator [[buffer(0)]],
    device uint* inhibitor [[buffer(1)]],
    device uint* density [[buffer(2)]],
    uint index [[thread_position_in_grid]])
{
    activator[index] = 0;
    inhibitor[index] = 0;
    density[index] = 0;
}

inline void saturating_add(device atomic_uint* target, uint amount) {
    uint expected = atomic_load_explicit(target, memory_order_relaxed);
    while (true) {
        const uint desired = expected > UINT_MAX - amount ? UINT_MAX : expected + amount;
        if (atomic_compare_exchange_weak_explicit(
                target, &expected, desired,
                memory_order_relaxed, memory_order_relaxed)) {
            return;
        }
    }
}

kernel void organism_deposit(
    const device packed_float3* position [[buffer(0)]],
    const device uint* activator_amount [[buffer(1)]],
    const device uint* inhibitor_amount [[buffer(2)]],
    device atomic_uint* activator [[buffer(3)]],
    device atomic_uint* inhibitor [[buffer(4)]],
    device atomic_uint* density [[buffer(5)]],
    constant uint& width [[buffer(6)]],
    uint index [[thread_position_in_grid]])
{
    const float2 normalized = clamp(float3(position[index]).xy * 0.45f + 0.5f, 0.0f, 0.999999f);
    const int2 center = int2(normalized * float(width));
    constexpr uint weights[9] = {1, 2, 1, 2, 4, 2, 1, 2, 1};
    uint weight_index = 0;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            const int2 point = clamp(center + int2(x, y), int2(0), int2(width - 1));
            const uint field = uint(point.y) * width + uint(point.x);
            const uint weight = weights[weight_index++];
            saturating_add(&activator[field], activator_amount[index] * weight / 16);
            saturating_add(&inhibitor[field], inhibitor_amount[index] * weight / 16);
            saturating_add(&density[field], LOOM_Q16 * weight / 16);
        }
    }
}

inline float diffuse_channel(
    const device float* current,
    const device uint* deposit,
    uint index,
    uint width,
    float alpha,
    float decay,
    float maximum)
{
    const uint x = index % width;
    const uint y = index / width;
    const uint lx = x == 0 ? 0 : x - 1;
    const uint rx = x + 1 == width ? x : x + 1;
    const uint dy = y == 0 ? 0 : y - 1;
    const uint uy = y + 1 == width ? y : y + 1;
    const float center = current[index];
    const float laplacian =
        current[y * width + lx] + current[y * width + rx] +
        current[dy * width + x] + current[uy * width + x] -
        4.0f * center;
    return clamp(
        center + alpha * laplacian - decay * center +
        float(deposit[index]) / float(LOOM_Q16),
        0.0f, maximum);
}

kernel void organism_diffuse(
    const device float* activator [[buffer(0)]],
    const device float* inhibitor [[buffer(1)]],
    const device float* nutrient [[buffer(2)]],
    const device float* density [[buffer(3)]],
    const device float* injury [[buffer(4)]],
    const device uint* activator_deposit [[buffer(5)]],
    const device uint* inhibitor_deposit [[buffer(6)]],
    const device uint* density_deposit [[buffer(7)]],
    device float* activator_next [[buffer(8)]],
    device float* inhibitor_next [[buffer(9)]],
    device float* nutrient_next [[buffer(10)]],
    device float* density_next [[buffer(11)]],
    device float* injury_next [[buffer(12)]],
    constant uint& width [[buffer(13)]],
    uint index [[thread_position_in_grid]])
{
    activator_next[index] = diffuse_channel(
        activator, activator_deposit, index, width, 0.10f, 0.002f, 16.0f);
    inhibitor_next[index] = diffuse_channel(
        inhibitor, inhibitor_deposit, index, width, 0.22f, 0.001f, 16.0f);
    density_next[index] = diffuse_channel(
        density, density_deposit, index, width, 0.08f, 0.08f, 16.0f);
    nutrient_next[index] = clamp(nutrient[index] + 0.001f * (1.0f - nutrient[index]), 0.0f, 1.0f);
    injury_next[index] = max(0.0f, injury[index] * 0.99f);
}

kernel void organism_commit_fields(
    device float* activator [[buffer(0)]],
    device float* inhibitor [[buffer(1)]],
    device float* nutrient [[buffer(2)]],
    device float* density [[buffer(3)]],
    device float* injury [[buffer(4)]],
    const device float* activator_next [[buffer(5)]],
    const device float* inhibitor_next [[buffer(6)]],
    const device float* nutrient_next [[buffer(7)]],
    const device float* density_next [[buffer(8)]],
    const device float* injury_next [[buffer(9)]],
    uint index [[thread_position_in_grid]])
{
    activator[index] = activator_next[index];
    inhibitor[index] = inhibitor_next[index];
    nutrient[index] = nutrient_next[index];
    density[index] = density_next[index];
    injury[index] = injury_next[index];
}

inline void copy_cell(
    uint destination, uint source,
    device uint* stable_id,
    device uint* parent_id,
    device packed_float3* position,
    device float* radius,
    device float* energy,
    device uint* age,
    device uint* fate,
    device uint* phase,
    device uint* health,
    device uint* previous_fate,
    device uint* fate_confidence,
    device uint* time_in_fate,
    device float4* color)
{
    stable_id[destination] = stable_id[source];
    parent_id[destination] = parent_id[source];
    position[destination] = position[source];
    radius[destination] = radius[source];
    energy[destination] = energy[source];
    age[destination] = age[source];
    fate[destination] = fate[source];
    phase[destination] = phase[source];
    health[destination] = health[source];
    previous_fate[destination] = previous_fate[source];
    fate_confidence[destination] = fate_confidence[source];
    time_in_fate[destination] = time_in_fate[source];
    color[destination] = color[source];
}

kernel void organism_resolve_population(
    device uint* active_count [[buffer(0)]],
    device uint* next_stable_id [[buffer(1)]],
    device uint* stable_id [[buffer(2)]],
    device uint* parent_id [[buffer(3)]],
    device packed_float3* position [[buffer(4)]],
    device float* radius [[buffer(5)]],
    device float* energy [[buffer(6)]],
    device uint* age [[buffer(7)]],
    device uint* fate [[buffer(8)]],
    device uint* phase [[buffer(9)]],
    device uint* health [[buffer(10)]],
    device uint* previous_fate [[buffer(11)]],
    device uint* fate_confidence [[buffer(12)]],
    device uint* time_in_fate [[buffer(13)]],
    device float4* color [[buffer(14)]],
    const device uint* divide_intent [[buffer(15)]],
    const device uint* death_intent [[buffer(16)]],
    constant uint& capacity [[buffer(17)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    const uint old_count = active_count[0];
    uint write = 0;
    for (uint read = 0; read < old_count; ++read) {
        if (death_intent[read] != 0) continue;
        if (write != read) {
            copy_cell(write, read, stable_id, parent_id, position, radius, energy,
                      age, fate, phase, health, previous_fate, fate_confidence,
                      time_in_fate, color);
        }
        ++write;
    }
    const uint survivor_count = write;
    for (uint parent = 0; parent < survivor_count && write < capacity; ++parent) {
        if (divide_intent[parent] == 0) continue;
        const float angle = float(stable_id[parent] % 32u) * 0.1963495408f;
        const float2 offset = float2(cos(angle), sin(angle)) * radius[parent] * 2.0f;
        const float3 candidate = float3(position[parent]) + float3(offset, 0.0f);
        if (any(abs(candidate.xy) > 1.0f - radius[parent])) continue;
        bool overlap = false;
        for (uint other = 0; other < survivor_count; ++other) {
            if (other == parent) continue;
            const float minimum = radius[parent] + radius[other];
            if (distance(candidate.xy, float3(position[other]).xy) < minimum) {
                overlap = true;
                break;
            }
        }
        if (overlap) continue;
        copy_cell(write, parent, stable_id, parent_id, position, radius, energy,
                  age, fate, phase, health, previous_fate, fate_confidence,
                  time_in_fate, color);
        parent_id[write] = stable_id[parent];
        stable_id[write] = next_stable_id[0]++;
        position[write] = packed_float3(candidate);
        energy[parent] = max(0.0f, energy[parent] - 1.0f);
        energy[write] = 1.0f;
        age[parent] = 0;
        age[write] = 0;
        fate[write] = 1;
        phase[write] = 0;
        previous_fate[write] = 1;
        fate_confidence[write] = 0;
        time_in_fate[write] = 0;
        ++write;
    }
    active_count[0] = write;
}
