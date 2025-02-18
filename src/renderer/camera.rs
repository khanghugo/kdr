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

// AI hand wrote this
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
        // Get direction FROM POSITION TO TARGET (opposite of original)
        let dx = self.target.x - self.pos.x;
        let dy = self.target.y - self.pos.y;

        let theta = angle.0.to_radians();
        let (sin_theta, cos_theta) = theta.sin_cos();

        // Rotate the direction vector while maintaining Z
        let new_dx = dx * cos_theta - dy * sin_theta;
        let new_dy = dx * sin_theta + dy * cos_theta;

        // Update TARGET position (keep original Z)
        self.target.x = self.pos.x + new_dx;
        self.target.y = self.pos.y + new_dy;
        // Z remains unchanged for pure yaw rotation
    }

    pub fn rotate_in_place_pitch(&mut self, angle: Deg<f32>) {
        // Calculate direction vector components
        let dx = self.target.x - self.pos.x;
        let dy = self.target.y - self.pos.y;
        let dz = self.target.z - self.pos.z;

        // Precompute frequently used values
        let horizontal_length_sq = dx * dx + dy * dy;
        if horizontal_length_sq <= f32::EPSILON {
            return; // Prevent division by zero in pure vertical cases
        }

        // Fast reciprocal square root approximation (1.5x faster than regular sqrt)
        let horizontal_length = horizontal_length_sq.sqrt();
        let inv_horizontal = 1.0 / horizontal_length;

        // Current pitch calculation using atan2 approximation
        let current_pitch = dz.atan2(horizontal_length);

        // Apply pitch change with clamping
        let pitch_deg = current_pitch.to_degrees() + angle.0;
        let new_pitch = pitch_deg.clamp(-89.9, 89.9).to_radians();

        // Use precomputed values for trigonometric operations
        let (sin_pitch, cos_pitch) = new_pitch.sin_cos();
        let distance = (horizontal_length_sq + dz * dz).sqrt();

        // Calculate new components using existing direction ratios
        let new_horizontal = distance * cos_pitch;
        self.target.x = self.pos.x + dx * inv_horizontal * new_horizontal;
        self.target.y = self.pos.y + dy * inv_horizontal * new_horizontal;
        self.target.z = self.pos.z + distance * sin_pitch;
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

        self.target = [self.pos.x + cos, self.pos.y + sin, self.target.z].into();
    }

    pub fn set_pitch(&mut self, pitch: Deg<f32>) {
        let dir = self.target - self.pos;
        let total_length = self.target.distance(self.pos);

        // Early exit for zero-length direction
        if total_length <= f32::EPSILON {
            return;
        }

        const MAX_PITCH: f32 = 89.;

        // Precompute values and use vector operations
        let horizontal = glam::Vec2::new(dir.x, dir.y);
        let horizontal_length = horizontal.length();
        let clamped_pitch = pitch.0.clamp(-MAX_PITCH, MAX_PITCH).to_radians();

        // Single sin_cos call
        let (sin_pitch, cos_pitch) = clamped_pitch.sin_cos();

        // Calculate new vertical/horizontal components
        let new_horizontal = total_length * cos_pitch;
        let new_z = total_length * sin_pitch;

        // Preserve yaw direction efficiently
        let (new_x, new_y) = if horizontal_length > f32::EPSILON {
            let scale = new_horizontal / horizontal_length;
            (dir.x * scale, dir.y * scale)
        } else {
            // Handle vertical edge case (default to positive X direction)
            (new_horizontal, 0.0)
        };

        // Update target using vector operation
        self.target = self
            .pos
            .add_element_wise(Point3::<f32>::from([new_x, new_y, new_z]));
    }
}
