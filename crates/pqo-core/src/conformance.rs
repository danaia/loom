use crate::{
    builder::*,
    model::{
        DataType, DeterminismContract, DeterminismScope, DeterminismTier, ExcessWallTimePolicy,
        Literal, Metric, OverloadPolicy, PresentationLifetimePolicy, Quantity, QueueModel,
        RenderOverloadPolicy, ReplayOverloadPolicy, ResourceAccess, ScenarioTimePolicy,
        SimulationTimePolicy, SlotAccess, SnapshotSemantics, Target, TickOverlapPolicy, Unit,
        ViewState,
    },
};

#[derive(Clone, Debug)]
pub struct HelloParticleConfig {
    pub module_name: &'static str,
    pub particle_count: u32,
    pub particle_buffering: u32,
    pub simulation_ticks_in_flight: u32,
    pub tick_overlap: TickOverlapPolicy,
    pub presentation_lifetime: PresentationLifetimePolicy,
    pub queue_model: QueueModel,
}

impl Default for HelloParticleConfig {
    fn default() -> Self {
        Self {
            module_name: "hello_particle",
            particle_count: 1,
            particle_buffering: 1,
            simulation_ticks_in_flight: 4,
            tick_overlap: TickOverlapPolicy::SerializeConflictingTicks,
            presentation_lifetime: PresentationLifetimePolicy::BlockNextTickUntilViewsComplete,
            queue_model: QueueModel::Unproven,
        }
    }
}

impl HelloParticleConfig {
    pub fn unsafe_unproven_overlap() -> Self {
        Self {
            tick_overlap: TickOverlapPolicy::RequireResourceVersions,
            ..Self::default()
        }
    }
}

pub fn hello_batch_builder(particle_count: u32) -> ModuleBuilder {
    hello_particle_builder(HelloParticleConfig {
        module_name: "hello_batch",
        particle_count,
        tick_overlap: TickOverlapPolicy::QueueOrderedReuse,
        presentation_lifetime: PresentationLifetimePolicy::QueueOrderedReuse,
        queue_model: QueueModel::SingleSerialQueue,
        ..HelloParticleConfig::default()
    })
}

