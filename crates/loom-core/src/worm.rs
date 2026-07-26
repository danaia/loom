//! Hello Worm: an interactive, food-seeking Loom species on a 3D plane.

use crate::*;

const SOURCE: &str = include_str!("../../../kernels/worm.metal");
const RENDER_SOURCE: &str = include_str!("../../../shaders/worm.metal");
pub const WORM_SEGMENTS: u32 = 24;
pub const FOOD_CAPACITY: u32 = 12;
pub const WORM_RENDER_INSTANCES: u32 = 1 + FOOD_CAPACITY + WORM_SEGMENTS;

fn ss(name: &str, ty: DataType, access: SlotAccess, whole: bool) -> SlotDraft {
    let slot = SlotDraft::stream(name, ty, Unit::DIMENSIONLESS, access);
    if whole { slot.whole_resource() } else { slot }
}

fn sr_all(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::Read, true)
}

fn srw_all(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::ReadWrite, true)
}

fn sw(name: &str, ty: DataType) -> SlotDraft {
    ss(name, ty, SlotAccess::Write, false)
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
                "kernels/worm.metal",
                name,
                SOURCE,
            )),
        |draft, slot| draft.slot(slot),
    )
}

fn scalar_stream(name: &str, ty: DataType, initial: Literal) -> StreamDraft {
    StreamDraft::new(name, ty, Unit::DIMENSIONLESS)
        .capacity(1)
        .length(1)
        .write_authority("mutate_worm_world")
        .initial(Literal::Array(vec![initial]))
}

fn repeated_stream(name: &str, ty: DataType, initial: Literal, count: u32) -> StreamDraft {
    StreamDraft::new(name, ty, Unit::DIMENSIONLESS)
        .capacity(count)
        .length(count)
        .write_authority("mutate_worm_world")
        .initial_repeat(initial, count)
}

fn vec3(x: f32, y: f32, z: f32) -> Literal {
    Literal::Vector(vec![Literal::f32(x), Literal::f32(y), Literal::f32(z)])
}

