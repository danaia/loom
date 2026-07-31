#include <metal_stdlib>
using namespace metal;

constant uint PQO_Q16 = 65536;
constant uint PQO_DECISION_MAX = 4095;

inline uint field_index(float2 position, uint width) {
    const float2 normalized = clamp(position * 0.45f + 0.5f, 0.0f, 0.999999f);
    const uint2 cell = uint2(normalized * float(width));
    return cell.y * width + cell.x;
}

inline float sample_field_bilinear(
    const device float* field,
    float2 position,
    uint width)
{
    const float2 normalized = clamp(position * 0.45f + 0.5f, 0.0f, 1.0f);
    const float2 coordinate = normalized * float(width - 1u);
    const uint2 lower = uint2(floor(coordinate));
    const uint2 upper = min(lower + 1u, uint2(width - 1u));
    const float2 fraction = coordinate - float2(lower);
    const float lower_value = mix(
        field[lower.y * width + lower.x],
        field[lower.y * width + upper.x],
        fraction.x);
    const float upper_value = mix(
        field[upper.y * width + lower.x],
        field[upper.y * width + upper.x],
        fraction.x);
    return mix(lower_value, upper_value, fraction.y);
}

inline uint decision_bin(float value, float maximum) {
    return uint(round(clamp(value / maximum, 0.0f, 1.0f) * float(PQO_DECISION_MAX)));
}

