use cgmath::{Deg, ElementWise, InnerSpace, Matrix4, MetricSpace, Point3, Vector3, perspective};

pub struct Camera {
    pub pos: Point3<f32>,
    pub target: Point3<f32>,
    pub up: Vector3<f32>,
    pub aspect: f32,
    pub fovy: Deg<f32>,
    pub znear: f32,
    pub zfar: f32,
}

// const CAM_START_POS: [f32; 3] = [-300., -1000., -2000.];
const CAM_START_POS: [f32; 3] = [1000., 300., 500.];

pub const CAM_SPEED: f32 = 1000.;
pub const CAM_TURN: f32 = 150.; // degrees

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Point3::<f32>::from(CAM_START_POS),
            target: Point3::<f32>::from(CAM_START_POS)
                .add_element_wise(Point3::<f32>::new(2., -2., -2.)),
            up: Vector3::unit_z(), // using the game up vector
            aspect: 640 as f32 / 480 as f32,
            fovy: Deg(90.0),
            znear: 1.0,
            zfar: 10000.0,
        }
    }
}

impl Camera {
    pub fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        self.proj() * self.view()
    }

    pub fn view(&self) -> Matrix4<f32> {
        Matrix4::look_at_rh(self.pos, self.target, self.up)
    }

    pub fn proj(&self) -> Matrix4<f32> {
        perspective(self.fovy, self.aspect, self.znear, self.zfar)
    }

    pub fn rotate_in_place_yaw(&mut self, angle: Deg<f32>) {
        let v = self.pos - self.target;

        let x = v.x;
        let y = v.y;

        let turn = angle.0.to_radians();

        let rotated_x = x * turn.cos() - y * turn.sin();
        let rotatex_y = x * turn.sin() + y * turn.cos();

        self.pos.x = self.target.x + rotated_x;
        self.pos.y = self.target.y + rotatex_y;
    }

    // written by deepseek specifically
    pub fn rotate_in_place_pitch(&mut self, angle: Deg<f32>) {
        // Convert to relative coordinates
        let to_camera = self.pos - self.target;
        let radius = self.pos.distance(self.target);

        // Get current spherical coordinates
        let horizontal_dist = (to_camera.x * to_camera.x + to_camera.y * to_camera.y).sqrt();
        let current_pitch = to_camera.z.atan2(horizontal_dist);

        // Apply pitch change with clamping
        let new_pitch = (current_pitch + angle.0.to_radians()).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );

        // Calculate new position while maintaining yaw
        let yaw = to_camera.y.atan2(to_camera.x);
        self.pos = Point3::new(
            self.target.x + radius * new_pitch.cos() * yaw.cos(),
            self.target.y + radius * new_pitch.cos() * yaw.sin(),
            self.target.z + radius * new_pitch.sin(),
        );
    }

    pub fn move_along_view(&mut self, distance: f32) {
        let v = self.target - self.pos;
        let offset = v.normalize() * distance;

        self.target += offset;
        self.pos += offset;
    }

    pub fn move_along_view_orthogonal(&mut self, distance: f32) {
        let v = self.target - self.pos;
        let up = self.up;
        let orthogonal = v.cross(up);

        let offset = orthogonal.normalize() * distance;

        self.target += offset;
        self.pos += offset;
    }

    pub fn set_yaw(&mut self, yaw: Deg<f32>) {
        let (sin, cos) = yaw.0.to_radians().sin_cos();
        // let z = self.target.z;

        self.target = [
            self.pos.x + cos,
            self.pos.y + sin,
            self.pos.z
        ].into();
    }
}
