//! Hello Crystal: 3D mesoscopic growth, cleavage, and fragmentation.

use crate::*;

const SOURCE: &str = include_str!("../../../kernels/crystal.metal");
const RENDER_SOURCE: &str = include_str!("../../../shaders/crystal.metal");
const METRIC_COUNT: u32 = 16;
const RELAXATION_ROUNDS: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelloCrystalConfig {
    pub cell_count: u32,
}

impl HelloCrystalConfig {
    pub const fn reference(cell_count: u32) -> Self {
        Self { cell_count }
    }
}

fn cube_axis(count: u32) -> u32 {
    assert!(count > 0, "Hello Crystal requires at least one cell");
    let axis = (count as f64).cbrt().round() as u32;
    assert_eq!(
        axis.checked_mul(axis).and_then(|n| n.checked_mul(axis)),
        Some(count),
        "Hello Crystal cell count must be a perfect cube (for example 1m = 100^3)"
    );
    assert!(
        (8..=128).contains(&axis),
        "Hello Crystal supports axes from 8 through 128 cells"
    );
    axis
}

fn ss(name: &str, ty: DataType, access: SlotAccess, whole: bool) -> SlotDraft {
    let slot = SlotDraft::stream(name, ty, Unit::DIMENSIONLESS, access);
    if whole { slot.whole_resource() } else { slot }
}

fn sr(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::Read, false)
}

fn srw(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::ReadWrite, false)
}

fn sw(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::Write, false)
}

fn sr_all(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::Read, true)
}

fn srw_all(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::ReadWrite, true)
}

fn value(name: &str, ty: DataType) -> SlotDraft {
    SlotDraft::value(name, ty, Unit::DIMENSIONLESS)
}

fn kernel(name: &str, slots: Vec<SlotDraft>) -> KernelDraft {
    let order = slots
        .iter()
        .map(|slot| slot.name.clone())
        .collect::<Vec<_>>();
    slots.into_iter().fold(
        KernelDraft::new(name)
            .abi(KernelAbiDraft::new(order))
            .implementation(packaged_metal_implementation(
                "kernels/crystal.metal",
                name,
                SOURCE,
            )),
        |draft, slot| draft.slot(slot),
    )
}

fn state_stream(name: &str, ty: DataType, initial: Literal, count: u32) -> StreamDraft {
    StreamDraft::new(name, ty, Unit::DIMENSIONLESS)
        .capacity(count)
        .length(count)
        .write_authority("mutate_crystal_state")
        .initial_repeat(initial, count)
}

pub fn hello_crystal_builder(cell_count: u32) -> ModuleBuilder {
    hello_crystal_builder_with_config(HelloCrystalConfig::reference(cell_count))
}

