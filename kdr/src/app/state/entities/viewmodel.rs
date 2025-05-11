use std::f32::{self, consts::PI};

pub(super) struct ViewModelState {
    // settings
    pub _bob: f32,
    pub _bob_cycle: f32,
    pub _bob_up: f32,
    // result
    pub cycle: f32,
    pub bob: f32,
    pub bob_time: f32,
}

impl Default for ViewModelState {
    fn default() -> Self {
        Self {
            _bob: 0.01,
            _bob_cycle: 0.8,
            _bob_up: 0.5,
            cycle: 0.,
            bob: 0.,
            bob_time: 0.,
        }
    }
}

impl ViewModelState {
    /// Takes delta time, not actual time :()
    pub(super) fn calculate_bob(&mut self, dt: f32) {
        self.bob_time += dt;
        self.cycle = self.bob_time - (self.bob_time / self._bob_cycle).round() * self._bob_cycle;
        self.cycle /= self._bob_cycle;

        if self.cycle < self._bob_up {
            self.cycle = PI * self.cycle / self._bob_up;
        } else {
            self.cycle = PI + PI * (self.cycle - self._bob_up) / (1. - self._bob_up);
        }
    }
}
