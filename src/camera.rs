use std::f32::consts::{FRAC_PI_2, PI};

use glam::{EulerRot, Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub enum CameraType {
    FirstPerson,
    LookAt,
}

pub struct CameraBuilder {
    translation: Vec3,
    rotation: (f32, f32, f32),
    fov: f32, // radius in (0, Pi)
    aspect_ratio: f32,
    z_limits: [f32; 2],
    move_speed: f32,
    rotate_speed: f32,
    camera_type: CameraType,
}

impl Default for CameraBuilder {
    fn default() -> Self {
        let eye = Vec3::new(0., 0., -10.);
        CameraBuilder {
            translation: eye,
            rotation: Quat::from_mat4(&Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y))
                .to_euler(EulerRot::XYZ),
            fov: FRAC_PI_2,
            aspect_ratio: 1.,
            z_limits: [0.1, 10.],
            move_speed: 1.,
            rotate_speed: 1.,
            camera_type: CameraType::LookAt,
        }
    }
}

impl CameraBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(&self) -> Camera {
        let mut camera = Camera {
            translation: self.translation,
            rotation: self.rotation,
            fov: self.fov,
            aspect_ratio: self.aspect_ratio,
            z_limits: self.z_limits,
            move_speed: self.move_speed,
            rotate_speed: self.rotate_speed,
            camera_type: self.camera_type,
            view_mat: Mat4::IDENTITY,
            perspective_mat: Mat4::IDENTITY,
        };
        camera.update_perspective_mat();
        camera.update_view_mat();
        camera
    }

    pub fn translation(mut self, t: Vec3) -> Self {
        self.translation = t;
        self
    }

    pub fn rotation(mut self, r: (f32, f32, f32)) -> Self {
        self.rotation = r;
        self
    }

    /// Set fov in radius within (0, Pi)
    pub fn fov(mut self, fov: f32) -> Self {
        assert!(fov > 0. && fov < PI);
        self.fov = fov;
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = aspect_ratio;
        self
    }

    pub fn z_limits(mut self, z_limits: [f32; 2]) -> Self {
        self.z_limits = z_limits;
        self
    }

    pub fn move_speed(mut self, move_speed: f32) -> Self {
        self.move_speed = move_speed;
        self
    }

    pub fn rotate_speed(mut self, rotate_speed: f32) -> Self {
        self.rotate_speed = rotate_speed;
        self
    }

    pub fn with_type(mut self, camera_type: CameraType) -> Self {
        self.camera_type = camera_type;
        self
    }
}

pub struct Camera {
    translation: Vec3,
    rotation: (f32, f32, f32),
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
        CameraBuilder::default().build()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> CameraBuilder {
        CameraBuilder::default()
    }

    pub fn set_translation(&mut self, t: Vec3) {
        self.translation = t;
        self.update_view_mat();
    }

    pub fn set_rotation(&mut self, r: (f32, f32, f32)) {
        self.rotation = r;
        self.update_view_mat();
    }

    /// Set fov in radius within (0, Pi)
    pub fn set_fov(&mut self, fov: f32) {
        assert!(fov > 0. && fov < PI);
        self.fov = fov;
        self.update_perspective_mat();
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
        self.update_perspective_mat();
    }

    pub fn set_z_limits(&mut self, z_limits: [f32; 2]) {
        self.z_limits = z_limits;
        self.update_perspective_mat();
    }

    pub fn set_move_speed(&mut self, move_speed: f32) {
        self.move_speed = move_speed;
    }

    pub fn set_rotate_speed(&mut self, rotate_speed: f32) {
        self.rotate_speed = rotate_speed;
    }

    pub fn set_type(&mut self, camera_type: CameraType) {
        self.camera_type = camera_type;
        self.update_view_mat();
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
        match direction {
            Direction::Up => self.rotation.1 += angle,
            Direction::Down => self.rotation.1 -= angle,
            Direction::Left => self.rotation.0 -= angle,
            Direction::Right => self.rotation.0 += angle,
            Direction::Front => self.rotation.2 += angle,
            Direction::Back => self.rotation.2 -= angle,
        };
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
        let (x, y, z) = self.rotation;
        let mat_rot = Mat4::from_euler(EulerRot::XYZ, x, y, z);
        let mat_trans = Mat4::from_translation(self.translation);
        match self.camera_type {
            CameraType::FirstPerson => self.view_mat = mat_rot * mat_trans,
            CameraType::LookAt => self.view_mat = mat_trans * mat_rot,
        }
    }

    fn update_perspective_mat(&mut self) {
        self.perspective_mat = Mat4::perspective_rh(
            self.fov,
            self.aspect_ratio,
            self.z_limits[0],
            self.z_limits[1],
        );
    }

    pub fn mvp_matrix(&self, model: Mat4) -> MVPMatrix {
        MVPMatrix {
            model,
            view: self.view_mat,
            perspective: self.perspective_mat,
        }
    }
}

#[repr(C, align(16))]
pub struct MVPMatrix {
    pub model: Mat4,
    pub view: Mat4,
    pub perspective: Mat4,
}

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    Front,
    Back,
}
