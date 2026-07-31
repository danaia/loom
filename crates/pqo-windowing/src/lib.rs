use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const PROJECT_CONFIG_PATH: &str = "config/window-layout.json";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f64,
}

impl WindowFrame {
    pub fn position(self) -> WindowPosition {
        WindowPosition {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowSide {
    Origin,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowBoundaryConfig {
    pub enabled: bool,
    pub snap_on_open: bool,
    pub preferred_side: WindowSide,
}

impl Default for WindowBoundaryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            snap_on_open: true,
            preferred_side: WindowSide::Right,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowLayoutConfig {
    pub enabled: bool,
    pub snap_distance: f64,
    pub detach_distance: f64,
    pub gap: f64,
    pub link_movement: bool,
    pub viewer_width: f64,
    pub viewer_height: f64,
    pub agents_width: f64,
    pub agents_height: f64,
    pub viewer_panel: WindowBoundaryConfig,
    #[serde(alias = "panelAgents")]
    pub viewer_agents: WindowBoundaryConfig,
}

impl Default for WindowLayoutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            snap_distance: 16.0,
            detach_distance: 32.0,
            gap: 0.0,
            link_movement: true,
            viewer_width: 960.0,
            viewer_height: 720.0,
            agents_width: 680.0,
            agents_height: 760.0,
            viewer_panel: WindowBoundaryConfig::default(),
            viewer_agents: WindowBoundaryConfig::default(),
        }
    }
}

impl WindowLayoutConfig {
    pub fn load(project_root: &Path) -> Result<Self, String> {
        let path = project_root.join(PROJECT_CONFIG_PATH);
        if !path.is_file() {
            return Ok(Self::default());
        }
        let config = serde_json::from_slice::<Self>(
            &fs::read(&path)
                .map_err(|error| format!("could not read `{}`: {error}", path.display()))?,
        )
        .map_err(|error| format!("invalid `{}`: {error}", path.display()))?;
        config.validate(&path)?;
        Ok(config)
    }

