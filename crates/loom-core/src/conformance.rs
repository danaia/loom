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

    let positions = (0..particle_count)
        .map(|index| {
            let columns = (particle_count as f32).sqrt().ceil().max(1.0) as u32;
            let column = index % columns;
            let row = index / columns;
            let x = if columns == 1 {
                0.0
            } else {
                -0.9 + 1.8 * column as f32 / (columns - 1) as f32
            };
            let y = 0.15 + (row % 24) as f32 * 0.035;
            Literal::Vector(vec![Literal::f32(x), Literal::f32(y), Literal::f32(0.0)])
        })
        .collect::<Vec<_>>();
    let repeated_vec3 = |x: f32, y: f32, z: f32| {
        Literal::Array(
            (0..particle_count)
                .map(|_| Literal::Vector(vec![Literal::f32(x), Literal::f32(y), Literal::f32(z)]))
                .collect(),
        )
    };
    let repeated_f32 =
        |value: f32| Literal::Array((0..particle_count).map(|_| Literal::f32(value)).collect());
    let colors = Literal::Array(
        (0..particle_count)
            .map(|index| {
                let phase = index as f32 / particle_count.max(1) as f32;
                Literal::Vector(vec![
                    Literal::f32(0.4 + 0.6 * phase),
                    Literal::f32(0.2 + 0.5 * (1.0 - phase)),
                    Literal::f32(1.0 - 0.5 * phase),
                    Literal::f32(1.0),
                ])
            })
            .collect(),
    );

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
                .initial(Literal::Array(positions)),
        )
        .stream(
            StreamDraft::new("particles.velocity", vec3.clone(), Unit::METERS_PER_SECOND)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .initial(repeated_vec3(0.0, 0.0, 0.0)),
        )
        .stream(
            StreamDraft::new("particles.radius", f32_type.clone(), Unit::METER)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .access(ResourceAccess::DeviceRead)
                .initial(repeated_f32(0.004)),
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
            .initial(repeated_f32(0.8)),
        )
        .stream(
            StreamDraft::new("particles.friction", f32_type.clone(), Unit::DIMENSIONLESS)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .access(ResourceAccess::DeviceRead)
                .initial(repeated_f32(0.3)),
        )
        .stream(
            StreamDraft::new("particles.color", vec4.clone(), Unit::DIMENSIONLESS)
                .capacity(particle_count)
                .length(particle_count)
                .buffering(config.particle_buffering)
                .access(ResourceAccess::DeviceRead)
                .initial(colors),
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