pub fn hello_particle_builder(config: HelloParticleConfig) -> ModuleBuilder {
    let particle_count = config.particle_count;
    let vec3 = DataType::vec3_f32();
    let vec4 = DataType::vec4_f32();
    let f32_type = DataType::f32();

    let overload = OverloadPolicy {
        catch_up_limit: 4,
        excess_wall_time: ExcessWallTimePolicy::Discard,
        simulation_time: SimulationTimePolicy::AdvanceExecutedTicksOnly,
        scenario_time: ScenarioTimePolicy::SimulationTicks,
        replay: ReplayOverloadPolicy::RecordDecisions,
        rendering: RenderOverloadPolicy::DropPresentationOnly,
    };

    let vector =
        |values: &[f32]| Literal::Vector(values.iter().copied().map(Literal::f32).collect());
    let columns = (particle_count as f32).sqrt().ceil().max(1.0) as u32;
    let rows = particle_count.div_ceil(columns);
    let position_column_step = if columns > 1 {
        1.8 / (columns - 1) as f32
    } else {
        0.0
    };
    let position_row_step = if rows > 1 {
        0.8 / (rows - 1) as f32
    } else {
        0.0
    };
    let color_scale = 1.0 / particle_count.max(1) as f32;

    ModuleBuilder::new(config.module_name)
        .target(Target::Metal)
        .value(ValueDraft::constant(
            "world.gravity",
            vec3.clone(),
            Unit::METERS_PER_SECOND_SQUARED,
            Literal::Vector(vec![
                Literal::f32(0.0),
                Literal::f32(-9.81),
                Literal::f32(0.0),
            ]),
        ))
        .value(ValueDraft::constant(
            "ground.height",
            f32_type.clone(),
            Unit::METER,
            Literal::f32(0.0),
        ))
        .stream(
            StreamDraft::new("particles.position", vec3.clone(), Unit::METER)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .initial_grid_2d(
                    vector(&[-0.9, 0.15, 0.0]),
                    vector(&[position_column_step, 0.0, 0.0]),
                    vector(&[0.0, position_row_step, 0.0]),
                    columns,
                    particle_count,
                ),
        )
        .stream(
            StreamDraft::new("particles.velocity", vec3.clone(), Unit::METERS_PER_SECOND)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .initial_repeat(vector(&[0.0, 0.0, 0.0]), particle_count),
        )
        .stream(
            StreamDraft::new("particles.radius", f32_type.clone(), Unit::METER)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .access(ResourceAccess::DeviceRead)
                .initial_repeat(Literal::f32(0.004), particle_count),
        )
        .stream(
            StreamDraft::new(
                "particles.restitution",
                f32_type.clone(),
                Unit::DIMENSIONLESS,
            )
            .capacity(particle_count)
            .length(particle_count)
            .buffering(config.particle_buffering)
            .access(ResourceAccess::DeviceRead)
            .initial_repeat(Literal::f32(0.8), particle_count),
        )
        .stream(
            StreamDraft::new("particles.friction", f32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .access(ResourceAccess::DeviceRead)
                .initial_repeat(Literal::f32(0.3), particle_count),
        )
        .stream(
            StreamDraft::new("particles.color", vec4.clone(), Unit::DIMENSIONLESS)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .access(ResourceAccess::DeviceRead)
                .initial_linear(
                    vector(&[0.4, 0.7, 1.0, 1.0]),
                    vector(&[
                        0.6 * color_scale,
                        -0.5 * color_scale,
                        -0.5 * color_scale,
                        0.0,
                    ]),
                    particle_count,
                ),
        )
        .kernel(
            KernelDraft::new("integrate")
                .slot(SlotDraft::stream(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::ReadWrite,
                ))
                .slot(SlotDraft::stream(
                    "velocity",
                    vec3.clone(),
                    Unit::METERS_PER_SECOND,
                    SlotAccess::ReadWrite,
                ))
                .slot(SlotDraft::value(
                    "gravity",
                    vec3.clone(),
                    Unit::METERS_PER_SECOND_SQUARED,
                ))
                .slot(SlotDraft::value("dt", f32_type.clone(), Unit::SECOND))
                .abi(KernelAbiDraft::new([
                    "position", "velocity", "gravity", "dt",
                ]))
                .implementation(metal_implementation(
                    "kernels/euler_integrate.metal",
                    "integrate_main",
                )),
        )
        .kernel(
            KernelDraft::new("contact_ground")
                .slot(SlotDraft::stream(
                    "position",
                    vec3.clone(),
                    Unit::METER,
                    SlotAccess::ReadWrite,
                ))
                .slot(SlotDraft::stream(
                    "velocity",
                    vec3,
                    Unit::METERS_PER_SECOND,
                    SlotAccess::ReadWrite,
                ))
                .slot(SlotDraft::stream(
                    "radius",
                    f32_type.clone(),
                    Unit::METER,
                    SlotAccess::Read,
                ))
                .slot(SlotDraft::stream(
                    "restitution",
                    f32_type.clone(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Read,
                ))
                .slot(SlotDraft::stream(
                    "friction",
                    f32_type.clone(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Read,
                ))
                .slot(SlotDraft::value("ground_height", f32_type, Unit::METER))
                .abi(KernelAbiDraft::new([
                    "position",
                    "velocity",
                    "radius",
                    "restitution",
                    "friction",
                    "ground_height",
                ]))
                .implementation(metal_implementation(
                    "kernels/ground_contact.metal",
                    "ground_contact_main",
                )),
        )
        .pass(
            PassDraft::new("fall", "integrate")
                .bind("position", "particles.position")
                .bind("velocity", "particles.velocity")
                .bind("gravity", "world.gravity")
                .bind("dt", "simulation.fixed_dt")
                .dispatch_over("particles.position"),
        )
        .pass(
            PassDraft::new("bounce", "contact_ground")
                .bind("position", "particles.position")
                .bind("velocity", "particles.velocity")
                .bind("radius", "particles.radius")
                .bind("restitution", "particles.restitution")
                .bind("friction", "particles.friction")
                .bind("ground_height", "ground.height")
                .dispatch_over("particles.position"),
        )
        .view(
            ViewDraft::render(
                "viewport",
                metal_implementation("shaders/particle.metal", "particle_pipeline"),
            )
            .read("position", "particles.position")
            .read("radius", "particles.radius")
            .read("color", "particles.color")
            .state(ViewState::CurrentCompletedTick),
        )
        .schedule(
            ScheduleDraft::fixed("simulation", 120)
                .run("fall")
                .run_after("bounce", "fall")
                .show_after("viewport", "bounce")
                .in_flight(config.simulation_ticks_in_flight, 2)
                .tick_overlap(config.tick_overlap)
                .presentation_lifetime(config.presentation_lifetime)
                .queue_model(config.queue_model)
                .overload(overload),
        )
        .contract(
            ContractDraft::new("physically_valid", "simulation")
                .clause(ContractClauseDraft::Invariant {
                    observation: ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    predicate: PredicateDraft::FiniteStreams(vec![
                        "particles.position".to_owned(),
                        "particles.velocity".to_owned(),
                    ]),
                })
                .clause(ContractClauseDraft::Invariant {
                    observation: ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    predicate: PredicateDraft::GroundClearance {
                        position: "particles.position".to_owned(),
                        radius: "particles.radius".to_owned(),
                        ground_height: "ground.height".to_owned(),
                        tolerance: Quantity {
                            value: Literal::f32(0.0001),
                            unit: Unit::METER,
                        },
                    },
                }),
        )
        .contract(
            ContractDraft::new("realtime", "simulation")
                .clause(ContractClauseDraft::SteadyStateZero {
                    observation: ObservationDraft::AfterGpuCompletion("simulation".to_owned()),
                    metric: Metric::HeapAllocationsPerTick,
                    excludes_requested_inspection: true,
                })
                .clause(ContractClauseDraft::SteadyStateZero {
                    observation: ObservationDraft::AfterGpuCompletion("simulation".to_owned()),
                    metric: Metric::ApplicationCopiesPerTick,
                    excludes_requested_inspection: true,
                })
                .clause(ContractClauseDraft::SteadyStateZero {
                    observation: ObservationDraft::AfterGpuCompletion("simulation".to_owned()),
                    metric: Metric::ApplicationBlitsPerTick,
                    excludes_requested_inspection: true,
                })
                .clause(ContractClauseDraft::MetricLimit {
                    observation: ObservationDraft::AfterGpuCompletion("simulation".to_owned()),
                    metric: Metric::GpuTimePerTick,
                    maximum: Quantity {
                        value: Literal::f32(8.33),
                        unit: Unit::new(0, 0, 1, -3),
                    },
                }),
        )
        .contract(ContractDraft::new("replay", "simulation").clause(
            ContractClauseDraft::Determinism(DeterminismContract {
                tier: DeterminismTier::Tier1,
                scope: DeterminismScope::ExactExecutionFingerprint,
            }),
        ))
        .scenario(
            ScenarioDraft::new("drop_and_bounce", "simulation", 600)
                .expect(
                    ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    PredicateDraft::GroundClearance {
                        position: "particles.position".to_owned(),
                        radius: "particles.radius".to_owned(),
                        ground_height: "ground.height".to_owned(),
                        tolerance: Quantity {
                            value: Literal::f32(0.0001),
                            unit: Unit::METER,
                        },
                    },
                )
                .expect(
                    ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    PredicateDraft::FiniteStreams(vec![
                        "particles.position".to_owned(),
                        "particles.velocity".to_owned(),
                    ]),
                ),
        )
        .benchmark(
            BenchmarkDraft::new("baseline", "simulation")
                .ticks(120, 600)
                .measure(Metric::GpuTimePerTick)
                .measure(Metric::HeapAllocationsPerTick)
                .measure(Metric::ApplicationCopiesPerTick)
                .measure(Metric::ApplicationBlitsPerTick)
                .measure(Metric::WorkingSetBytes)
                .measure(Metric::OverloadEvents),
        )
        .capability(CapabilityDraft::inspect(
            "inspect_particle_state",
            ["particles.position", "particles.velocity"],
            SnapshotSemantics::NextGpuCompletedTickAfterRequest,
        ))
}

/// First emergent-systems substrate specimen: a GPU-resident dynamic population
/// whose active length is a mutable stream and whose committed state is protected
/// by explicit mutation authority.
pub fn hello_population_builder(capacity: u32, initial_count: u32) -> ModuleBuilder {
    assert!(capacity > 0);
    assert!(initial_count <= capacity);
    const SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;
kernel void population_age(
    device uint* age [[buffer(0)]],
    uint index [[thread_position_in_grid]])
{
    age[index] += 1;
}

kernel void population_reset_age(
    device uint* age [[buffer(0)]],
    constant uint& reset_age [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    age[index] = reset_age;
}
"#;

    ModuleBuilder::new("hello_population")
        .target(Target::Metal)
        .value(ValueDraft::constant(
            "population.reset_age",
            DataType::u32(),
            Unit::DIMENSIONLESS,
            Literal::U32(0),
        ))
        .stream(
            StreamDraft::new(
                "population.active_count",
                DataType::u32(),
                Unit::DIMENSIONLESS,
            )
            .capacity(1)
            .length(1)
            .write_authority("mutate_population_membership")
            .initial(Literal::Array(vec![Literal::U32(initial_count)])),
        )
        .stream(
            StreamDraft::new("population.age", DataType::u32(), Unit::DIMENSIONLESS)
                .capacity(capacity)
                .dynamic_length("population.active_count")
                .write_authority("mutate_population_state")
                .initial_repeat(Literal::U32(0), initial_count),
        )
        .kernel(
            KernelDraft::new("population_age")
                .slot(SlotDraft::stream(
                    "age",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::ReadWrite,
                ))
                .abi(KernelAbiDraft::new(["age"]))
                .implementation(packaged_metal_implementation(
                    "pqo://specimens/hello_population.metal",
                    "population_age",
                    SOURCE,
                )),
        )
        .kernel(
            KernelDraft::new("population_reset_age")
                .slot(SlotDraft::stream(
                    "age",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Write,
                ))
                .slot(SlotDraft::value(
                    "reset_age",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                ))
                .abi(KernelAbiDraft::new(["age", "reset_age"]))
                .implementation(packaged_metal_implementation(
                    "pqo://specimens/hello_population.metal",
                    "population_reset_age",
                    SOURCE,
                )),
        )
        .pass(
            PassDraft::new("age_population", "population_age")
                .bind("age", "population.age")
                .dispatch_over("population.age")
                .grant("mutate_population_state"),
        )
        .pass(
            PassDraft::new("reset_population_age", "population_reset_age")
                .bind("age", "population.age")
                .bind("reset_age", "population.reset_age")
                .dispatch_over("population.age")
                .grant("mutate_population_state"),
        )
        .schedule(
            ScheduleDraft::fixed("simulation", 120)
                .run("age_population")
                .tick_overlap(TickOverlapPolicy::QueueOrderedReuse)
                .presentation_lifetime(PresentationLifetimePolicy::QueueOrderedReuse)
                .queue_model(QueueModel::SingleSerialQueue),
        )
        .contract(ContractDraft::new("logical_replay", "simulation").clause(
            ContractClauseDraft::Determinism(DeterminismContract {
                tier: DeterminismTier::Tier1,
                scope: DeterminismScope::ExactExecutionFingerprint,
            }),
        ))
        .scenario(
            ScenarioDraft::new("recorded_reset", "simulation", 2).intervene(
                1,
                "reset_population_age",
                [("population.reset_age", Literal::U32(42))],
            ),
        )
        .capability(CapabilityDraft::membership_mutate(
            "mutate_population_membership",
            "population.active_count",
            ["population.age"],
        ))
        .capability(CapabilityDraft::state_mutate(
            "mutate_population_state",
            ["population.age"],
        ))
}

/// Independent deterministic field-computation specimen.
pub fn hello_field_builder() -> ModuleBuilder {
    const WIDTH: u32 = 256;
    const CELL_COUNT: u32 = WIDTH * WIDTH;
    const SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

kernel void clear_deposit(
    device uint* deposit [[buffer(0)]],
    uint index [[thread_position_in_grid]])
{
    deposit[index] = 0;
}

kernel void seed_deposit(
    device uint* deposit [[buffer(0)]],
    constant uint& width [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    if (index == 0) {
        deposit[(width / 2) * width + width / 2] = 65536;
    }
}

kernel void diffuse_reflective(
    const device float* current [[buffer(0)]],
    const device uint* deposit [[buffer(1)]],
    device float* next [[buffer(2)]],
    constant float& alpha [[buffer(3)]],
    constant float& decay [[buffer(4)]],
    constant float& maximum [[buffer(5)]],
    constant uint& width [[buffer(6)]],
    uint index [[thread_position_in_grid]])
{
    const uint x = index % width;
    const uint y = index / width;
    const uint left_x = x == 0 ? 0 : x - 1;
    const uint right_x = x + 1 == width ? x : x + 1;
    const uint down_y = y == 0 ? 0 : y - 1;
    const uint up_y = y + 1 == width ? y : y + 1;
    const float center = current[index];
    const float laplacian =
        current[y * width + left_x] +
        current[y * width + right_x] +
        current[down_y * width + x] +
        current[up_y * width + x] -
        4.0f * center;
    const float source = float(deposit[index]) / 65536.0f;
    next[index] = clamp(
        center + alpha * laplacian - decay * center + source,
        0.0f,
        maximum);
}

kernel void commit_field(
    device float* current [[buffer(0)]],
    const device float* next [[buffer(1)]],
    uint index [[thread_position_in_grid]])
{
    current[index] = next[index];
}
"#;

    let mut builder = ModuleBuilder::new("hello_field")
        .target(Target::Metal)
        .value(ValueDraft::constant(
            "field.alpha",
            DataType::f32(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.2),
        ))
        .value(ValueDraft::constant(
            "field.decay",
            DataType::f32(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.001),
        ))
        .value(ValueDraft::constant(
            "field.maximum",
            DataType::f32(),
            Unit::DIMENSIONLESS,
            Literal::f32(16.0),
        ))
        .value(ValueDraft::constant(
            "field.width",
            DataType::u32(),
            Unit::DIMENSIONLESS,
            Literal::U32(WIDTH),
        ));
    for (name, data_type, protected) in [
        ("field.activator", DataType::f32(), true),
        ("field.activator_next", DataType::f32(), true),
        ("field.deposit_q16", DataType::u32(), false),
    ] {
        let mut stream = StreamDraft::new(name, data_type, Unit::DIMENSIONLESS)
            .capacity(CELL_COUNT)
            .length(CELL_COUNT)
            .initial_repeat(
                if name.ends_with("q16") {
                    Literal::U32(0)
                } else {
                    Literal::f32(0.0)
                },
                CELL_COUNT,
            );
        if protected {
            stream = stream.write_authority("mutate_field_state");
        }
        builder = builder.stream(stream);
    }
    builder
        .kernel(
            KernelDraft::new("clear_deposit")
                .slot(SlotDraft::stream(
                    "deposit",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Write,
                ))
                .abi(KernelAbiDraft::new(["deposit"]))
                .implementation(packaged_metal_implementation(
                    "pqo://specimens/hello_field.metal",
                    "clear_deposit",
                    SOURCE,
                )),
        )
        .kernel(
            KernelDraft::new("seed_deposit")
                .slot(
                    SlotDraft::stream(
                        "deposit",
                        DataType::u32(),
                        Unit::DIMENSIONLESS,
                        SlotAccess::Write,
                    )
                    .whole_resource(),
                )
                .slot(SlotDraft::value(
                    "width",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                ))
                .abi(KernelAbiDraft::new(["deposit", "width"]))
                .implementation(packaged_metal_implementation(
                    "pqo://specimens/hello_field.metal",
                    "seed_deposit",
                    SOURCE,
                )),
        )
        .kernel(
            KernelDraft::new("diffuse_reflective")
                .slot(SlotDraft::stream(
                    "current",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Read,
                ))
                .slot(SlotDraft::stream(
                    "deposit",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Read,
                ))
                .slot(SlotDraft::stream(
                    "next",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Write,
                ))
                .slot(SlotDraft::value(
                    "alpha",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                ))
                .slot(SlotDraft::value(
                    "decay",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                ))
                .slot(SlotDraft::value(
                    "maximum",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                ))
                .slot(SlotDraft::value(
                    "width",
                    DataType::u32(),
                    Unit::DIMENSIONLESS,
                ))
                .abi(KernelAbiDraft::new([
                    "current", "deposit", "next", "alpha", "decay", "maximum", "width",
                ]))
                .implementation(packaged_metal_implementation(
                    "pqo://specimens/hello_field.metal",
                    "diffuse_reflective",
                    SOURCE,
                )),
        )
        .kernel(
            KernelDraft::new("commit_field")
                .slot(SlotDraft::stream(
                    "current",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Write,
                ))
                .slot(SlotDraft::stream(
                    "next",
                    DataType::f32(),
                    Unit::DIMENSIONLESS,
                    SlotAccess::Read,
                ))
                .abi(KernelAbiDraft::new(["current", "next"]))
                .implementation(packaged_metal_implementation(
                    "pqo://specimens/hello_field.metal",
                    "commit_field",
                    SOURCE,
                )),
        )
        .pass(
            PassDraft::new("clear_deposits", "clear_deposit")
                .bind("deposit", "field.deposit_q16")
                .dispatch_over("field.deposit_q16"),
        )
        .pass(
            PassDraft::new("seed", "seed_deposit")
                .bind("deposit", "field.deposit_q16")
                .bind("width", "field.width"),
        )
        .pass(
            PassDraft::new("diffuse", "diffuse_reflective")
                .bind("current", "field.activator")
                .bind("deposit", "field.deposit_q16")
                .bind("next", "field.activator_next")
                .bind("alpha", "field.alpha")
                .bind("decay", "field.decay")
                .bind("maximum", "field.maximum")
                .bind("width", "field.width")
                .dispatch_over("field.activator")
                .grant("mutate_field_state"),
        )
        .pass(
            PassDraft::new("commit", "commit_field")
                .bind("current", "field.activator")
                .bind("next", "field.activator_next")
                .dispatch_over("field.activator")
                .grant("mutate_field_state"),
        )
        .schedule(
            ScheduleDraft::fixed("simulation", 120)
                .run("clear_deposits")
                .run_after("seed", "clear_deposits")
                .run_after("diffuse", "seed")
                .run_after("commit", "diffuse")
                .tick_overlap(TickOverlapPolicy::QueueOrderedReuse)
                .presentation_lifetime(PresentationLifetimePolicy::QueueOrderedReuse)
                .queue_model(QueueModel::SingleSerialQueue),
        )
        .contract(ContractDraft::new("field_finite", "simulation").clause(
            ContractClauseDraft::Invariant {
                observation: ObservationDraft::AfterTickExecution("simulation".to_owned()),
                predicate: PredicateDraft::FiniteStreams(vec![
                    "field.activator".to_owned(),
                    "field.activator_next".to_owned(),
                ]),
            },
        ))
        .capability(CapabilityDraft::state_mutate(
            "mutate_field_state",
            ["field.activator", "field.activator_next"],
        ))
}