/// Builds the dense hybrid particle/field specimen described by Hello Crystal.
///
/// A cell represents a mesoscopic material volume, not an atom. Solute and heat
/// diffuse through the environment; a cubic orientation law advances the phase
/// boundary. Damage is absent from autonomous ticks: a mouse drag enters as an
/// explicit slice intervention, after which connected solid is relabeled and
/// detached material receives independent motion.
pub fn hello_crystal_builder_with_config(config: HelloCrystalConfig) -> ModuleBuilder {
    let width = cube_axis(config.cell_count);
    let count = config.cell_count;
    let seed_index = ((width / 2) * width + width / 2) * width + width / 2;
    let f32t = DataType::f32();
    let u32t = DataType::u32();
    let v3t = DataType::vec3_f32();
    let v4t = DataType::vec4_f32();
    let zero3 = || {
        Literal::Vector(vec![
            Literal::f32(0.0),
            Literal::f32(0.0),
            Literal::f32(0.0),
        ])
    };

    let mut builder = ModuleBuilder::new("hello_crystal")
        .target(Target::Metal)
        .value(ValueDraft::constant(
            "field.width",
            u32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(width),
        ))
        .value(ValueDraft::constant(
            "crystal.seed_index",
            u32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::U32(seed_index),
        ))
        .value(ValueDraft::constant(
            "growth.rate",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.19),
        ))
        .value(ValueDraft::constant(
            "growth.anisotropy_strength",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.68),
        ))
        .value(ValueDraft::constant(
            "field.solute_diffusion",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.075),
        ))
        .value(ValueDraft::constant(
            "field.thermal_diffusion",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.065),
        ))
        .value(ValueDraft::constant(
            "interaction.slice_start_x",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.slice_start_y",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.slice_end_x",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.slice_end_y",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.slice_radius",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.024),
        ))
        .stream(
            StreamDraft::new("simulation.tick", u32t.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .write_authority("mutate_crystal_state")
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );

    for name in [
        "field.phase",
        "field.phase_next",
        "field.solute",
        "field.solute_next",
        "field.temperature",
        "field.temperature_next",
        "material.damage",
        "render.radius",
    ] {
        builder = builder.stream(state_stream(name, f32t.clone(), Literal::f32(0.0), count));
    }
    builder = builder
        .stream(state_stream(
            "material.component",
            u32t.clone(),
            Literal::U32(u32::MAX),
            count,
        ))
        .stream(state_stream(
            "material.position",
            v3t.clone(),
            zero3(),
            count,
        ))
        .stream(state_stream(
            "material.velocity",
            v3t.clone(),
            zero3(),
            count,
        ))
        .stream(state_stream("render.position", v3t.clone(), zero3(), count))
        .stream(state_stream("render.normal", v3t.clone(), zero3(), count))
        .stream(state_stream(
            "render.color",
            v4t.clone(),
            Literal::Vector(vec![
                Literal::f32(0.0),
                Literal::f32(0.0),
                Literal::f32(0.0),
                Literal::f32(0.0),
            ]),
            count,
        ))
        .stream(state_stream(
            "metrics.snapshot",
            u32t.clone(),
            Literal::U32(0),
            METRIC_COUNT,
        ))
        .stream(
            StreamDraft::new("interaction.slice_count", u32t.clone(), Unit::DIMENSIONLESS)
                .capacity(1)
                .length(1)
                .write_authority("mutate_crystal_state")
                .initial(Literal::Array(vec![Literal::U32(0)])),
        );

    builder = builder
        .kernel(kernel(
            "crystal_initialize",
            vec![
                sw("phase", f32t.clone()),
                sw("phase_next", f32t.clone()),
                sw("solute", f32t.clone()),
                sw("solute_next", f32t.clone()),
                sw("temperature", f32t.clone()),
                sw("temperature_next", f32t.clone()),
                sw("damage", f32t.clone()),
                sw("component", u32t.clone()),
                sw("position", v3t.clone()),
                sw("velocity", v3t.clone()),
                sr_all("tick", u32t.clone()),
                value("width", u32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_evolve_fields",
            vec![
                sr_all("phase", f32t.clone()),
                sr_all("solute", f32t.clone()),
                sr_all("temperature", f32t.clone()),
                sr_all("damage", f32t.clone()),
                sw("phase_next", f32t.clone()),
                sw("solute_next", f32t.clone()),
                sw("temperature_next", f32t.clone()),
                sr_all("tick", u32t.clone()),
                value("width", u32t.clone()),
                value("growth_rate", f32t.clone()),
                value("anisotropy_strength", f32t.clone()),
                value("solute_diffusion", f32t.clone()),
                value("thermal_diffusion", f32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_commit_fields",
            vec![
                sw("phase", f32t.clone()),
                sr("phase_next", f32t.clone()),
                sw("solute", f32t.clone()),
                sr("solute_next", f32t.clone()),
                sw("temperature", f32t.clone()),
                sr("temperature_next", f32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_slice_material",
            vec![
                sr("phase", f32t.clone()),
                sr("position", v3t.clone()),
                srw("damage", f32t.clone()),
                srw("velocity", v3t.clone()),
                srw_all("slice_count", u32t.clone()),
                value("start_x", f32t.clone()),
                value("start_y", f32t.clone()),
                value("end_x", f32t.clone()),
                value("end_y", f32t.clone()),
                value("radius", f32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_initialize_components",
            vec![
                sr("phase", f32t.clone()),
                sr("damage", f32t.clone()),
                srw("component", u32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_relax_components",
            vec![
                sr_all("phase", f32t.clone()),
                sr_all("damage", f32t.clone()),
                srw_all("component", u32t.clone()),
                value("width", u32t.clone()),
            ],
        ))
        .kernel({
            let mut fixed_dt = SlotDraft::value("fixed_dt", f32t.clone(), Unit::SECOND);
            fixed_dt.name = "fixed_dt".to_owned();
            kernel(
                "crystal_integrate_fragments",
                vec![
                    sr("phase", f32t.clone()),
                    sr("damage", f32t.clone()),
                    sr_all("component", u32t.clone()),
                    srw("position", v3t.clone()),
                    srw("velocity", v3t.clone()),
                    sr_all("slice_count", u32t.clone()),
                    value("seed_index", u32t.clone()),
                    fixed_dt,
                ],
            )
        })
        .kernel(kernel(
            "crystal_prepare_render",
            vec![
                sr_all("phase", f32t.clone()),
                sr_all("damage", f32t.clone()),
                sr_all("component", u32t.clone()),
                sr("position", v3t.clone()),
                sw("render_position", v3t.clone()),
                sw("render_normal", v3t.clone()),
                sw("color", v4t.clone()),
                sw("radius", f32t.clone()),
                value("width", u32t.clone()),
                value("seed_index", u32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_clear_metrics",
            vec![sw("metrics", u32t.clone())],
        ))
        .kernel(kernel(
            "crystal_reduce_metrics",
            vec![
                sr("phase", f32t.clone()),
                sr("solute", f32t.clone()),
                sr("temperature", f32t.clone()),
                sr("damage", f32t.clone()),
                sr_all("component", u32t.clone()),
                sr("radius", f32t.clone()),
                sr_all("slice_count", u32t.clone()),
                srw_all("metrics", u32t.clone()),
                value("seed_index", u32t.clone()),
            ],
        ))
        .kernel(kernel(
            "crystal_advance_tick",
            vec![srw("tick", u32t.clone())],
        ));

    builder = builder
        .pass(
            PassDraft::new("initialize_crystal", "crystal_initialize")
                .bind("phase", "field.phase")
                .bind("phase_next", "field.phase_next")
                .bind("solute", "field.solute")
                .bind("solute_next", "field.solute_next")
                .bind("temperature", "field.temperature")
                .bind("temperature_next", "field.temperature_next")
                .bind("damage", "material.damage")
                .bind("component", "material.component")
                .bind("position", "material.position")
                .bind("velocity", "material.velocity")
                .bind("tick", "simulation.tick")
                .bind("width", "field.width")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("evolve_growth_fields", "crystal_evolve_fields")
                .bind("phase", "field.phase")
                .bind("solute", "field.solute")
                .bind("temperature", "field.temperature")
                .bind("damage", "material.damage")
                .bind("phase_next", "field.phase_next")
                .bind("solute_next", "field.solute_next")
                .bind("temperature_next", "field.temperature_next")
                .bind("tick", "simulation.tick")
                .bind("width", "field.width")
                .bind("growth_rate", "growth.rate")
                .bind("anisotropy_strength", "growth.anisotropy_strength")
                .bind("solute_diffusion", "field.solute_diffusion")
                .bind("thermal_diffusion", "field.thermal_diffusion")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("commit_growth_fields", "crystal_commit_fields")
                .bind("phase", "field.phase")
                .bind("phase_next", "field.phase_next")
                .bind("solute", "field.solute")
                .bind("solute_next", "field.solute_next")
                .bind("temperature", "field.temperature")
                .bind("temperature_next", "field.temperature_next")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("slice_material", "crystal_slice_material")
                .bind("phase", "field.phase")
                .bind("position", "material.position")
                .bind("damage", "material.damage")
                .bind("velocity", "material.velocity")
                .bind("slice_count", "interaction.slice_count")
                .bind("start_x", "interaction.slice_start_x")
                .bind("start_y", "interaction.slice_start_y")
                .bind("end_x", "interaction.slice_end_x")
                .bind("end_y", "interaction.slice_end_y")
                .bind("radius", "interaction.slice_radius")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new(
                "initialize_solid_components",
                "crystal_initialize_components",
            )
            .bind("phase", "field.phase")
            .bind("damage", "material.damage")
            .bind("component", "material.component")
            .dispatch_over("field.phase")
            .grant("mutate_crystal_state"),
        );

    for round in 0..RELAXATION_ROUNDS {
        builder = builder.pass(
            PassDraft::new(
                format!("relax_solid_components_{round}"),
                "crystal_relax_components",
            )
            .bind("phase", "field.phase")
            .bind("damage", "material.damage")
            .bind("component", "material.component")
            .bind("width", "field.width")
            .dispatch_over("field.phase")
            .grant("mutate_crystal_state"),
        );
    }

    builder = builder
        .pass(
            PassDraft::new("integrate_fragments", "crystal_integrate_fragments")
                .bind("phase", "field.phase")
                .bind("damage", "material.damage")
                .bind("component", "material.component")
                .bind("position", "material.position")
                .bind("velocity", "material.velocity")
                .bind("slice_count", "interaction.slice_count")
                .bind("seed_index", "crystal.seed_index")
                .bind("fixed_dt", "simulation.fixed_dt")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("extract_crystal_surface", "crystal_prepare_render")
                .bind("phase", "field.phase")
                .bind("damage", "material.damage")
                .bind("component", "material.component")
                .bind("position", "material.position")
                .bind("render_position", "render.position")
                .bind("render_normal", "render.normal")
                .bind("color", "render.color")
                .bind("radius", "render.radius")
                .bind("width", "field.width")
                .bind("seed_index", "crystal.seed_index")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("clear_crystal_metrics", "crystal_clear_metrics")
                .bind("metrics", "metrics.snapshot")
                .dispatch_over("metrics.snapshot")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("reduce_crystal_metrics", "crystal_reduce_metrics")
                .bind("phase", "field.phase")
                .bind("solute", "field.solute")
                .bind("temperature", "field.temperature")
                .bind("damage", "material.damage")
                .bind("component", "material.component")
                .bind("radius", "render.radius")
                .bind("slice_count", "interaction.slice_count")
                .bind("metrics", "metrics.snapshot")
                .bind("seed_index", "crystal.seed_index")
                .dispatch_over("field.phase")
                .grant("mutate_crystal_state"),
        )
        .pass(
            PassDraft::new("advance_crystal_tick", "crystal_advance_tick")
                .bind("tick", "simulation.tick")
                .grant("mutate_crystal_state"),
        );

    let mut schedule = ScheduleDraft::fixed("simulation", 30)
        .run("initialize_crystal")
        .run_after("evolve_growth_fields", "initialize_crystal")
        .run_after("commit_growth_fields", "evolve_growth_fields")
        .run_after("initialize_solid_components", "commit_growth_fields");
    let mut predecessor = "initialize_solid_components".to_owned();
    for round in 0..RELAXATION_ROUNDS {
        let pass = format!("relax_solid_components_{round}");
        schedule = schedule.run_after(pass.clone(), predecessor);
        predecessor = pass;
    }
    schedule = schedule
        .run_after("integrate_fragments", predecessor)
        .run_after("extract_crystal_surface", "integrate_fragments")
        .run_after("clear_crystal_metrics", "extract_crystal_surface")
        .run_after("reduce_crystal_metrics", "clear_crystal_metrics")
        .run_after("advance_crystal_tick", "reduce_crystal_metrics")
        .show_after("crystal", "extract_crystal_surface")
        .tick_overlap(TickOverlapPolicy::QueueOrderedReuse)
        .presentation_lifetime(PresentationLifetimePolicy::QueueOrderedReuse)
        .queue_model(QueueModel::SingleSerialQueue);

    builder
        .view(
            ViewDraft::render(
                "crystal",
                packaged_metal_implementation(
                    "shaders/crystal.metal",
                    "crystal_pipeline",
                    RENDER_SOURCE,
                ),
            )
            .read("color", "render.color")
            .read("position", "render.position")
            .read("radius", "render.radius")
            .read("z_normal", "render.normal"),
        )
        .schedule(schedule)
        .contract(ContractDraft::new("crystal_finite", "simulation").clause(
            ContractClauseDraft::Invariant {
                observation: ObservationDraft::AfterTickExecution("simulation".to_owned()),
                predicate: PredicateDraft::FiniteStreams(vec![
                    "field.phase".to_owned(),
                    "field.solute".to_owned(),
                    "field.temperature".to_owned(),
                    "material.damage".to_owned(),
                    "material.position".to_owned(),
                ]),
            },
        ))
        .scenario(
            ScenarioDraft::new("growth_without_damage", "simulation", 600).expect(
                ObservationDraft::AfterTickExecution("simulation".to_owned()),
                PredicateDraft::FiniteStreams(vec![
                    "field.phase".to_owned(),
                    "material.damage".to_owned(),
                    "material.position".to_owned(),
                ]),
            ),
        )
        .scenario(
            ScenarioDraft::new("slice_interaction_probe", "simulation", 180)
                .intervene(
                    100,
                    "slice_material",
                    [
                        ("interaction.slice_start_x", Literal::f32(-0.65)),
                        ("interaction.slice_start_y", Literal::f32(0.0)),
                        ("interaction.slice_end_x", Literal::f32(0.65)),
                        ("interaction.slice_end_y", Literal::f32(0.0)),
                        ("interaction.slice_radius", Literal::f32(0.024)),
                    ],
                )
                .expect(
                    ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    PredicateDraft::FiniteStreams(vec![
                        "field.phase".to_owned(),
                        "material.damage".to_owned(),
                        "material.position".to_owned(),
                    ]),
                ),
        )
        .capability(CapabilityDraft::state_mutate(
            "mutate_crystal_state",
            [
                "simulation.tick",
                "field.phase",
                "field.phase_next",
                "field.solute",
                "field.solute_next",
                "field.temperature",
                "field.temperature_next",
                "material.damage",
                "material.component",
                "material.position",
                "material.velocity",
                "interaction.slice_count",
                "render.position",
                "render.normal",
                "render.color",
                "render.radius",
                "metrics.snapshot",
            ],
        ))
}
