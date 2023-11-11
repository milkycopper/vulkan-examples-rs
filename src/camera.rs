use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Mat4, Quat, Vec3};

use super::transforms::Direction;

pub struct Camera {
    translation: Vec3,
    rotation: Quat,
    visual_angle: f32, // radius in (0, Pi)
    aspect_ratio: f32,
    z_limits: [f32; 2],
    move_speed: f32,
    rotate_speed: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            translation: Vec3::default(),
            rotation: Quat::default(),
            visual_angle: FRAC_PI_2,
            aspect_ratio: 1.,
            z_limits: [0.1, 10.],
            move_speed: 1.,
            rotate_speed: 1.,
        }
    }
}

impl Camera {
    pub fn with_translation(t: Vec3) -> Self {
        Self {
            translation: t,
            ..Default::default()
        }
    }

    pub fn with_rotation(mut self, r: Quat) -> Self {
        self.rotation = r;
        self
    }

    /// Set visual angle in radius within (0, Pi)
    pub fn with_visual_angle(mut self, angle: f32) -> Self {
        assert!(angle > 0. && angle < PI);
        self.visual_angle = angle;
        self
    }

    pub fn with_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = aspect_ratio;
        self
    }

    pub fn with_z_limits(mut self, z_limits: [f32; 2]) -> Self {
        self.z_limits = z_limits;
        self
    }

    pub fn with_move_speed(mut self, move_speed: f32) -> Self {
        self.move_speed = move_speed;
        self
    }

    pub fn with_rotate_speed(mut self, rotate_speed: f32) -> Self {
        self.rotate_speed = rotate_speed;
        self
    }

    pub fn translate(&mut self, direction: Direction, distance: f32) {
        let moving_direction = match direction {
            Direction::Up => Vec3::Y,
            Direction::Down => Vec3::NEG_Y,
            Direction::Left => Vec3::NEG_X,
            Direction::Right => Vec3::X,
            Direction::Front => Vec3::Z,
            Direction::Back => Vec3::NEG_Z,
        };
        self.translation += moving_direction * distance;
    }

    pub fn translate_in_time(&mut self, direction: Direction, time: f32) {
        self.translate(direction, time * self.move_speed)
    }

    pub fn rotate(&mut self, direction: Direction, angle: f32) {
        let axis = self.rotation
            * match direction {
                Direction::Up => Vec3::X,
                Direction::Down => Vec3::NEG_X,
                Direction::Left => Vec3::Y,
                Direction::Right => Vec3::NEG_Y,
                Direction::Front | Direction::Back => unreachable!(),
            };
        self.rotation *= Quat::from_axis_angle(axis, angle);
    }

    pub fn rotate_in_time(&mut self, direction: Direction, time: f32) {
        self.rotate(direction, time * self.rotate_speed)
    }

    pub fn view_transform(&self) -> Mat4 {
        Mat4::look_at_rh(
            self.translation,
            self.rotation * Vec3::Z,
            self.rotation * Vec3::Y,
        )
    }

    pub fn projection_transform(&self) -> Mat4 {
        Mat4::perspective_rh(
            self.visual_angle,
            self.aspect_ratio,
            self.z_limits[0],
            self.z_limits[1],
        )
    }
}