/// Builds an autonomous Loom worm with persistent perception, steering, body
/// articulation, food consumption, and explicit pointer/camera interventions.
pub fn hello_worm_builder() -> ModuleBuilder {
    let f32t = DataType::f32();
    let u32t = DataType::u32();
    let v3t = DataType::vec3_f32();
    let v4t = DataType::vec4_f32();

    let initial_body = (0..WORM_SEGMENTS)
        .map(|index| vec3(0.42 - index as f32 * 0.052, 0.058, 0.04))
        .collect::<Vec<_>>();

    let mut builder = ModuleBuilder::new("hello_worm")
        .target(Target::Metal)
        .value(ValueDraft::constant(
            "interaction.drop_x",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.drop_y",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.orbit_delta_yaw",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.orbit_delta_pitch",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .value(ValueDraft::constant(
            "interaction.zoom_delta",
            f32t.clone(),
            Unit::DIMENSIONLESS,
            Literal::f32(0.0),
        ))
        .stream(
            StreamDraft::new("worm.position", v3t.clone(), Unit::DIMENSIONLESS)
                .capacity(WORM_SEGMENTS)
                .length(WORM_SEGMENTS)
                .write_authority("mutate_worm_world")
                .initial(Literal::Array(initial_body)),
        )
        .stream(scalar_stream(
            "worm.heading",
            v3t.clone(),
            vec3(1.0, 0.0, 0.0),
        ))
        .stream(scalar_stream(
            "worm.smell_strength",
            f32t.clone(),
            Literal::f32(0.0),
        ))
        .stream(scalar_stream("worm.meals", u32t.clone(), Literal::U32(0)))
        .stream(repeated_stream(
            "food.position",
            v3t.clone(),
            vec3(0.0, 0.047, 0.0),
            FOOD_CAPACITY,
        ))
        .stream(repeated_stream(
            "food.active",
            u32t.clone(),
            Literal::U32(0),
            FOOD_CAPACITY,
        ))
        .stream(scalar_stream(
            "simulation.tick",
            u32t.clone(),
            Literal::U32(0),
        ))
        .stream(scalar_stream(
            "interaction.drop_cursor",
            u32t.clone(),
            Literal::U32(0),
        ))
        .stream(scalar_stream(
            "interaction.camera_yaw",
            f32t.clone(),
            Literal::f32(-0.48),
        ))
        .stream(scalar_stream(
            "interaction.camera_pitch",
            f32t.clone(),
            Literal::f32(0.68),
        ))
        .stream(scalar_stream(
            "interaction.camera_zoom",
            f32t.clone(),
            Literal::f32(0.86),
        ))
        .stream(repeated_stream(
            "render.position",
            v3t.clone(),
            vec3(0.0, 0.0, 0.0),
            WORM_RENDER_INSTANCES,
        ))
        .stream(repeated_stream(
            "render.color",
            v4t.clone(),
            Literal::Vector(vec![
                Literal::f32(0.0),
                Literal::f32(0.0),
                Literal::f32(0.0),
                Literal::f32(0.0),
            ]),
            WORM_RENDER_INSTANCES,
        ))
        .stream(repeated_stream(
            "render.radius",
            f32t.clone(),
            Literal::f32(0.0),
            WORM_RENDER_INSTANCES,
        ))
        .stream(repeated_stream(
            "render.kind",
            u32t.clone(),
            Literal::U32(0),
            WORM_RENDER_INSTANCES,
        ));

    let mut fixed_dt = SlotDraft::value("fixed_dt", f32t.clone(), Unit::SECOND);
    fixed_dt.name = "fixed_dt".to_owned();
    builder = builder
        .kernel(kernel(
            "worm_think_and_move",
            vec![
                srw_all("position", v3t.clone()),
                srw_all("heading", v3t.clone()),
                srw_all("smell_strength", f32t.clone()),
                srw_all("meals", u32t.clone()),
                sr_all("food_position", v3t.clone()),
                srw_all("food_active", u32t.clone()),
                srw_all("tick", u32t.clone()),
                fixed_dt,
            ],
        ))
        .kernel(kernel(
            "worm_drop_food",
            vec![
                srw_all("food_position", v3t.clone()),
                srw_all("food_active", u32t.clone()),
                srw_all("drop_cursor", u32t.clone()),
                sr_all("camera_yaw", f32t.clone()),
                sr_all("camera_pitch", f32t.clone()),
                sr_all("camera_zoom", f32t.clone()),
                value("pick_x", f32t.clone()),
                value("pick_y", f32t.clone()),
            ],
        ))
        .kernel(kernel(
            "worm_orbit_camera",
            vec![
                srw_all("camera_yaw", f32t.clone()),
                srw_all("camera_pitch", f32t.clone()),
                value("delta_yaw", f32t.clone()),
                value("delta_pitch", f32t.clone()),
            ],
        ))
        .kernel(kernel(
            "worm_zoom_camera",
            vec![
                srw_all("camera_zoom", f32t.clone()),
                value("zoom_delta", f32t.clone()),
            ],
        ))
        .kernel(kernel(
            "worm_prepare_render",
            vec![
                sr_all("worm_position", v3t.clone()),
                sr_all("smell_strength", f32t.clone()),
                sr_all("meals", u32t.clone()),
                sr_all("food_position", v3t.clone()),
                sr_all("food_active", u32t.clone()),
                sr_all("camera_yaw", f32t.clone()),
                sr_all("camera_pitch", f32t.clone()),
                sr_all("camera_zoom", f32t.clone()),
                sw("render_position", v3t.clone()),
                sw("render_color", v4t.clone()),
                sw("render_radius", f32t.clone()),
                sw("render_kind", u32t.clone()),
            ],
        ));

    builder = builder
        .pass(
            PassDraft::new("think_and_move", "worm_think_and_move")
                .bind("position", "worm.position")
                .bind("heading", "worm.heading")
                .bind("smell_strength", "worm.smell_strength")
                .bind("meals", "worm.meals")
                .bind("food_position", "food.position")
                .bind("food_active", "food.active")
                .bind("tick", "simulation.tick")
                .bind("fixed_dt", "simulation.fixed_dt")
                .grant("mutate_worm_world"),
        )
        .pass(
            PassDraft::new("drop_food", "worm_drop_food")
                .bind("food_position", "food.position")
                .bind("food_active", "food.active")
                .bind("drop_cursor", "interaction.drop_cursor")
                .bind("camera_yaw", "interaction.camera_yaw")
                .bind("camera_pitch", "interaction.camera_pitch")
                .bind("camera_zoom", "interaction.camera_zoom")
                .bind("pick_x", "interaction.drop_x")
                .bind("pick_y", "interaction.drop_y")
                .grant("mutate_worm_world"),
        )
        .pass(
            PassDraft::new("orbit_camera", "worm_orbit_camera")
                .bind("camera_yaw", "interaction.camera_yaw")
                .bind("camera_pitch", "interaction.camera_pitch")
                .bind("delta_yaw", "interaction.orbit_delta_yaw")
                .bind("delta_pitch", "interaction.orbit_delta_pitch")
                .grant("mutate_worm_world"),
        )
        .pass(
            PassDraft::new("zoom_camera", "worm_zoom_camera")
                .bind("camera_zoom", "interaction.camera_zoom")
                .bind("zoom_delta", "interaction.zoom_delta")
                .grant("mutate_worm_world"),
        )
        .pass(
            PassDraft::new("prepare_worm_scene", "worm_prepare_render")
                .bind("worm_position", "worm.position")
                .bind("smell_strength", "worm.smell_strength")
                .bind("meals", "worm.meals")
                .bind("food_position", "food.position")
                .bind("food_active", "food.active")
                .bind("camera_yaw", "interaction.camera_yaw")
                .bind("camera_pitch", "interaction.camera_pitch")
                .bind("camera_zoom", "interaction.camera_zoom")
                .bind("render_position", "render.position")
                .bind("render_color", "render.color")
                .bind("render_radius", "render.radius")
                .bind("render_kind", "render.kind")
                .dispatch_over("render.position")
                .grant("mutate_worm_world"),
        );

    builder
        .view(
            ViewDraft::render(
                "worm_world",
                packaged_metal_implementation("shaders/worm.metal", "worm_pipeline", RENDER_SOURCE),
            )
            .read("color", "render.color")
            .read("kind", "render.kind")
            .read("position", "render.position")
            .read("radius", "render.radius"),
        )
        .schedule(
            ScheduleDraft::fixed("simulation", 60)
                .run("think_and_move")
                .run_after("prepare_worm_scene", "think_and_move")
                .show_after("worm_world", "prepare_worm_scene")
                .tick_overlap(TickOverlapPolicy::QueueOrderedReuse)
                .presentation_lifetime(PresentationLifetimePolicy::QueueOrderedReuse)
                .queue_model(QueueModel::SingleSerialQueue),
        )
        .contract(
            ContractDraft::new("worm_world_finite", "simulation").clause(
                ContractClauseDraft::Invariant {
                    observation: ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    predicate: PredicateDraft::FiniteStreams(vec![
                        "worm.position".to_owned(),
                        "worm.heading".to_owned(),
                        "food.position".to_owned(),
                    ]),
                },
            ),
        )
        .scenario(
            ScenarioDraft::new("worm_smells_and_eats", "simulation", 600)
                .intervene(
                    10,
                    "drop_food",
                    [
                        ("interaction.drop_x", Literal::f32(0.35)),
                        ("interaction.drop_y", Literal::f32(0.05)),
                    ],
                )
                .expect(
                    ObservationDraft::AfterTickExecution("simulation".to_owned()),
                    PredicateDraft::FiniteStreams(vec![
                        "worm.position".to_owned(),
                        "worm.smell_strength".to_owned(),
                    ]),
                ),
        )
        .scenario(
            ScenarioDraft::new("worm_camera_probe", "simulation", 120)
                .intervene(
                    20,
                    "orbit_camera",
                    [
                        ("interaction.orbit_delta_yaw", Literal::f32(0.25)),
                        ("interaction.orbit_delta_pitch", Literal::f32(-0.12)),
                    ],
                )
                .intervene(
                    30,
                    "zoom_camera",
                    [("interaction.zoom_delta", Literal::f32(0.1))],
                ),
        )
        .capability(CapabilityDraft::state_mutate(
            "mutate_worm_world",
            [
                "worm.position",
                "worm.heading",
                "worm.smell_strength",
                "worm.meals",
                "food.position",
                "food.active",
                "simulation.tick",
                "interaction.drop_cursor",
                "interaction.camera_yaw",
                "interaction.camera_pitch",
                "interaction.camera_zoom",
                "render.position",
                "render.color",
                "render.radius",
                "render.kind",
            ],
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worm_scene_has_plane_food_and_body_instances() {
        let graph = hello_worm_builder().build().expect("worm graph");
        let render = graph
            .resources
            .streams
            .iter()
            .find(|stream| stream.name == "render.position")
            .expect("render positions");
        assert_eq!(render.capacity, WORM_RENDER_INSTANCES);
        assert!(graph.passes.iter().any(|pass| pass.name == "drop_food"));
        assert!(
            graph
                .passes
                .iter()
                .any(|pass| pass.name == "think_and_move")
        );
    }
}