    fn validate(&self, path: &Path) -> Result<(), String> {
        for (name, value) in [
            ("snapDistance", self.snap_distance),
            ("detachDistance", self.detach_distance),
            ("gap", self.gap),
        ] {
            if !value.is_finite() || !(0.0..=10_000.0).contains(&value) {
                return Err(format!(
                    "invalid `{}`: {name} must be between 0 and 10000",
                    path.display()
                ));
            }
        }
        if self.detach_distance < self.snap_distance {
            return Err(format!(
                "invalid `{}`: detachDistance must be at least snapDistance",
                path.display()
            ));
        }
        for (name, value) in [
            ("viewerWidth", self.viewer_width),
            ("viewerHeight", self.viewer_height),
            ("agentsWidth", self.agents_width),
            ("agentsHeight", self.agents_height),
        ] {
            if !value.is_finite() || !(200.0..=4096.0).contains(&value) {
                return Err(format!(
                    "invalid `{}`: {name} must be between 200 and 4096",
                    path.display()
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowLink {
    side: WindowSide,
    cross_offset: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapUpdate {
    None,
    Move(WindowPosition),
    Linked,
    Detached,
}

#[derive(Clone, Debug)]
pub struct SnapManager {
    layout: WindowLayoutConfig,
    boundary: WindowBoundaryConfig,
    link: Option<WindowLink>,
    opened: bool,
}

impl SnapManager {
    pub fn new(layout: WindowLayoutConfig, boundary: WindowBoundaryConfig) -> Self {
        Self {
            layout,
            boundary,
            link: None,
            opened: false,
        }
    }

    pub fn is_linked(&self) -> bool {
        self.link.is_some()
    }

    pub fn observe(&mut self, anchor: WindowFrame, moving: WindowFrame) -> SnapUpdate {
        if !self.layout.enabled || !self.boundary.enabled {
            self.link = None;
            return SnapUpdate::None;
        }

        if !self.opened {
            self.opened = true;
            if self.boundary.snap_on_open {
                return self.attach_on_open(anchor, moving, self.boundary.preferred_side);
            }
        }

        if let Some(mut link) = self.link {
            let expected = attached_position(anchor, moving, link, self.layout.gap);
            let perpendicular_error = match link.side {
                WindowSide::Origin => moving
                    .x
                    .abs_diff(expected.x)
                    .max(moving.y.abs_diff(expected.y)),
                WindowSide::Left | WindowSide::Right => moving.x.abs_diff(expected.x),
                WindowSide::Top | WindowSide::Bottom => moving.y.abs_diff(expected.y),
            };
            if f64::from(perpendicular_error) > scaled(self.layout.detach_distance, anchor, moving)
            {
                self.link = None;
                return SnapUpdate::Detached;
            }

            link.cross_offset = match link.side {
                WindowSide::Origin => link.cross_offset,
                WindowSide::Left | WindowSide::Right => moving.y - anchor.y,
                WindowSide::Top | WindowSide::Bottom => moving.x - anchor.x,
            };
            self.link = Some(link);
            return SnapUpdate::Linked;
        }

        let Some(side) = nearest_side(
            anchor,
            moving,
            scaled(self.layout.snap_distance, anchor, moving),
            scaled(self.layout.gap, anchor, moving).round() as i32,
        ) else {
            return SnapUpdate::None;
        };
        self.attach(anchor, moving, side)
    }

    pub fn follow(&self, anchor: WindowFrame, moving: WindowFrame) -> Option<WindowPosition> {
        if !self.layout.link_movement {
            return None;
        }
        self.link
            .map(|link| attached_position(anchor, moving, link, self.layout.gap))
    }

    fn attach(&mut self, anchor: WindowFrame, moving: WindowFrame, side: WindowSide) -> SnapUpdate {
        let link = WindowLink {
            side,
            cross_offset: match side {
                WindowSide::Origin => 0,
                WindowSide::Left | WindowSide::Right => moving.y - anchor.y,
                WindowSide::Top | WindowSide::Bottom => moving.x - anchor.x,
            },
        };
        self.link = Some(link);
        let position = attached_position(anchor, moving, link, self.layout.gap);
        if position == moving.position() {
            SnapUpdate::Linked
        } else {
            SnapUpdate::Move(position)
        }
    }

    fn attach_on_open(
        &mut self,
        anchor: WindowFrame,
        moving: WindowFrame,
        side: WindowSide,
    ) -> SnapUpdate {
        let link = WindowLink {
            side,
            cross_offset: 0,
        };
        self.link = Some(link);
        let position = attached_position(anchor, moving, link, self.layout.gap);
        if position == moving.position() {
            SnapUpdate::Linked
        } else {
            SnapUpdate::Move(position)
        }
    }
}

fn nearest_side(
    anchor: WindowFrame,
    moving: WindowFrame,
    distance: f64,
    gap: i32,
) -> Option<WindowSide> {
    let anchor_right = i64::from(anchor.x) + i64::from(anchor.width);
    let anchor_bottom = i64::from(anchor.y) + i64::from(anchor.height);
    let moving_right = i64::from(moving.x) + i64::from(moving.width);
    let moving_bottom = i64::from(moving.y) + i64::from(moving.height);
    let vertical_overlap =
        i64::from(moving.y) <= anchor_bottom && moving_bottom >= i64::from(anchor.y);
    let horizontal_overlap =
        i64::from(moving.x) <= anchor_right && moving_right >= i64::from(anchor.x);

    let candidates = [
        (
            WindowSide::Right,
            (i64::from(moving.x) - (anchor_right + i64::from(gap))).abs(),
            vertical_overlap,
        ),
        (
            WindowSide::Left,
            (moving_right - (i64::from(anchor.x) - i64::from(gap))).abs(),
            vertical_overlap,
        ),
        (
            WindowSide::Bottom,
            (i64::from(moving.y) - (anchor_bottom + i64::from(gap))).abs(),
            horizontal_overlap,
        ),
        (
            WindowSide::Top,
            (moving_bottom - (i64::from(anchor.y) - i64::from(gap))).abs(),
            horizontal_overlap,
        ),
    ];
    candidates
        .into_iter()
        .filter(|(_, candidate_distance, overlap)| {
            *overlap && *candidate_distance as f64 <= distance
        })
        .min_by_key(|(_, candidate_distance, _)| *candidate_distance)
        .map(|(side, _, _)| side)
}

fn attached_position(
    anchor: WindowFrame,
    moving: WindowFrame,
    link: WindowLink,
    logical_gap: f64,
) -> WindowPosition {
    let gap = scaled(logical_gap, anchor, moving).round() as i64;
    let x = i64::from(anchor.x);
    let y = i64::from(anchor.y);
    let width = i64::from(anchor.width);
    let height = i64::from(anchor.height);
    let moving_width = i64::from(moving.width);
    let moving_height = i64::from(moving.height);
    match link.side {
        WindowSide::Origin => WindowPosition {
            x: anchor.x,
            y: anchor.y,
        },
        WindowSide::Left => WindowPosition {
            x: clamp_i32(x - gap - moving_width),
            y: clamp_i32(y + i64::from(link.cross_offset)),
        },
        WindowSide::Right => WindowPosition {
            x: clamp_i32(x + width + gap),
            y: clamp_i32(y + i64::from(link.cross_offset)),
        },
        WindowSide::Top => WindowPosition {
            x: clamp_i32(x + i64::from(link.cross_offset)),
            y: clamp_i32(y - gap - moving_height),
        },
        WindowSide::Bottom => WindowPosition {
            x: clamp_i32(x + i64::from(link.cross_offset)),
            y: clamp_i32(y + height + gap),
        },
    }
}

fn scaled(value: f64, anchor: WindowFrame, moving: WindowFrame) -> f64 {
    value * anchor.scale_factor.max(moving.scale_factor).max(1.0)
}

fn clamp_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn default_scale_factor() -> f64 {
    1.0
}

pub fn project_config_path(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_CONFIG_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: i32, y: i32, width: u32, height: u32) -> WindowFrame {
        WindowFrame {
            x,
            y,
            width,
            height,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn snaps_and_links_to_nearest_edge() {
        let layout = WindowLayoutConfig {
            viewer_panel: WindowBoundaryConfig {
                snap_on_open: false,
                ..WindowBoundaryConfig::default()
            },
            ..WindowLayoutConfig::default()
        };
        let mut manager = SnapManager::new(layout.clone(), layout.viewer_panel.clone());
        let update = manager.observe(frame(100, 100, 500, 400), frame(610, 150, 200, 300));
        assert_eq!(update, SnapUpdate::Move(WindowPosition { x: 600, y: 150 }));
        assert!(manager.is_linked());
    }

    #[test]
    fn linked_window_follows_anchor_and_keeps_edge_offset() {
        let layout = WindowLayoutConfig::default();
        let mut manager = SnapManager::new(layout.clone(), layout.viewer_panel.clone());
        let moving = frame(800, 140, 200, 300);
        assert_eq!(
            manager.observe(frame(100, 100, 500, 400), moving),
            SnapUpdate::Move(WindowPosition { x: 600, y: 100 })
        );
        assert_eq!(
            manager.follow(frame(160, 120, 500, 400), moving),
            Some(WindowPosition { x: 660, y: 120 })
        );
    }

    #[test]
    fn origin_anchor_places_panel_at_viewer_zero_zero() {
        let layout = WindowLayoutConfig {
            viewer_panel: WindowBoundaryConfig {
                preferred_side: WindowSide::Origin,
                ..WindowBoundaryConfig::default()
            },
            ..WindowLayoutConfig::default()
        };
        let mut manager = SnapManager::new(layout.clone(), layout.viewer_panel.clone());
        let panel = frame(900, 300, 200, 300);
        assert_eq!(
            manager.observe(frame(100, 80, 500, 400), panel),
            SnapUpdate::Move(WindowPosition { x: 100, y: 80 })
        );
        assert_eq!(
            manager.follow(frame(180, 140, 500, 400), panel),
            Some(WindowPosition { x: 180, y: 140 })
        );
    }

    #[test]
    fn dragging_perpendicular_to_edge_detaches() {
        let layout = WindowLayoutConfig::default();
        let mut manager = SnapManager::new(layout.clone(), layout.viewer_panel.clone());
        let anchor = frame(100, 100, 500, 400);
        manager.observe(anchor, frame(600, 100, 200, 300));
        assert_eq!(
            manager.observe(anchor, frame(700, 100, 200, 300)),
            SnapUpdate::Detached
        );
        assert!(!manager.is_linked());
    }

    #[test]
    fn logical_distances_scale_for_retina_frames() {
        let layout = WindowLayoutConfig {
            viewer_panel: WindowBoundaryConfig {
                snap_on_open: false,
                ..WindowBoundaryConfig::default()
            },
            ..WindowLayoutConfig::default()
        };
        let mut manager = SnapManager::new(layout.clone(), layout.viewer_panel.clone());
        let mut anchor = frame(0, 0, 1000, 800);
        anchor.scale_factor = 2.0;
        let mut moving = frame(1030, 0, 400, 600);
        moving.scale_factor = 2.0;
        assert!(matches!(
            manager.observe(anchor, moving),
            SnapUpdate::Move(_)
        ));
    }

    #[test]
    fn loads_project_local_configuration() {
        let project = tempfile::tempdir().expect("temporary project");
        let path = project_config_path(project.path());
        fs::create_dir_all(path.parent().expect("config directory")).expect("config directory");
        fs::write(
            &path,
            r#"{
              "snapDistance": 24,
              "viewerWidth": 740,
              "agentsWidth": 620,
              "viewerPanel": {
                "snapOnOpen": false,
                "preferredSide": "left"
              },
              "viewerAgents": {
                "preferredSide": "right"
              }
            }"#,
        )
        .expect("window config");

        let config = WindowLayoutConfig::load(project.path()).expect("valid window config");
        assert_eq!(config.snap_distance, 24.0);
        assert_eq!(config.viewer_width, 740.0);
        assert_eq!(config.agents_width, 620.0);
        assert!(!config.viewer_panel.snap_on_open);
        assert_eq!(config.viewer_panel.preferred_side, WindowSide::Left);
        assert!(config.viewer_agents.enabled);
        assert_eq!(config.viewer_agents.preferred_side, WindowSide::Right);
    }

    #[test]
    fn rejects_detach_distance_smaller_than_snap_distance() {
        let project = tempfile::tempdir().expect("temporary project");
        let path = project_config_path(project.path());
        fs::create_dir_all(path.parent().expect("config directory")).expect("config directory");
        fs::write(
            &path,
            r#"{
              "snapDistance": 30,
              "detachDistance": 20
            }"#,
        )
        .expect("window config");

        let error = WindowLayoutConfig::load(project.path()).expect_err("invalid window config");
        assert!(error.contains("detachDistance must be at least snapDistance"));
    }
}