kernel void organism_sample(
    const device packed_float3* position [[buffer(0)]],
    const device float* energy [[buffer(1)]],
    const device float* activator [[buffer(2)]],
    const device float* inhibitor [[buffer(3)]],
    const device float* nutrient [[buffer(4)]],
    const device float* density [[buffer(5)]],
    const device float* injury [[buffer(6)]],
    device uint* activator_bin [[buffer(7)]],
    device uint* inhibitor_bin [[buffer(8)]],
    device uint* nutrient_bin [[buffer(9)]],
    device uint* density_bin [[buffer(10)]],
    device uint* injury_bin [[buffer(11)]],
    device uint* energy_bin [[buffer(12)]],
    constant uint& width [[buffer(13)]],
    const device uint* active_count [[buffer(14)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const uint field = field_index(float3(position[index]).xy, width);
    activator_bin[index] = decision_bin(activator[field], 16.0f);
    inhibitor_bin[index] = decision_bin(inhibitor[field], 16.0f);
    nutrient_bin[index] = decision_bin(nutrient[field], 1.0f);
    density_bin[index] = decision_bin(density[field], 16.0f);
    injury_bin[index] = decision_bin(injury[field], 16.0f);
    energy_bin[index] = decision_bin(energy[index], 8.0f);
}

kernel void organism_decide(
    const device uint* stable_id [[buffer(0)]],
    const device uint* fate [[buffer(1)]],
    const device uint* phase [[buffer(2)]],
    const device uint* health [[buffer(3)]],
    const device uint* age [[buffer(4)]],
    const device uint* fate_confidence [[buffer(5)]],
    const device uint* time_in_fate [[buffer(6)]],
    const device uint* activator_bin [[buffer(7)]],
    const device uint* inhibitor_bin [[buffer(8)]],
    const device uint* nutrient_bin [[buffer(9)]],
    const device uint* density_bin [[buffer(10)]],
    const device uint* local_density_bin [[buffer(11)]],
    const device uint* contact_count [[buffer(12)]],
    const device uint* surface_mask [[buffer(13)]],
    const device uint* surface_exposure_bin [[buffer(14)]],
    const device uint* recent_surface_exposure [[buffer(15)]],
    const device uint* energy_bin [[buffer(16)]],
    const device uint* injury_bin [[buffer(17)]],
    const device uint* repair_enabled [[buffer(18)]],
    device uint* requested_fate [[buffer(19)]],
    device uint* requested_phase [[buffer(20)]],
    device uint* requested_health [[buffer(21)]],
    device uint* divide_intent [[buffer(22)]],
    device uint* death_intent [[buffer(23)]],
    device uint* repair_intent [[buffer(24)]],
    device uint* activator_deposit [[buffer(25)]],
    device uint* inhibitor_deposit [[buffer(26)]],
    device uint* injury_deposit [[buffer(27)]],
    const device uint* active_count [[buffer(28)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const uint current_fate = fate[index];
    const uint current_phase = phase[index];
    const uint current_health = health[index];
    const bool organizer = stable_id[index] == 1u;
    const bool receives_activator = organizer || activator_bin[index] >= 1u;
    const bool repair =
        repair_enabled[0] != 0u &&
        current_health == 1u &&
        injury_bin[index] >= 8u &&
        energy_bin[index] >= 384u;
    const uint exposure =
        (recent_surface_exposure[index] * 3u + surface_exposure_bin[index]) / 4u;
    const bool repair_complete =
        current_health == 1u &&
        current_phase == 2u &&
        (injury_bin[index] < 256u ||
         (contact_count[index] >= 3u && exposure < 2560u));
    uint next_phase = current_phase;
    if (repair_complete) next_phase = 3u;
    else if (current_health == 1u && current_phase == 2u) next_phase = 2u;
    else if (repair && current_phase == 3u) next_phase = 1u;
    else if (current_phase == 0 && age[index] >= 60 && receives_activator) next_phase = 1;
    else if (current_phase == 1 && fate_confidence[index] >= 60) next_phase = 2;
    else if (current_phase == 2 && fate_confidence[index] >= 120) next_phase = 3;

    uint next_fate = current_fate;
    if (current_fate == 1 && next_phase >= 2) {
        next_fate =
            surface_exposure_bin[index] >= 2560u || contact_count[index] < 3u ? 2u : 3u;
    } else if (current_fate == 2 && time_in_fate[index] >= 120u &&
               exposure < 2560u && contact_count[index] >= 3u) {
        next_fate = 3;
    } else if (current_fate == 3 && time_in_fate[index] >= 120u &&
               exposure > 3072u) {
        next_fate = 2;
    }

    requested_fate[index] = organizer ? 0 : next_fate;
    requested_phase[index] = next_phase;
    requested_health[index] = repair_complete ? 0u : current_health;
    const bool can_divide =
        next_phase >= 1 &&
        current_health == 0 &&
        injury_bin[index] < 8u &&
        age[index] >= 240 &&
        energy_bin[index] >= 1536 &&
        nutrient_bin[index] >= 2048 &&
        inhibitor_bin[index] < 256 &&
        local_density_bin[index] < 1024 &&
        ((~surface_mask[index]) & 255u) != 0u &&
        receives_activator;
    const bool regenerative_division =
        repair_enabled[0] != 0u &&
        injury_bin[index] >= 8u &&
        current_health == 1u &&
        current_phase == 1u &&
        age[index] >= 60u &&
        energy_bin[index] >= 384u &&
        nutrient_bin[index] >= 1024u &&
        inhibitor_bin[index] < 2048u &&
        local_density_bin[index] < 3072u &&
        ((~surface_mask[index]) & 255u) != 0u &&
        receives_activator;
    divide_intent[index] = can_divide || regenerative_division ? 1 : 0;
    death_intent[index] = current_health == 2 || energy_bin[index] == 0 ? 1 : 0;
    repair_intent[index] = repair || regenerative_division ? 1u : 0u;
    activator_deposit[index] = organizer ? PQO_Q16 : PQO_Q16 / 16;
    inhibitor_deposit[index] = organizer ? PQO_Q16 / 256 : PQO_Q16 / 16;
    injury_deposit[index] =
        current_health == 1u && current_phase != 2u ? PQO_Q16 / 32u : 0u;
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

inline uint mix_event(uint hash, uint tag, uint left, uint right) {
    hash = (hash ^ tag) * 16777619u;
    hash = (hash ^ left) * 16777619u;
    return (hash ^ right) * 16777619u;
}

kernel void organism_resolve_state(
    device uint* fate [[buffer(0)]],
    device uint* phase [[buffer(1)]],
    device uint* health [[buffer(2)]],
    device uint* previous_fate [[buffer(3)]],
    device uint* fate_confidence [[buffer(4)]],
    device uint* time_in_fate [[buffer(5)]],
    device uint* recent_activator [[buffer(6)]],
    device uint* recent_inhibitor [[buffer(7)]],
    device uint* recent_surface_exposure [[buffer(8)]],
    device uint* age [[buffer(9)]],
    device float* energy [[buffer(10)]],
    device float4* color [[buffer(11)]],
    const device uint* requested_fate [[buffer(12)]],
    const device uint* requested_phase [[buffer(13)]],
    const device uint* requested_health [[buffer(14)]],
    const device uint* nutrient_bin [[buffer(15)]],
    const device uint* activator_bin [[buffer(16)]],
    const device uint* inhibitor_bin [[buffer(17)]],
    const device uint* surface_exposure_bin [[buffer(18)]],
    const device uint* activator_deposit [[buffer(19)]],
    const device uint* inhibitor_deposit [[buffer(20)]],
    const device uint* injury_deposit [[buffer(21)]],
    const device uint* active_count [[buffer(22)]],
    const device uint* stable_id [[buffer(23)]],
    device uint* event_hash [[buffer(24)]],
    const device uint* divide_intent [[buffer(25)]],
    const device uint* death_intent [[buffer(26)]],
    device float* ledger_absorbed [[buffer(27)]],
    device float* ledger_maintenance [[buffer(28)]],
    device float* ledger_decisions [[buffer(29)]],
    device float* ledger_signaling [[buffer(30)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const uint old_fate = fate[index];
    const uint old_phase = phase[index];
    const uint old_health = health[index];
    const uint next_fate = requested_fate[index];
    previous_fate[index] = old_fate;
    if (fate_allowed(old_fate, next_fate)) {
        fate[index] = next_fate;
    }
    const bool repair_rollback =
        phase[index] == 3u &&
        requested_phase[index] == 1u &&
        health[index] == 1u;
    if (phase_allowed(
            phase[index], requested_phase[index], health[index],
            repair_rollback)) {
        phase[index] = requested_phase[index];
    }
    if (health_allowed(health[index], requested_health[index])) {
        health[index] = requested_health[index];
    }
    uint hash = event_hash[index];
    if (fate[index] != old_fate) {
        hash = mix_event(hash, 1u, old_fate, fate[index]);
    }
    if (phase[index] != old_phase) {
        hash = mix_event(hash, 2u, old_phase, phase[index]);
    }
    if (health[index] != old_health) {
        hash = mix_event(hash, 3u, old_health, health[index]);
    }
    event_hash[index] = mix_event(hash, 4u, stable_id[index], age[index]);
    if (fate[index] == old_fate) {
        fate_confidence[index] = min(fate_confidence[index] + 1, 1000000u);
        time_in_fate[index] += 1;
    } else {
        fate_confidence[index] = 0;
        time_in_fate[index] = 0;
    }
    recent_activator[index] =
        (recent_activator[index] * 7u + activator_bin[index]) / 8u;
    recent_inhibitor[index] =
        (recent_inhibitor[index] * 7u + inhibitor_bin[index]) / 8u;
    recent_surface_exposure[index] =
        (recent_surface_exposure[index] * 7u + surface_exposure_bin[index]) / 8u;
    age[index] += 1;
    const float previous_energy = energy[index];
    const float absorbed =
        float(nutrient_bin[index]) / float(PQO_DECISION_MAX) * 0.00978f;
    const float maintenance = 0.001f + previous_energy * 0.0022f;
    const float signaling =
        float(
            activator_deposit[index] +
            inhibitor_deposit[index] +
            injury_deposit[index]) /
        float(PQO_Q16) * 0.0001f;
    const float decisions =
        float(
            divide_intent[index] +
            death_intent[index] +
            uint(repair_rollback)) * 0.00001f;
    ledger_absorbed[index] = absorbed;
    ledger_maintenance[index] = maintenance;
    ledger_decisions[index] = decisions;
    ledger_signaling[index] = signaling;
    energy[index] = max(
        0.0f,
        previous_energy + absorbed - maintenance - decisions - signaling);
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
    device uint* injury [[buffer(3)]],
    uint index [[thread_position_in_grid]])
{
    activator[index] = 0;
    inhibitor[index] = 0;
    density[index] = 0;
    injury[index] = 0;
}

inline bool saturating_add(device atomic_uint* target, uint amount) {
    uint expected = atomic_load_explicit(target, memory_order_relaxed);
    while (true) {
        const bool saturated = expected > UINT_MAX - amount;
        const uint desired = saturated ? UINT_MAX : expected + amount;
        if (atomic_compare_exchange_weak_explicit(
                target, &expected, desired,
                memory_order_relaxed, memory_order_relaxed)) {
            return saturated;
        }
    }
}

kernel void organism_deposit(
    const device packed_float3* position [[buffer(0)]],
    const device uint* activator_amount [[buffer(1)]],
    const device uint* inhibitor_amount [[buffer(2)]],
    const device uint* injury_amount [[buffer(3)]],
    device atomic_uint* activator [[buffer(4)]],
    device atomic_uint* inhibitor [[buffer(5)]],
    device atomic_uint* density [[buffer(6)]],
    device atomic_uint* injury [[buffer(7)]],
    device atomic_uint* saturation_count [[buffer(8)]],
    constant uint& width [[buffer(9)]],
    const device uint* active_count [[buffer(10)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const float2 normalized = clamp(float3(position[index]).xy * 0.45f + 0.5f, 0.0f, 0.999999f);
    const int2 center = int2(normalized * float(width));
    constexpr uint weights[9] = {1, 2, 1, 2, 4, 2, 1, 2, 1};
    uint weight_index = 0;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            const int2 point = clamp(center + int2(x, y), int2(0), int2(width - 1));
            const uint field = uint(point.y) * width + uint(point.x);
            const uint weight = weights[weight_index++];
            const bool saturated =
                saturating_add(&activator[field], activator_amount[index] * weight / 16) |
                saturating_add(&inhibitor[field], inhibitor_amount[index] * weight / 16) |
                saturating_add(&density[field], PQO_Q16 * weight / 16) |
                saturating_add(&injury[field], injury_amount[index] * weight / 16);
            if (saturated) {
                atomic_fetch_add_explicit(
                    &saturation_count[0], 1u, memory_order_relaxed);
            }
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
        float(deposit[index]) / float(PQO_Q16),
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
    const device uint* injury_deposit [[buffer(8)]],
    device float* activator_next [[buffer(9)]],
    device float* inhibitor_next [[buffer(10)]],
    device float* nutrient_next [[buffer(11)]],
    device float* density_next [[buffer(12)]],
    device float* injury_next [[buffer(13)]],
    constant uint& width [[buffer(14)]],
    constant float& activator_transport [[buffer(15)]],
    constant float& inhibitor_transport [[buffer(16)]],
    const device float* nutrient_supply [[buffer(17)]],
    const device float* injury_transport [[buffer(18)]],
    uint index [[thread_position_in_grid]])
{
    activator_next[index] = diffuse_channel(
        activator, activator_deposit, index, width, 0.10f, 0.002f, 16.0f) *
        activator_transport;
    inhibitor_next[index] = diffuse_channel(
        inhibitor, inhibitor_deposit, index, width, 0.22f, 0.001f, 16.0f) *
        inhibitor_transport;
    density_next[index] = diffuse_channel(
        density, density_deposit, index, width, 0.08f, 0.08f, 16.0f);
    const float local_consumption =
        float(density_deposit[index]) / float(PQO_Q16) * 0.00005f;
    nutrient_next[index] = clamp(
        nutrient[index] +
        0.001f * (clamp(nutrient_supply[0], 0.0f, 1.0f) - nutrient[index]) -
        local_consumption,
        0.0f, 1.0f);
    injury_next[index] = diffuse_channel(
        injury, injury_deposit, index, width, 0.18f, 0.01f, 16.0f) *
        injury_transport[0];
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

kernel void organism_begin_energy_ledger(
    const device uint* active_count [[buffer(0)]],
    const device float* energy [[buffer(1)]],
    device float* previous_total [[buffer(2)]],
    device float* absorbed [[buffer(3)]],
    device float* maintenance [[buffer(4)]],
    device float* decisions [[buffer(5)]],
    device float* motion [[buffer(6)]],
    device float* signaling [[buffer(7)]],
    device float* division [[buffer(8)]],
    device float* environmental_death_loss [[buffer(9)]],
    device float* current_total [[buffer(10)]],
    device float* residual [[buffer(11)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    float total = 0.0f;
    for (uint cell = 0; cell < active_count[0]; ++cell) {
        total += energy[cell];
    }
    previous_total[0] = total;
    absorbed[0] = 0.0f;
    maintenance[0] = 0.0f;
    decisions[0] = 0.0f;
    motion[0] = 0.0f;
    signaling[0] = 0.0f;
    division[0] = 0.0f;
    environmental_death_loss[0] = 0.0f;
    current_total[0] = 0.0f;
    residual[0] = 0.0f;
}

kernel void organism_reduce_energy_ledger(
    const device uint* active_count [[buffer(0)]],
    const device float* cell_absorbed [[buffer(1)]],
    const device float* cell_maintenance [[buffer(2)]],
    const device float* cell_decisions [[buffer(3)]],
    const device float* cell_signaling [[buffer(4)]],
    const device uint* death_intent [[buffer(5)]],
    const device float* energy [[buffer(6)]],
    device float* absorbed [[buffer(7)]],
    device float* maintenance [[buffer(8)]],
    device float* decisions [[buffer(9)]],
    device float* signaling [[buffer(10)]],
    device float* environmental_death_loss [[buffer(11)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    float absorbed_total = 0.0f;
    float maintenance_total = 0.0f;
    float decision_total = 0.0f;
    float signaling_total = 0.0f;
    float death_total = 0.0f;
    for (uint cell = 0; cell < active_count[0]; ++cell) {
        absorbed_total += cell_absorbed[cell];
        maintenance_total += cell_maintenance[cell];
        decision_total += cell_decisions[cell];
        signaling_total += cell_signaling[cell];
        if (death_intent[cell] != 0u) {
            death_total += energy[cell];
        }
    }
    absorbed[0] = absorbed_total;
    maintenance[0] = maintenance_total;
    decisions[0] = decision_total;
    signaling[0] = signaling_total;
    environmental_death_loss[0] = death_total;
}

kernel void organism_finalize_energy_ledger(
    const device uint* active_count [[buffer(0)]],
    const device float* energy [[buffer(1)]],
    const device uint* accepted_birth_count [[buffer(2)]],
    const device float* previous_total [[buffer(3)]],
    const device float* absorbed [[buffer(4)]],
    const device float* maintenance [[buffer(5)]],
    const device float* decisions [[buffer(6)]],
    const device float* motion [[buffer(7)]],
    const device float* signaling [[buffer(8)]],
    device float* division [[buffer(9)]],
    const device float* environmental_death_loss [[buffer(10)]],
    device float* current_total [[buffer(11)]],
    device float* residual [[buffer(12)]],
    device float* cumulative_residual [[buffer(13)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    float total = 0.0f;
    for (uint cell = 0; cell < active_count[0]; ++cell) {
        total += energy[cell];
    }
    const float division_cost = float(accepted_birth_count[0]) * 0.1f;
    const float tick_residual =
        previous_total[0] + absorbed[0] -
        maintenance[0] - decisions[0] - motion[0] - signaling[0] -
        division_cost - environmental_death_loss[0] - total;
    division[0] = division_cost;
    current_total[0] = total;
    residual[0] = tick_residual;
    cumulative_residual[0] += tick_residual;
}

inline bool homeostasis_metric_within(
    uint value,
    uint minimum,
    uint maximum,
    uint metric)
{
    uint margin = 1u;
    if (metric <= 2u) {
        margin = max(1u, maximum / 10u);
    } else if (metric <= 5u || metric >= 8u) {
        margin = max(1u, maximum * 15u / 100u);
    } else {
        margin = 6554u;
    }
    const uint lower = minimum > margin ? minimum - margin : 0u;
    const uint upper = maximum > UINT_MAX - margin ? UINT_MAX : maximum + margin;
    return value >= lower && value <= upper;
}

kernel void organism_measure_homeostasis_events(
    const device uint* tick [[buffer(0)]],
    const device uint* component_count [[buffer(1)]],
    const device uint* component_unresolved [[buffer(2)]],
    const device uint* organizer_count [[buffer(3)]],
    const device uint* boundary_count [[buffer(4)]],
    const device uint* interior_count [[buffer(5)]],
    const device uint* neighbor_overflow [[buffer(6)]],
    const device uint* physical_overflow [[buffer(7)]],
    const device uint* perception_truncation [[buffer(8)]],
    const device uint* deposit_saturation [[buffer(9)]],
    const device float* current_energy [[buffer(10)]],
    const device uint* perturbation_window [[buffer(11)]],
    device uint* invariant_violations [[buffer(12)]],
    device float* perturbation_energy_min [[buffer(13)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    const uint now = tick[0];
    if (now >= 3200u) {
        const bool invalid =
            component_count[0] != 1u ||
            component_unresolved[0] != 0u ||
            organizer_count[0] != 1u ||
            boundary_count[0] == 0u ||
            interior_count[0] == 0u ||
            neighbor_overflow[0] != 0u ||
            physical_overflow[0] != 0u ||
            perception_truncation[0] != 0u ||
            deposit_saturation[0] != 0u;
        invariant_violations[0] += uint(invalid);
    }
    if (now >= perturbation_window[0] && now < perturbation_window[1]) {
        perturbation_energy_min[0] = now == perturbation_window[0]
            ? current_energy[0]
            : min(perturbation_energy_min[0], current_energy[0]);
    }
}

kernel void organism_audit_homeostasis(
    const device uint* tick [[buffer(0)]],
    const device uint* population [[buffer(1)]],
    const device uint* component_count [[buffer(2)]],
    const device uint* component_unresolved [[buffer(3)]],
    const device uint* organizer_count [[buffer(4)]],
    const device uint* boundary_count [[buffer(5)]],
    const device uint* interior_count [[buffer(6)]],
    const device uint* area_q16 [[buffer(7)]],
    const device uint* perimeter_q16 [[buffer(8)]],
    const device uint* compactness_q16 [[buffer(9)]],
    const device int* centroid_x_q16 [[buffer(10)]],
    const device int* centroid_y_q16 [[buffer(11)]],
    const device uint* radial_density [[buffer(12)]],
    const device float* current_energy [[buffer(13)]],
    const device uint* neighbor_overflow [[buffer(14)]],
    const device uint* physical_overflow [[buffer(15)]],
    const device uint* perception_truncation [[buffer(16)]],
    const device uint* deposit_saturation [[buffer(17)]],
    device uint* metric_min [[buffer(18)]],
    device uint* metric_max [[buffer(19)]],
    device float* metric_sum [[buffer(20)]],
    device float* metric_sum_sq [[buffer(21)]],
    device float* energy_min [[buffer(22)]],
    device float* energy_max [[buffer(23)]],
    device float* energy_sum [[buffer(24)]],
    device float* energy_sum_sq [[buffer(25)]],
    device uint* reference_samples [[buffer(26)]],
    device uint* validation_samples [[buffer(27)]],
    device uint* validation_violations [[buffer(28)]],
    const device uint* window [[buffer(29)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    const uint now = tick[0];
    const uint reference_start = window[0];
    const uint reference_end = window[1];
    const uint validation_start = window[2];
    const uint validation_end = window[3];
    uint metrics[16] = {
        population[0],
        boundary_count[0],
        interior_count[0],
        area_q16[0],
        perimeter_q16[0],
        compactness_q16[0],
        uint(clamp(centroid_x_q16[0] + 65536, 0, 131072)),
        uint(clamp(centroid_y_q16[0] + 65536, 0, 131072)),
        radial_density[0],
        radial_density[1],
        radial_density[2],
        radial_density[3],
        radial_density[4],
        radial_density[5],
        radial_density[6],
        radial_density[7]
    };

    if (now >= reference_start && now < reference_end) {
        if (reference_samples[0] == 0u) {
            for (uint metric = 0; metric < 16u; ++metric) {
                metric_min[metric] = UINT_MAX;
                metric_max[metric] = 0u;
                metric_sum[metric] = 0.0f;
                metric_sum_sq[metric] = 0.0f;
            }
            energy_min[0] = INFINITY;
            energy_max[0] = 0.0f;
            energy_sum[0] = 0.0f;
            energy_sum_sq[0] = 0.0f;
        }
        for (uint metric = 0; metric < 16u; ++metric) {
            metric_min[metric] = min(metric_min[metric], metrics[metric]);
            metric_max[metric] = max(metric_max[metric], metrics[metric]);
            const float value = float(metrics[metric]);
            metric_sum[metric] += value;
            metric_sum_sq[metric] += value * value;
        }
        energy_min[0] = min(energy_min[0], current_energy[0]);
        energy_max[0] = max(energy_max[0], current_energy[0]);
        energy_sum[0] += current_energy[0];
        energy_sum_sq[0] += current_energy[0] * current_energy[0];
        reference_samples[0] += 1u;
    }

    if (now >= validation_start && now < validation_end) {
        bool invalid =
            reference_samples[0] == 0u ||
            component_count[0] != 1u ||
            component_unresolved[0] != 0u ||
            organizer_count[0] != 1u ||
            boundary_count[0] == 0u ||
            interior_count[0] == 0u ||
            neighbor_overflow[0] != 0u ||
            physical_overflow[0] != 0u ||
            perception_truncation[0] != 0u ||
            deposit_saturation[0] != 0u;
        for (uint metric = 0; metric < 16u; ++metric) {
            invalid = invalid || !homeostasis_metric_within(
                metrics[metric], metric_min[metric], metric_max[metric], metric);
        }
        const float energy_margin =
            max(0.001f, max(abs(energy_min[0]), abs(energy_max[0])) * 0.10f);
        invalid = invalid ||
            current_energy[0] < energy_min[0] - energy_margin ||
            current_energy[0] > energy_max[0] + energy_margin;
        validation_samples[0] += 1u;
        validation_violations[0] += uint(invalid);
    }
}

kernel void organism_advance_tick(
    device uint* tick [[buffer(0)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) tick[0] += 1u;
}

kernel void organism_set_nutrient_supply(
    device float* nutrient_supply [[buffer(0)]],
    constant float& requested_supply [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) {
        nutrient_supply[0] = clamp(requested_supply, 0.0f, 1.0f);
    }
}

kernel void organism_set_injury_transport(
    device float* injury_transport [[buffer(0)]],
    constant float& requested_transport [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) {
        injury_transport[0] = clamp(requested_transport, 0.0f, 1.0f);
    }
}

kernel void organism_set_repair_enabled(
    device uint* repair_enabled [[buffer(0)]],
    constant uint& requested_enabled [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) repair_enabled[0] = min(requested_enabled, 1u);
}

kernel void organism_apply_lesion_cells(
    const device packed_float3* position [[buffer(0)]],
    const device uint* stable_id [[buffer(1)]],
    const device uint* fate [[buffer(2)]],
    device uint* health [[buffer(3)]],
    device uint* event_hash [[buffer(4)]],
    const device float* energy [[buffer(5)]],
    const device uint* active_count [[buffer(6)]],
    constant int& center_x_q16 [[buffer(7)]],
    constant int& center_y_q16 [[buffer(8)]],
    constant uint& radius_q16 [[buffer(9)]],
    device uint* removed_ids [[buffer(10)]],
    device uint* removed_count [[buffer(11)]],
    device uint* damaged_count [[buffer(12)]],
    device float* removed_energy [[buffer(13)]],
    device int* recorded_center_q16 [[buffer(14)]],
    device uint* recorded_radius_q16 [[buffer(15)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    uint removed = 0u;
    uint damaged = 0u;
    float lost_energy = 0.0f;
    const long radius_squared = long(radius_q16) * long(radius_q16);
    const long shell_radius = long(radius_q16) + 1966l;
    const long shell_squared = shell_radius * shell_radius;
    for (uint cell = 0; cell < active_count[0]; ++cell) {
        const float2 point = float3(position[cell]).xy;
        const long x = long(round(point.x * float(PQO_Q16))) - long(center_x_q16);
        const long y = long(round(point.y * float(PQO_Q16))) - long(center_y_q16);
        const long distance_squared = x * x + y * y;
        if (fate[cell] != 0u && distance_squared <= radius_squared) {
            health[cell] = 2u;
            if (removed < 64u) removed_ids[removed] = stable_id[cell];
            removed += 1u;
            lost_energy += energy[cell];
            event_hash[cell] = mix_event(
                event_hash[cell], 7u, stable_id[cell], uint(distance_squared));
        } else if (
            fate[cell] != 0u &&
            health[cell] == 0u &&
            distance_squared <= shell_squared) {
            health[cell] = 1u;
            damaged += 1u;
            event_hash[cell] = mix_event(
                event_hash[cell], 8u, stable_id[cell], uint(distance_squared));
        }
    }
    removed_count[0] = removed;
    damaged_count[0] = damaged;
    removed_energy[0] = lost_energy;
    recorded_center_q16[0] = center_x_q16;
    recorded_center_q16[1] = center_y_q16;
    recorded_radius_q16[0] = radius_q16;
}

kernel void organism_apply_lesion_field(
    device float* injury [[buffer(0)]],
    constant uint& width [[buffer(1)]],
    constant int& center_x_q16 [[buffer(2)]],
    constant int& center_y_q16 [[buffer(3)]],
    constant uint& radius_q16 [[buffer(4)]],
    constant float& injury_strength [[buffer(5)]],
    const device float* injury_transport [[buffer(6)]],
    uint index [[thread_position_in_grid]])
{
    const uint x = index % width;
    const uint y = index / width;
    const float2 world =
        (float2(float(x) + 0.5f, float(y) + 0.5f) / float(width) - 0.5f) /
        0.45f;
    const long dx =
        long(round(world.x * float(PQO_Q16))) - long(center_x_q16);
    const long dy =
        long(round(world.y * float(PQO_Q16))) - long(center_y_q16);
    if (dx * dx + dy * dy <= long(radius_q16) * long(radius_q16)) {
        injury[index] = max(
            injury[index],
            max(0.0f, injury_strength) * injury_transport[0]);
    }
}

kernel void organism_clear_regeneration_metrics(
    device atomic_uint* region_occupancy [[buffer(0)]],
    device atomic_uint* injury_total_q16 [[buffer(1)]],
    device atomic_uint* injury_peak_q16 [[buffer(2)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) {
        atomic_store_explicit(&region_occupancy[0], 0u, memory_order_relaxed);
        atomic_store_explicit(&injury_total_q16[0], 0u, memory_order_relaxed);
        atomic_store_explicit(&injury_peak_q16[0], 0u, memory_order_relaxed);
    }
}

kernel void organism_reduce_lesion_occupancy(
    const device packed_float3* position [[buffer(0)]],
    const device float* cell_radius [[buffer(1)]],
    const device int* center_q16 [[buffer(2)]],
    const device uint* radius_q16 [[buffer(3)]],
    device atomic_uint* region_occupancy [[buffer(4)]],
    const device uint* active_count [[buffer(5)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0] || radius_q16[0] == 0u) return;
    const float2 point = float3(position[index]).xy;
    const long dx =
        long(round(point.x * float(PQO_Q16))) - long(center_q16[0]);
    const long dy =
        long(round(point.y * float(PQO_Q16))) - long(center_q16[1]);
    const long occupied_radius =
        long(radius_q16[0]) +
        long(round(clamp(cell_radius[index], 0.0f, 1.0f) * float(PQO_Q16)));
    if (dx * dx + dy * dy <= occupied_radius * occupied_radius) {
        atomic_fetch_add_explicit(&region_occupancy[0], 1u, memory_order_relaxed);
    }
}

kernel void organism_reduce_injury(
    const device float* injury [[buffer(0)]],
    device atomic_uint* injury_total_q16 [[buffer(1)]],
    device atomic_uint* injury_peak_q16 [[buffer(2)]],
    uint index [[thread_position_in_grid]])
{
    const uint value = uint(round(clamp(injury[index], 0.0f, 16.0f) * 4096.0f));
    atomic_fetch_add_explicit(&injury_total_q16[0], value, memory_order_relaxed);
    atomic_fetch_max_explicit(&injury_peak_q16[0], value, memory_order_relaxed);
}

inline bool regeneration_metric_within(
    uint value,
    uint minimum,
    uint maximum,
    uint percent)
{
    const uint margin = max(1u, maximum * percent / 100u);
    const uint lower = minimum > margin ? minimum - margin : 0u;
    return value >= lower && value <= maximum + margin;
}

inline uint integer_sqrt(ulong value) {
    ulong result = 0ul;
    ulong bit = 1ul << 62;
    while (bit > value) bit >>= 2;
    while (bit != 0ul) {
        if (value >= result + bit) {
            value -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    return uint(min(result, ulong(UINT_MAX)));
}

kernel void organism_audit_regeneration(
    const device uint* tick [[buffer(0)]],
    const device uint* population [[buffer(1)]],
    const device uint* component_count [[buffer(2)]],
    const device uint* component_unresolved [[buffer(3)]],
    const device uint* organizer_count [[buffer(4)]],
    const device uint* boundary_count [[buffer(5)]],
    const device uint* interior_count [[buffer(6)]],
    const device uint* area_q16 [[buffer(7)]],
    const device uint* compactness_q16 [[buffer(8)]],
    const device int* centroid_x_q16 [[buffer(9)]],
    const device int* centroid_y_q16 [[buffer(10)]],
    const device uint* injury_total_q16 [[buffer(11)]],
    device uint* post_lesion_peak_q16 [[buffer(12)]],
    const device uint* removed_count [[buffer(13)]],
    const device uint* region_occupancy [[buffer(14)]],
    const device uint* metric_min [[buffer(15)]],
    const device uint* metric_max [[buffer(16)]],
    const device uint* reference_samples [[buffer(17)]],
    const device uint* neighbor_overflow [[buffer(18)]],
    const device uint* physical_overflow [[buffer(19)]],
    const device uint* perception_truncation [[buffer(20)]],
    const device uint* deposit_saturation [[buffer(21)]],
    const device float* energy_residual [[buffer(22)]],
    const device uint* window [[buffer(23)]],
    device uint* consecutive_ticks [[buffer(24)]],
    device uint* success_tick [[buffer(25)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    const uint now = tick[0];
    if (now < window[0] || now >= window[1] || removed_count[0] == 0u) return;
    post_lesion_peak_q16[0] =
        max(post_lesion_peak_q16[0], injury_total_q16[0]);
    const uint encoded_centroid_x =
        uint(clamp(centroid_x_q16[0] + 65536, 0, 131072));
    const uint encoded_centroid_y =
        uint(clamp(centroid_y_q16[0] + 65536, 0, 131072));
    const uint required_occupancy = (removed_count[0] * 9u + 9u) / 10u;
    constexpr ulong PI_Q16 = 205887ul;
    const ulong reference_area_scaled =
        ulong(metric_max[3]) * ulong(PQO_Q16) * ulong(PQO_Q16) / PI_Q16;
    const uint centroid_tolerance =
        max(1u, integer_sqrt(reference_area_scaled) / 10u);
    const bool injury_cleared =
        post_lesion_peak_q16[0] > 0u &&
        injury_total_q16[0] * 20u <= post_lesion_peak_q16[0];
    const bool recovered =
        reference_samples[0] == 1000u &&
        component_count[0] == 1u &&
        component_unresolved[0] == 0u &&
        organizer_count[0] == 1u &&
        regeneration_metric_within(population[0], metric_min[0], metric_max[0], 10u) &&
        regeneration_metric_within(boundary_count[0], metric_min[1], metric_max[1], 10u) &&
        regeneration_metric_within(interior_count[0], metric_min[2], metric_max[2], 10u) &&
        regeneration_metric_within(area_q16[0], metric_min[3], metric_max[3], 10u) &&
        regeneration_metric_within(
            compactness_q16[0], metric_min[5], metric_max[5], 15u) &&
        encoded_centroid_x + centroid_tolerance >= metric_min[6] &&
        encoded_centroid_x <= metric_max[6] + centroid_tolerance &&
        encoded_centroid_y + centroid_tolerance >= metric_min[7] &&
        encoded_centroid_y <= metric_max[7] + centroid_tolerance &&
        region_occupancy[0] >= required_occupancy &&
        injury_cleared &&
        neighbor_overflow[0] == 0u &&
        physical_overflow[0] == 0u &&
        perception_truncation[0] == 0u &&
        deposit_saturation[0] == 0u &&
        abs(energy_residual[0]) <= 0.001f;
    consecutive_ticks[0] = recovered ? consecutive_ticks[0] + 1u : 0u;
    if (success_tick[0] == 0u && consecutive_ticks[0] >= window[2]) {
        success_tick[0] = now + 1u;
    }
}

constant uint PQO_RADIX_BUCKETS = 16;
constant uint PQO_SCAN_BLOCK = 256;

inline uint stable_id_digit(
    uint source,
    const device uint* stable_id,
    uint shift)
{
    return source == UINT_MAX ? 15u : ((stable_id[source] >> shift) & 15u);
}

kernel void organism_initialize_population_order(
    const device uint* active_count [[buffer(0)]],
    device uint* order [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    order[index] = index < active_count[0] ? index : UINT_MAX;
}

kernel void organism_radix_histogram(
    const device uint* order [[buffer(0)]],
    const device uint* stable_id [[buffer(1)]],
    device uint* block_count [[buffer(2)]],
    constant uint& shift [[buffer(3)]],
    uint global [[thread_position_in_grid]],
    uint local [[thread_index_in_threadgroup]],
    uint group [[threadgroup_position_in_grid]])
{
    threadgroup atomic_uint histogram[PQO_RADIX_BUCKETS];
    if (local < PQO_RADIX_BUCKETS) {
        atomic_store_explicit(&histogram[local], 0u, memory_order_relaxed);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const uint digit = stable_id_digit(order[global], stable_id, shift);
    atomic_fetch_add_explicit(&histogram[digit], 1u, memory_order_relaxed);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (local < PQO_RADIX_BUCKETS) {
        block_count[group * PQO_RADIX_BUCKETS + local] =
            atomic_load_explicit(&histogram[local], memory_order_relaxed);
    }
}

kernel void organism_radix_offsets(
    const device uint* block_count [[buffer(0)]],
    device uint* offset [[buffer(1)]],
    constant uint& block_count_value [[buffer(2)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    uint bucket_base = 0;
    for (uint bucket = 0; bucket < PQO_RADIX_BUCKETS; ++bucket) {
        uint running = bucket_base;
        for (uint block = 0; block < block_count_value; ++block) {
            const uint slot = block * PQO_RADIX_BUCKETS + bucket;
            offset[slot] = running;
            running += block_count[slot];
        }
        bucket_base = running;
    }
}

kernel void organism_radix_scatter(
    const device uint* input [[buffer(0)]],
    device uint* output [[buffer(1)]],
    const device uint* stable_id [[buffer(2)]],
    const device uint* offset [[buffer(3)]],
    constant uint& shift [[buffer(4)]],
    uint global [[thread_position_in_grid]],
    uint group [[threadgroup_position_in_grid]])
{
    const uint source = input[global];
    const uint digit = stable_id_digit(source, stable_id, shift);
    const uint block_start = group * PQO_SCAN_BLOCK;
    uint local_rank = 0;
    for (uint preceding = block_start; preceding < global; ++preceding) {
        local_rank += uint(stable_id_digit(input[preceding], stable_id, shift) == digit);
    }
    const uint destination =
        offset[group * PQO_RADIX_BUCKETS + digit] + local_rank;
    output[destination] = source;
}

constant uint PQO_SPATIAL_AXIS = 64;
constant uint PQO_SPATIAL_BINS = PQO_SPATIAL_AXIS * PQO_SPATIAL_AXIS;
constant uint PQO_BIN_CAPACITY = 128;

inline uint spatial_key(float2 position) {
    const float2 normalized = clamp(position * 0.5f + 0.5f, 0.0f, 0.999999f);
    const uint2 cell = uint2(normalized * float(PQO_SPATIAL_AXIS));
    return cell.y * PQO_SPATIAL_AXIS + cell.x;
}

inline int2 spatial_cell(float2 position) {
    const uint key = spatial_key(position);
    return int2(int(key % PQO_SPATIAL_AXIS), int(key / PQO_SPATIAL_AXIS));
}

inline float2 daughter_position(float2 parent, float radius, uint sector) {
    constexpr float diagonal = 0.7071067812f;
    constexpr float2 directions[8] = {
        float2(1.0f, 0.0f),
        float2(diagonal, diagonal),
        float2(0.0f, 1.0f),
        float2(-diagonal, diagonal),
        float2(-1.0f, 0.0f),
        float2(-diagonal, -diagonal),
        float2(0.0f, -1.0f),
        float2(diagonal, -diagonal)
    };
    return parent + directions[sector & 7u] * radius * 2.05f;
}

inline int2 q16_position(float2 value) {
    return int2(round(value * float(PQO_Q16)));
}

inline uint q16_radius(float value) {
    return uint(round(max(value, 0.0f) * float(PQO_Q16)));
}

inline bool quantized_overlap(float2 left_position, float left_radius,
                              float2 right_position, float right_radius) {
    const long2 delta = long2(q16_position(left_position)) - long2(q16_position(right_position));
    const ulong distance_squared = ulong(delta.x * delta.x + delta.y * delta.y);
    const ulong minimum = ulong(q16_radius(left_radius) + q16_radius(right_radius));
    return distance_squared < minimum * minimum;
}

inline bool quantized_contact(float2 left_position, float left_radius,
                              float2 right_position, float right_radius) {
    const long2 delta = long2(q16_position(left_position)) - long2(q16_position(right_position));
    const ulong distance_squared = ulong(delta.x * delta.x + delta.y * delta.y);
    const uint left = q16_radius(left_radius);
    const uint right = q16_radius(right_radius);
    const ulong maximum = ulong(left + right + min(left, right) / 4u);
    return distance_squared <= maximum * maximum;
}

kernel void organism_clear_population_bins(
    device uint* living_count [[buffer(0)]],
    device uint* candidate_count [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    living_count[index] = 0;
    candidate_count[index] = 0;
}

kernel void organism_bin_living(
    const device uint* active_count [[buffer(0)]],
    const device packed_float3* position [[buffer(1)]],
    device atomic_uint* count [[buffer(2)]],
    device uint* indices [[buffer(3)]],
    device atomic_uint* overflow [[buffer(4)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const uint key = spatial_key(float3(position[index]).xy);
    const uint slot = atomic_fetch_add_explicit(&count[key], 1u, memory_order_relaxed);
    if (slot < PQO_BIN_CAPACITY) {
        indices[key * PQO_BIN_CAPACITY + slot] = index;
    } else {
        atomic_fetch_add_explicit(&overflow[0], 1u, memory_order_relaxed);
    }
}

kernel void organism_sort_bins(
    const device uint* count [[buffer(0)]],
    device uint* indices [[buffer(1)]],
    const device uint* stable_id [[buffer(2)]],
    uint bin [[thread_position_in_grid]])
{
    const uint length = min(count[bin], PQO_BIN_CAPACITY);
    const uint base = bin * PQO_BIN_CAPACITY;
    for (uint i = 1; i < length; ++i) {
        const uint candidate = indices[base + i];
        const uint candidate_id = stable_id[candidate];
        uint insertion = i;
        while (insertion > 0) {
            const uint previous = indices[base + insertion - 1];
            if (stable_id[previous] <= candidate_id) break;
            indices[base + insertion] = previous;
            --insertion;
        }
        indices[base + insertion] = candidate;
    }
}

inline uint exact_sector(int2 delta) {
    const uint ax = uint(abs(delta.x));
    const uint ay = uint(abs(delta.y));
    if (ax >= ay * 2u) return delta.x >= 0 ? 0u : 4u;
    if (ay >= ax * 2u) return delta.y >= 0 ? 2u : 6u;
    if (delta.x >= 0 && delta.y >= 0) return 1u;
    if (delta.x < 0 && delta.y >= 0) return 3u;
    if (delta.x < 0 && delta.y < 0) return 5u;
    return 7u;
}

kernel void organism_observe_neighbors(
    const device packed_float3* position [[buffer(0)]],
    const device float* radius [[buffer(1)]],
    const device uint* stable_id [[buffer(2)]],
    const device uint* bin_count [[buffer(3)]],
    const device uint* bin_indices [[buffer(4)]],
    device uint* local_density_bin [[buffer(5)]],
    device uint* neighbor_count [[buffer(6)]],
    device uint* contact_count [[buffer(7)]],
    device uint* surface_mask [[buffer(8)]],
    device uint* surface_exposure_bin [[buffer(9)]],
    device atomic_uint* physical_overflow [[buffer(10)]],
    device atomic_uint* perception_truncation [[buffer(11)]],
    const device uint* active_count [[buffer(12)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const float2 center_position = float3(position[index]).xy;
    const int2 center_bin = spatial_cell(center_position);
    const int2 center_q16 = q16_position(center_position);
    constexpr ulong perception_radius_squared = 2400ul * 2400ul;
    uint physical_observations = 0;
    uint perceived = 0;
    uint contacts = 0;
    uint occupied_mask = 0;
    bool physical_exceeded = false;
    bool perception_exceeded = false;

    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            const int2 cell = center_bin + int2(x, y);
            if (any(cell < 0) || any(cell >= int(PQO_SPATIAL_AXIS))) continue;
            const uint key = uint(cell.y) * PQO_SPATIAL_AXIS + uint(cell.x);
            const uint length = min(bin_count[key], PQO_BIN_CAPACITY);
            const uint base = key * PQO_BIN_CAPACITY;
            for (uint item = 0; item < length; ++item) {
                const uint other = bin_indices[base + item];
                if (other == index || stable_id[other] == stable_id[index]) continue;
                if (physical_observations >= 128u) {
                    physical_exceeded = true;
                    continue;
                }
                ++physical_observations;
                const int2 delta_q16 =
                    q16_position(float3(position[other]).xy) - center_q16;
                const long2 delta = long2(delta_q16);
                const ulong distance_squared =
                    ulong(delta.x * delta.x + delta.y * delta.y);
                if (quantized_contact(
                        center_position, radius[index],
                        float3(position[other]).xy, radius[other])) {
                    ++contacts;
                }
                if (distance_squared <= perception_radius_squared) {
                    if (perceived < 64u) {
                        occupied_mask |= 1u << exact_sector(delta_q16);
                        ++perceived;
                    } else {
                        perception_exceeded = true;
                    }
                }
            }
        }
    }
    if (physical_exceeded) {
        atomic_fetch_add_explicit(&physical_overflow[0], 1u, memory_order_relaxed);
    }
    if (perception_exceeded) {
        atomic_fetch_add_explicit(&perception_truncation[0], 1u, memory_order_relaxed);
    }
    const uint exposed_sectors = popcount((~occupied_mask) & 255u);
    neighbor_count[index] = perceived;
    contact_count[index] = contacts;
    surface_mask[index] = occupied_mask;
    surface_exposure_bin[index] = exposed_sectors * PQO_DECISION_MAX / 8u;
    local_density_bin[index] = perceived * PQO_DECISION_MAX / 64u;
}

kernel void organism_initialize_components(
    const device uint* stable_id [[buffer(0)]],
    device uint* label [[buffer(1)]],
    const device uint* active_count [[buffer(2)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    label[index] = stable_id[index];
}

kernel void organism_clear_component_changes(
    device uint* changes [[buffer(0)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) changes[0] = 0;
}

kernel void organism_relax_components(
    const device packed_float3* position [[buffer(0)]],
    const device float* radius [[buffer(1)]],
    const device uint* bin_count [[buffer(2)]],
    const device uint* bin_indices [[buffer(3)]],
    const device uint* input_label [[buffer(4)]],
    device uint* output_label [[buffer(5)]],
    device atomic_uint* changes [[buffer(6)]],
    const device uint* active_count [[buffer(7)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const float2 center_position = float3(position[index]).xy;
    const int2 center_bin = spatial_cell(center_position);
    uint minimum_label = input_label[index];
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            const int2 cell = center_bin + int2(x, y);
            if (any(cell < 0) || any(cell >= int(PQO_SPATIAL_AXIS))) continue;
            const uint key = uint(cell.y) * PQO_SPATIAL_AXIS + uint(cell.x);
            const uint length = min(bin_count[key], PQO_BIN_CAPACITY);
            const uint base = key * PQO_BIN_CAPACITY;
            for (uint item = 0; item < length; ++item) {
                const uint other = bin_indices[base + item];
                if (other == index) continue;
                if (quantized_contact(
                        center_position, radius[index],
                        float3(position[other]).xy, radius[other])) {
                    minimum_label = min(minimum_label, input_label[other]);
                }
            }
        }
    }
    output_label[index] = minimum_label;
    if (minimum_label != input_label[index]) {
        atomic_fetch_add_explicit(&changes[0], 1u, memory_order_relaxed);
    }
}

kernel void organism_clear_morphology(
    device uint* radial_density [[buffer(0)]],
    device uint* component_count [[buffer(1)]],
    device uint* organizer_count [[buffer(2)]],
    device uint* undifferentiated_count [[buffer(3)]],
    device uint* boundary_count [[buffer(4)]],
    device uint* interior_count [[buffer(5)]],
    device uint* area_q16 [[buffer(6)]],
    device uint* perimeter_q16 [[buffer(7)]],
    device int* centroid_sum_x_q16 [[buffer(8)]],
    device int* centroid_sum_y_q16 [[buffer(9)]],
    uint index [[thread_position_in_grid]])
{
    radial_density[index] = 0;
    if (index == 0) {
        component_count[0] = 0;
        organizer_count[0] = 0;
        undifferentiated_count[0] = 0;
        boundary_count[0] = 0;
        interior_count[0] = 0;
        area_q16[0] = 0;
        perimeter_q16[0] = 0;
        centroid_sum_x_q16[0] = 0;
        centroid_sum_y_q16[0] = 0;
    }
}

kernel void organism_reduce_morphology(
    const device uint* stable_id [[buffer(0)]],
    const device uint* fate [[buffer(1)]],
    const device packed_float3* position [[buffer(2)]],
    const device float* radius [[buffer(3)]],
    const device uint* surface_exposure_bin [[buffer(4)]],
    const device uint* component_label [[buffer(5)]],
    device atomic_uint* component_count [[buffer(6)]],
    device atomic_uint* organizer_count [[buffer(7)]],
    device atomic_uint* undifferentiated_count [[buffer(8)]],
    device atomic_uint* boundary_count [[buffer(9)]],
    device atomic_uint* interior_count [[buffer(10)]],
    device atomic_uint* area_q16 [[buffer(11)]],
    device atomic_uint* perimeter_q16 [[buffer(12)]],
    device atomic_int* centroid_sum_x_q16 [[buffer(13)]],
    device atomic_int* centroid_sum_y_q16 [[buffer(14)]],
    const device uint* active_count [[buffer(15)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    if (component_label[index] == stable_id[index]) {
        atomic_fetch_add_explicit(&component_count[0], 1u, memory_order_relaxed);
    }
    switch (min(fate[index], 3u)) {
        case 0: atomic_fetch_add_explicit(&organizer_count[0], 1u, memory_order_relaxed); break;
        case 1: atomic_fetch_add_explicit(&undifferentiated_count[0], 1u, memory_order_relaxed); break;
        case 2: atomic_fetch_add_explicit(&boundary_count[0], 1u, memory_order_relaxed); break;
        default: atomic_fetch_add_explicit(&interior_count[0], 1u, memory_order_relaxed); break;
    }
    const uint radius_q16 = q16_radius(radius[index]);
    const ulong radius_squared = ulong(radius_q16) * ulong(radius_q16);
    const uint cell_area_q16 =
        uint((radius_squared * 205887ul) / (ulong(PQO_Q16) * ulong(PQO_Q16)));
    const uint circumference_q16 =
        uint((ulong(radius_q16) * 411775ul) / ulong(PQO_Q16));
    const uint exposed_perimeter_q16 =
        uint(ulong(circumference_q16) * ulong(surface_exposure_bin[index]) /
             ulong(PQO_DECISION_MAX));
    atomic_fetch_add_explicit(&area_q16[0], cell_area_q16, memory_order_relaxed);
    atomic_fetch_add_explicit(
        &perimeter_q16[0], exposed_perimeter_q16, memory_order_relaxed);
    const int2 position_q16 = q16_position(float3(position[index]).xy);
    atomic_fetch_add_explicit(
        &centroid_sum_x_q16[0], position_q16.x, memory_order_relaxed);
    atomic_fetch_add_explicit(
        &centroid_sum_y_q16[0], position_q16.y, memory_order_relaxed);
}

kernel void organism_finalize_morphology(
    const device uint* active_count [[buffer(0)]],
    device uint* population [[buffer(1)]],
    const device uint* area_q16 [[buffer(2)]],
    const device uint* perimeter_q16 [[buffer(3)]],
    const device int* centroid_sum_x_q16 [[buffer(4)]],
    const device int* centroid_sum_y_q16 [[buffer(5)]],
    device int* centroid_x_q16 [[buffer(6)]],
    device int* centroid_y_q16 [[buffer(7)]],
    device uint* compactness_q16 [[buffer(8)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    const uint count = active_count[0];
    population[0] = count;
    centroid_x_q16[0] = count == 0 ? 0 : centroid_sum_x_q16[0] / int(count);
    centroid_y_q16[0] = count == 0 ? 0 : centroid_sum_y_q16[0] / int(count);
    const ulong perimeter_squared =
        ulong(perimeter_q16[0]) * ulong(perimeter_q16[0]);
    compactness_q16[0] = perimeter_squared == 0 ? 0u : uint(min(
        (ulong(area_q16[0]) * 823550ul * ulong(PQO_Q16)) / perimeter_squared,
        ulong(PQO_Q16)));
}

kernel void organism_reduce_radial_density(
    const device packed_float3* position [[buffer(0)]],
    const device int* centroid_x_q16 [[buffer(1)]],
    const device int* centroid_y_q16 [[buffer(2)]],
    device atomic_uint* radial_density [[buffer(3)]],
    const device uint* active_count [[buffer(4)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const int2 delta = q16_position(float3(position[index]).xy) -
        int2(centroid_x_q16[0], centroid_y_q16[0]);
    const long2 wide = long2(delta);
    const ulong distance_squared = ulong(wide.x * wide.x + wide.y * wide.y);
    uint radial_bin = 7;
    for (uint candidate = 0; candidate < 7; ++candidate) {
        const ulong threshold = ulong(candidate + 1u) * 8192ul;
        if (distance_squared < threshold * threshold) {
            radial_bin = candidate;
            break;
        }
    }
    atomic_fetch_add_explicit(&radial_density[radial_bin], 1u, memory_order_relaxed);
}

kernel void organism_prequalify_population(
    const device uint* active_count [[buffer(0)]],
    const device packed_float3* position [[buffer(1)]],
    const device float* radius [[buffer(2)]],
    const device uint* stable_id [[buffer(3)]],
    const device uint* age [[buffer(4)]],
    const device uint* surface_mask [[buffer(5)]],
    const device uint* divide [[buffer(6)]],
    const device uint* death [[buffer(7)]],
    const device uint* repair [[buffer(8)]],
    const device float* injury [[buffer(9)]],
    const device float* density [[buffer(10)]],
    constant uint& field_width [[buffer(11)]],
    const device uint* bin_count [[buffer(12)]],
    const device uint* bin_indices [[buffer(13)]],
    device uint* survival [[buffer(14)]],
    device uint* birth [[buffer(15)]],
    device uint* candidate_sector [[buffer(16)]],
    device atomic_uint* overflow [[buffer(17)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) {
        survival[index] = 0;
        birth[index] = 0;
        candidate_sector[index] = UINT_MAX;
        return;
    }
    const bool survives = death[index] == 0;
    survival[index] = survives ? 1u : 0u;
    birth[index] = 0;
    candidate_sector[index] = UINT_MAX;
    if (!survives || divide[index] == 0) return;

    const float2 parent = float3(position[index]).xy;
    const uint parent_injury =
        decision_bin(sample_field_bilinear(injury, parent, field_width), 16.0f);
    const uint available = (~surface_mask[index]) & 255u;
    const uint start =
        (stable_id[index] * 2654435761u + age[index] * 2246822519u) & 7u;
    uint best_sector = UINT_MAX;
    uint best_contacts = 0u;
    uint best_density = 0u;
    uint best_injury = 0u;
    for (uint rank = 0; rank < 8u; ++rank) {
        const uint sector = (start + rank) & 7u;
        if ((available & (1u << sector)) == 0u) continue;
        const float2 candidate = daughter_position(parent, radius[index], sector);
        if (any(abs(candidate) > 1.0f - radius[index])) continue;
        if (!quantized_contact(parent, radius[index], candidate, radius[index])) continue;

        const int2 center = spatial_cell(candidate);
        uint observed = 0;
        uint candidate_contacts = 0u;
        bool invalid = false;
        for (int y = -1; y <= 1 && !invalid; ++y) {
            for (int x = -1; x <= 1 && !invalid; ++x) {
                const int2 cell = center + int2(x, y);
                if (any(cell < 0) || any(cell >= int(PQO_SPATIAL_AXIS))) continue;
                const uint key = uint(cell.y) * PQO_SPATIAL_AXIS + uint(cell.x);
                const uint length = min(bin_count[key], PQO_BIN_CAPACITY);
                const uint base = key * PQO_BIN_CAPACITY;
                for (uint item = 0; item < length; ++item) {
                    const uint other = bin_indices[base + item];
                    if (other == index) continue;
                    if (observed++ >= PQO_BIN_CAPACITY) {
                        atomic_fetch_add_explicit(
                            &overflow[0], 1u, memory_order_relaxed);
                        return;
                    }
                    invalid = quantized_overlap(
                        candidate, radius[index],
                        float3(position[other]).xy, radius[other]);
                    if (invalid) break;
                    candidate_contacts += uint(quantized_contact(
                        candidate, radius[index],
                        float3(position[other]).xy, radius[other]));
                }
            }
        }
        if (!invalid) {
            if (repair[index] != 0u) {
                const uint candidate_injury =
                    decision_bin(
                        sample_field_bilinear(injury, candidate, field_width),
                        16.0f);
                const uint candidate_density =
                    decision_bin(
                        sample_field_bilinear(density, candidate, field_width),
                        16.0f);
                if (candidate_injury >= 8u &&
                    candidate_injury > parent_injury &&
                    (candidate_contacts > best_contacts ||
                     (candidate_contacts == best_contacts &&
                      (candidate_density > best_density ||
                       (candidate_density == best_density &&
                        candidate_injury > best_injury))))) {
                    best_contacts = candidate_contacts;
                    best_density = candidate_density;
                    best_injury = candidate_injury;
                    best_sector = sector;
                }
                continue;
            }
            candidate_sector[index] = sector;
            birth[index] = 1u;
            return;
        }
    }
    if (best_sector != UINT_MAX) {
        candidate_sector[index] = best_sector;
        birth[index] = 1u;
    }
}

kernel void organism_bin_candidates(
    const device uint* active_count [[buffer(0)]],
    const device packed_float3* position [[buffer(1)]],
    const device float* radius [[buffer(2)]],
    const device uint* stable_id [[buffer(3)]],
    const device uint* birth [[buffer(4)]],
    const device uint* candidate_sector [[buffer(5)]],
    device atomic_uint* count [[buffer(6)]],
    device uint* indices [[buffer(7)]],
    device atomic_uint* overflow [[buffer(8)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0] || birth[index] == 0) return;
    const float2 candidate = daughter_position(
        float3(position[index]).xy, radius[index], candidate_sector[index]);
    const uint key = spatial_key(candidate);
    const uint slot = atomic_fetch_add_explicit(&count[key], 1u, memory_order_relaxed);
    if (slot < PQO_BIN_CAPACITY) {
        indices[key * PQO_BIN_CAPACITY + slot] = index;
    } else {
        atomic_fetch_add_explicit(&overflow[0], 1u, memory_order_relaxed);
    }
}

kernel void organism_resolve_candidate_conflicts(
    const device uint* active_count [[buffer(0)]],
    const device packed_float3* position [[buffer(1)]],
    const device float* radius [[buffer(2)]],
    const device uint* stable_id [[buffer(3)]],
    const device uint* prequalified [[buffer(4)]],
    const device uint* candidate_sector [[buffer(5)]],
    const device uint* bin_count [[buffer(6)]],
    const device uint* bin_indices [[buffer(7)]],
    device uint* birth [[buffer(8)]],
    device atomic_uint* overflow [[buffer(9)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0] || prequalified[index] == 0) {
        birth[index] = 0;
        return;
    }
    const float2 candidate = daughter_position(
        float3(position[index]).xy, radius[index], candidate_sector[index]);
    const int2 center = spatial_cell(candidate);
    uint observed = 0;
    bool blocked = false;
    bool exceeded = false;
    for (int y = -1; y <= 1 && !blocked; ++y) {
        for (int x = -1; x <= 1 && !blocked; ++x) {
            const int2 cell = center + int2(x, y);
            if (any(cell < 0) || any(cell >= int(PQO_SPATIAL_AXIS))) continue;
            const uint key = uint(cell.y) * PQO_SPATIAL_AXIS + uint(cell.x);
            const uint length = min(bin_count[key], PQO_BIN_CAPACITY);
            const uint base = key * PQO_BIN_CAPACITY;
            for (uint item = 0; item < length; ++item) {
                const uint other = bin_indices[base + item];
                if (other == index || stable_id[other] >= stable_id[index]) continue;
                if (observed++ >= PQO_BIN_CAPACITY) {
                    exceeded = true;
                    blocked = true;
                    break;
                }
                const float2 other_candidate = daughter_position(
                    float3(position[other]).xy, radius[other],
                    candidate_sector[other]);
                blocked = quantized_overlap(
                    candidate, radius[index], other_candidate, radius[other]);
                if (blocked) break;
            }
        }
    }
    if (exceeded) {
        atomic_fetch_add_explicit(&overflow[0], 1u, memory_order_relaxed);
    }
    birth[index] = blocked ? 0u : 1u;
}

kernel void organism_scan_population_blocks(
    const device uint* active_count [[buffer(0)]],
    const device uint* order [[buffer(1)]],
    const device uint* survival [[buffer(2)]],
    const device uint* birth [[buffer(3)]],
    device uint* survival_prefix [[buffer(4)]],
    device uint* birth_prefix [[buffer(5)]],
    device uint* survival_block_sum [[buffer(6)]],
    device uint* birth_block_sum [[buffer(7)]],
    uint global [[thread_position_in_grid]],
    uint local [[thread_index_in_threadgroup]],
    uint group [[threadgroup_position_in_grid]])
{
    threadgroup uint survival_scan[PQO_SCAN_BLOCK];
    threadgroup uint birth_scan[PQO_SCAN_BLOCK];
    const bool active = global < active_count[0];
    const uint source = active ? order[global] : UINT_MAX;
    survival_scan[local] = active ? survival[source] : 0u;
    birth_scan[local] = active ? birth[source] : 0u;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint offset = 1; offset < PQO_SCAN_BLOCK; offset <<= 1) {
        const uint survival_add = local >= offset ? survival_scan[local - offset] : 0u;
        const uint birth_add = local >= offset ? birth_scan[local - offset] : 0u;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        survival_scan[local] += survival_add;
        birth_scan[local] += birth_add;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    survival_prefix[global] = survival_scan[local];
    birth_prefix[global] = birth_scan[local];
    if (local + 1 == PQO_SCAN_BLOCK) {
        survival_block_sum[group] = survival_scan[local];
        birth_block_sum[group] = birth_scan[local];
    }
}

kernel void organism_scan_population_block_sums(
    const device uint* survival_block_sum [[buffer(0)]],
    const device uint* birth_block_sum [[buffer(1)]],
    device uint* survival_block_prefix [[buffer(2)]],
    device uint* birth_block_prefix [[buffer(3)]],
    constant uint& block_count [[buffer(4)]],
    uint local [[thread_index_in_threadgroup]])
{
    threadgroup uint survival_scan[PQO_SCAN_BLOCK];
    threadgroup uint birth_scan[PQO_SCAN_BLOCK];
    survival_scan[local] = local < block_count ? survival_block_sum[local] : 0u;
    birth_scan[local] = local < block_count ? birth_block_sum[local] : 0u;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint offset = 1; offset < PQO_SCAN_BLOCK; offset <<= 1) {
        const uint survival_add = local >= offset ? survival_scan[local - offset] : 0u;
        const uint birth_add = local >= offset ? birth_scan[local - offset] : 0u;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        survival_scan[local] += survival_add;
        birth_scan[local] += birth_add;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (local < block_count) {
        survival_block_prefix[local] = survival_scan[local];
        birth_block_prefix[local] = birth_scan[local];
    }
}

kernel void organism_add_population_block_offsets(
    const device uint* active_count [[buffer(0)]],
    device uint* survival_prefix [[buffer(1)]],
    device uint* birth_prefix [[buffer(2)]],
    const device uint* survival_block_prefix [[buffer(3)]],
    const device uint* birth_block_prefix [[buffer(4)]],
    uint global [[thread_position_in_grid]],
    uint group [[threadgroup_position_in_grid]])
{
    if (global >= active_count[0] || group == 0) return;
    survival_prefix[global] += survival_block_prefix[group - 1];
    birth_prefix[global] += birth_block_prefix[group - 1];
}

kernel void organism_resolve_population_counts(
    const device uint* active_count [[buffer(0)]],
    const device uint* survival [[buffer(1)]],
    const device uint* birth [[buffer(2)]],
    const device uint* survival_prefix [[buffer(3)]],
    const device uint* birth_prefix [[buffer(4)]],
    device uint* survivor_count [[buffer(5)]],
    device uint* accepted_birth_count [[buffer(6)]],
    device uint* next_count [[buffer(7)]],
    device uint* rejected_births [[buffer(8)]],
    constant uint& capacity [[buffer(9)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    const uint count = active_count[0];
    const uint survivors = count == 0 ? 0u : survival_prefix[count - 1];
    const uint requested = count == 0 ? 0u : birth_prefix[count - 1];
    const uint available = capacity > survivors ? capacity - survivors : 0u;
    const uint accepted = min(requested, available);
    survivor_count[0] = survivors;
    accepted_birth_count[0] = accepted;
    next_count[0] = survivors + accepted;
    rejected_births[0] = requested - accepted;
}

kernel void organism_scatter_population_core(
    const device uint* active_count [[buffer(0)]],
    const device uint* order [[buffer(1)]],
    const device uint* stable_id [[buffer(2)]],
    const device uint* parent_id [[buffer(3)]],
    const device packed_float3* position [[buffer(4)]],
    const device float* radius [[buffer(5)]],
    const device float* energy [[buffer(6)]],
    const device uint* candidate_sector [[buffer(7)]],
    const device uint* survival [[buffer(8)]],
    const device uint* birth [[buffer(9)]],
    const device uint* survival_prefix [[buffer(10)]],
    const device uint* birth_prefix [[buffer(11)]],
    const device uint* survivor_count [[buffer(12)]],
    const device uint* accepted_birth_count [[buffer(13)]],
    const device uint* next_stable_id [[buffer(14)]],
    device uint* stage_stable_id [[buffer(15)]],
    device uint* stage_parent_id [[buffer(16)]],
    device packed_float3* stage_position [[buffer(17)]],
    device float* stage_radius [[buffer(18)]],
    device float* stage_energy [[buffer(19)]],
    const device uint* event_hash [[buffer(20)]],
    device uint* stage_event_hash [[buffer(21)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const uint source = order[index];
    const bool accepted_birth =
        birth[source] != 0 && birth_prefix[index] <= accepted_birth_count[0];
    if (survival[source] != 0) {
        const uint destination = survival_prefix[index] - 1;
        stage_stable_id[destination] = stable_id[source];
        stage_parent_id[destination] = parent_id[source];
        stage_position[destination] = position[source];
        stage_radius[destination] = radius[source];
        stage_energy[destination] =
            accepted_birth ? max(0.0f, energy[source] - 1.1f) : energy[source];
        stage_event_hash[destination] = event_hash[source];
        if (accepted_birth) {
            const uint rank = birth_prefix[index] - 1;
            const uint child_id = next_stable_id[0] + rank;
            stage_event_hash[destination] = mix_event(
                stage_event_hash[destination], 5u, stable_id[source], child_id);
        }
    }
    if (accepted_birth) {
        const uint rank = birth_prefix[index] - 1;
        const uint destination = survivor_count[0] + rank;
        stage_stable_id[destination] = next_stable_id[0] + rank;
        stage_parent_id[destination] = stable_id[source];
        stage_position[destination] = packed_float3(
            daughter_position(
                float3(position[source]).xy, radius[source],
                candidate_sector[source]),
            0.0f);
        stage_radius[destination] = radius[source];
        stage_energy[destination] = 1.0f;
        stage_event_hash[destination] = mix_event(
            2166136261u, 6u, stable_id[source], next_stable_id[0] + rank);
    }
}

kernel void organism_scatter_population_development(
    const device uint* active_count [[buffer(0)]],
    const device uint* order [[buffer(1)]],
    const device uint* age [[buffer(2)]],
    const device uint* fate [[buffer(3)]],
    const device uint* phase [[buffer(4)]],
    const device uint* health [[buffer(5)]],
    const device uint* previous_fate [[buffer(6)]],
    const device uint* fate_confidence [[buffer(7)]],
    const device uint* time_in_fate [[buffer(8)]],
    const device uint* recent_activator [[buffer(9)]],
    const device uint* recent_inhibitor [[buffer(10)]],
    const device uint* recent_surface_exposure [[buffer(11)]],
    const device float4* color [[buffer(12)]],
    const device uint* survival [[buffer(13)]],
    const device uint* birth [[buffer(14)]],
    const device uint* survival_prefix [[buffer(15)]],
    const device uint* birth_prefix [[buffer(16)]],
    const device uint* survivor_count [[buffer(17)]],
    const device uint* accepted_birth_count [[buffer(18)]],
    const device uint* repair [[buffer(19)]],
    device uint* stage_age [[buffer(20)]],
    device uint* stage_fate [[buffer(21)]],
    device uint* stage_phase [[buffer(22)]],
    device uint* stage_health [[buffer(23)]],
    device uint* stage_previous_fate [[buffer(24)]],
    device uint* stage_fate_confidence [[buffer(25)]],
    device uint* stage_time_in_fate [[buffer(26)]],
    device uint* stage_recent_activator [[buffer(27)]],
    device uint* stage_recent_inhibitor [[buffer(28)]],
    device uint* stage_recent_surface_exposure [[buffer(29)]],
    device float4* stage_color [[buffer(30)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= active_count[0]) return;
    const uint source = order[index];
    const bool accepted_birth =
        birth[source] != 0 && birth_prefix[index] <= accepted_birth_count[0];
    if (survival[source] != 0) {
        const uint destination = survival_prefix[index] - 1;
        stage_age[destination] = accepted_birth ? 0u : age[source];
        stage_fate[destination] = fate[source];
        stage_phase[destination] = phase[source];
        stage_health[destination] = health[source];
        stage_previous_fate[destination] = previous_fate[source];
        stage_fate_confidence[destination] = fate_confidence[source];
        stage_time_in_fate[destination] = time_in_fate[source];
        stage_recent_activator[destination] = recent_activator[source];
        stage_recent_inhibitor[destination] = recent_inhibitor[source];
        stage_recent_surface_exposure[destination] =
            recent_surface_exposure[source];
        stage_color[destination] = color[source];
    }
    if (accepted_birth) {
        const uint destination = survivor_count[0] + birth_prefix[index] - 1;
        stage_age[destination] = 0;
        stage_fate[destination] = 1;
        stage_phase[destination] = repair[source] != 0u ? 1u : 0u;
        stage_health[destination] = repair[source] != 0u ? 1u : 0u;
        stage_previous_fate[destination] = 1;
        stage_fate_confidence[destination] = 0;
        stage_time_in_fate[destination] = 0;
        stage_recent_activator[destination] = recent_activator[source];
        stage_recent_inhibitor[destination] = recent_inhibitor[source];
        stage_recent_surface_exposure[destination] =
            recent_surface_exposure[source];
        stage_color[destination] = float4(0.8, 0.8, 0.9, 1.0);
    }
}

kernel void organism_commit_population_core(
    const device uint* next_count [[buffer(0)]],
    const device uint* stage_stable_id [[buffer(1)]],
    const device uint* stage_parent_id [[buffer(2)]],
    const device packed_float3* stage_position [[buffer(3)]],
    const device float* stage_radius [[buffer(4)]],
    const device float* stage_energy [[buffer(5)]],
    device uint* stable_id [[buffer(6)]],
    device uint* parent_id [[buffer(7)]],
    device packed_float3* position [[buffer(8)]],
    device float* radius [[buffer(9)]],
    device float* energy [[buffer(10)]],
    const device uint* stage_event_hash [[buffer(11)]],
    device uint* event_hash [[buffer(12)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= next_count[0]) return;
    stable_id[index] = stage_stable_id[index];
    parent_id[index] = stage_parent_id[index];
    position[index] = stage_position[index];
    radius[index] = stage_radius[index];
    energy[index] = stage_energy[index];
    event_hash[index] = stage_event_hash[index];
}

kernel void organism_commit_population_development(
    const device uint* next_count [[buffer(0)]],
    const device uint* stage_age [[buffer(1)]],
    const device uint* stage_fate [[buffer(2)]],
    const device uint* stage_phase [[buffer(3)]],
    const device uint* stage_health [[buffer(4)]],
    const device uint* stage_previous_fate [[buffer(5)]],
    const device uint* stage_fate_confidence [[buffer(6)]],
    const device uint* stage_time_in_fate [[buffer(7)]],
    const device uint* stage_recent_activator [[buffer(8)]],
    const device uint* stage_recent_inhibitor [[buffer(9)]],
    const device uint* stage_recent_surface_exposure [[buffer(10)]],
    const device float4* stage_color [[buffer(11)]],
    device uint* age [[buffer(12)]],
    device uint* fate [[buffer(13)]],
    device uint* phase [[buffer(14)]],
    device uint* health [[buffer(15)]],
    device uint* previous_fate [[buffer(16)]],
    device uint* fate_confidence [[buffer(17)]],
    device uint* time_in_fate [[buffer(18)]],
    device uint* recent_activator [[buffer(19)]],
    device uint* recent_inhibitor [[buffer(20)]],
    device uint* recent_surface_exposure [[buffer(21)]],
    device float4* color [[buffer(22)]],
    uint index [[thread_position_in_grid]])
{
    if (index >= next_count[0]) return;
    age[index] = stage_age[index];
    fate[index] = stage_fate[index];
    phase[index] = stage_phase[index];
    health[index] = stage_health[index];
    previous_fate[index] = stage_previous_fate[index];
    fate_confidence[index] = stage_fate_confidence[index];
    time_in_fate[index] = stage_time_in_fate[index];
    recent_activator[index] = stage_recent_activator[index];
    recent_inhibitor[index] = stage_recent_inhibitor[index];
    recent_surface_exposure[index] = stage_recent_surface_exposure[index];
    color[index] = stage_color[index];
}

kernel void organism_finalize_population(
    device uint* active_count [[buffer(0)]],
    device uint* next_stable_id [[buffer(1)]],
    const device uint* next_count [[buffer(2)]],
    const device uint* accepted_birth_count [[buffer(3)]],
    uint index [[thread_position_in_grid]])
{
    if (index != 0) return;
    active_count[0] = next_count[0];
    next_stable_id[0] += accepted_birth_count[0];
}
