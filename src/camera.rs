use std::f32::consts::{FRAC_PI_2, PI};

use glam::{EulerRot, Mat4, Quat, Vec3};

use super::transforms::Direction;

pub enum CameraType {
    FirstPerson,
    LookAt,
}

pub struct Camera {
    translation: Vec3,
    rotation: Quat,
    fov: f32, // radius in (0, Pi)
    aspect_ratio: f32,
    z_limits: [f32; 2],
    move_speed: f32,
    rotate_speed: f32,
    camera_type: CameraType,
    view_mat: Mat4,
    perspective_mat: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        let eye = Vec3::new(0., 0., -10.);
        let mut camera = Camera {
            translation: eye,
            rotation: Quat::from_mat4(&Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y)),
            fov: FRAC_PI_2,
            aspect_ratio: 1.,
            z_limits: [0.1, 10.],
            move_speed: 1.,
            rotate_speed: 1.,
            camera_type: CameraType::LookAt,
            view_mat: Mat4::IDENTITY,
            perspective_mat: Mat4::IDENTITY,
        };
        camera.perspective_mat = Mat4::perspective_rh(
            camera.fov,
            camera.aspect_ratio,
            camera.z_limits[0],
            camera.z_limits[1],
        );
        camera.update_view_mat();
        camera
    }
}

impl Camera {
    pub fn with_translation(t: Vec3) -> Self {
        let mut c = Self {
            translation: t,
            ..Default::default()
        };
        c.update_view_mat();
        c
    }

    pub fn with_rotation(mut self, r: Quat) -> Self {
        self.rotation = r;
        self
    }

    /// Set fov in radius within (0, Pi)
    pub fn with_fov(mut self, fov: f32) -> Self {
        assert!(fov > 0. && fov < PI);
        self.fov = fov;
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

    pub fn with_type(mut self, camera_type: CameraType) -> Self {
        self.camera_type = camera_type;
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
        self.update_view_mat();
    }

    pub fn translate_in_time(&mut self, direction: Direction, time: f32) {
        self.translate(direction, time * self.move_speed)
    }

    pub fn rotate(&mut self, direction: Direction, angle: f32) {
        let mut euler = self.rotation.to_euler(EulerRot::XYZ);
        match direction {
            Direction::Up => euler.1 += angle,
            Direction::Down => euler.1 -= angle,
            Direction::Left => euler.0 -= angle,
            Direction::Right => euler.0 += angle,
            Direction::Front => euler.2 += angle,
            Direction::Back => euler.2 -= angle,
        };
        self.rotation = Quat::from_euler(EulerRot::XYZ, euler.0, euler.1, euler.2);
        self.update_view_mat()
    }

    pub fn rotate_in_time(&mut self, direction: Direction, time: f32) {
        self.rotate(direction, time * self.rotate_speed)
    }

    pub fn view_mat(&self) -> Mat4 {
        self.view_mat
    }

    pub fn perspective_mat(&self) -> Mat4 {
        self.perspective_mat
    }

    fn update_view_mat(&mut self) {
        match self.camera_type {
            CameraType::FirstPerson => {
                self.view_mat =
                    Mat4::from_quat(self.rotation) * Mat4::from_translation(self.translation)
            }
            CameraType::LookAt => {
                self.view_mat =
                    Mat4::from_translation(self.translation) * Mat4::from_quat(self.rotation)
            }
        }
    }
}
