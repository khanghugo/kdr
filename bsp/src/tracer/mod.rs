use glam::FloatExt;

use crate::{Bsp, LeafContent, Plane};

mod types;

pub use types::{HullType, TraceResult};

const DIST_EPSILON: f32 = 1. / 32.;

#[inline]
fn plane_diff(p: glam::Vec3, plane: &Plane) -> f32 {
    let type_ = plane.type_ as usize;

    if (type_) < 3 {
        p[type_] - plane.distance
    } else {
        plane.normal.dot(p.into()) - plane.distance
    }
}

impl Bsp {
    pub fn trace_line(&self, hull_type: HullType, p1: glam::Vec3, p2: glam::Vec3) -> TraceResult {
        let mut tr = TraceResult::default();
        let head_node = self.models[0].head_nodes[hull_type as i32 as usize];

        match hull_type {
            HullType::Point => self.trace_line_point(head_node, p1, p2, 0., 1., &mut tr),
            HullType::Stand | HullType::Monster | HullType::Duck => {
                self.trace_line_hull(hull_type, head_node, p1, p2, 0., 1., &mut tr)
            }
        };

        return tr;
    }

    // ONLY FOR HULL 1 to 3
    fn trace_line_hull(
        &self,
        hull_type: HullType,
        num: i32,
        p1: glam::Vec3,
        p2: glam::Vec3,
        p1f: f32,
        p2f: f32,
        trace: &mut TraceResult,
    ) -> bool {
        if num < 0 {
            let leaf_content = LeafContent::try_from(num).expect("unknown leaf content");

            if !matches!(leaf_content, LeafContent::ContentsSolid) {
                trace.all_solid = false;
            } else {
                trace.start_solid = true;
            }

            return true;
        }

        let node_index = num as usize;

        if node_index >= self.clipnodes.len() {
            return false;
        }

        let node = &self.clipnodes[node_index];
        let plane = &self.planes[node.plane as usize];

        let d1 = plane_diff(p1, plane);
        let d2 = plane_diff(p2, plane);

        // positive side
        if d1 >= 0. && d2 >= 0. {
            return self.trace_line_hull(
                hull_type,
                node.children[0] as i32,
                p1,
                p2,
                p1f,
                p2f,
                trace,
            );
        }

        // negative side
        if d1 < 0. && d2 < 0. {
            return self.trace_line_hull(
                hull_type,
                node.children[1] as i32,
                p1,
                p2,
                p1f,
                p2f,
                trace,
            );
        }

        // until the segment intersects a node/plane
        // side gives the child containing p1
        // frac to calculate the interection point
        let (side, frac) = if d1 < 0. {
            (1, (d1 + DIST_EPSILON) / (d1 - d2))
        } else {
            (0, (d1 - DIST_EPSILON) / (d1 - d2))
        };

        let mut frac = frac.clamp(0., 1.);

        let mut midf = p1f.lerp(p2f, frac);
        let mut mid = p1.lerp(p2, frac);

        // back to front
        if !self.trace_line_hull(
            hull_type,
            node.children[side] as i32,
            p1,
            mid,
            p1f,
            midf,
            trace,
        ) {
            return false;
        }

        if !matches!(
            self.trace_point_hull(node.children[side ^ 1] as i32, mid),
            LeafContent::ContentsSolid
        ) {
            // if not solid then keep tracing with the latter half
            return self.trace_line_hull(
                hull_type,
                node.children[side ^ 1] as i32,
                mid,
                p2,
                midf,
                p2f,
                trace,
            );
        }

        if trace.all_solid {
            return false;
        }

        if side == 0 {
            trace.plane = plane.clone();
        } else {
            trace.plane = plane.flip();
        }

        // moving the fraction value out of solid
        let head_node = self.models[0].head_nodes[hull_type as i32 as usize];

        while matches!(
            self.trace_point_hull(head_node, mid),
            LeafContent::ContentsSolid
        ) {
            frac -= 0.1;

            if frac < 0. {
                trace.fraction = midf;
                trace.end_pos = mid;

                return false;
            }

            midf = p1f.lerp(p2f, frac);
            mid = p1.lerp(p2, frac);
        }

        trace.fraction = midf;
        trace.end_pos = mid;

        return false;
    }

    /// ONLY FOR HULL 0
    fn trace_line_point(
        &self,
        num: i32,
        p1: glam::Vec3,
        p2: glam::Vec3,
        p1f: f32,
        p2f: f32,
        trace: &mut TraceResult,
    ) -> bool {
        if num < 0 {
            let leaf_content = self.leaves[!num as usize].contents;

            if !matches!(leaf_content, LeafContent::ContentsSolid) {
                trace.all_solid = false;
            } else {
                trace.start_solid = true;
            }

            return true;
        }

        let node_index = num as usize;

        if node_index >= self.nodes.len() {
            return false;
        }

        let node = &self.nodes[node_index];
        let plane = &self.planes[node.plane as usize];

        let d1 = plane_diff(p1, plane);
        let d2 = plane_diff(p2, plane);

        // positive side
        if d1 >= 0. && d2 >= 0. {
            return self.trace_line_point(node.children[0] as i32, p1, p2, p1f, p2f, trace);
        }

        // negative side
        if d1 < 0. && d2 < 0. {
            return self.trace_line_point(node.children[1] as i32, p1, p2, p1f, p2f, trace);
        }

        // until the segment intersects a node/plane
        // side gives the child containing p1
        // frac to calculate the interection point
        let (side, frac) = if d1 < 0. {
            (1, (d1 + DIST_EPSILON) / (d1 - d2))
        } else {
            (0, (d1 - DIST_EPSILON) / (d1 - d2))
        };

        let mut frac = frac.clamp(0., 1.);

        let mut midf = p1f.lerp(p2f, frac);
        let mut mid = p1.lerp(p2, frac);

        // back to front
        if !self.trace_line_point(node.children[side] as i32, p1, mid, p1f, midf, trace) {
            return false;
        }

        if !matches!(
            self.trace_point(node.children[side ^ 1] as i32, mid),
            LeafContent::ContentsSolid
        ) {
            // if not solid then keep tracing with the latter half
            return self.trace_line_point(
                node.children[side ^ 1] as i32,
                mid,
                p2,
                midf,
                p2f,
                trace,
            );
        }

        if trace.all_solid {
            return false;
        }

        if side == 0 {
            trace.plane = plane.clone();
        } else {
            trace.plane = plane.flip();
        }

        // moving the fraction value out of solid
        while matches!(self.trace_point(0, mid), LeafContent::ContentsSolid) {
            frac -= 0.1;

            if frac < 0. {
                trace.fraction = midf;
                trace.end_pos = mid;

                return false;
            }

            midf = p1f.lerp(p2f, frac);
            mid = p1.lerp(p2, frac);
        }

        trace.fraction = midf;
        trace.end_pos = mid;

        return false;
    }

    /// Use this function exclusively for HULL POINT (HULL 0)
    pub fn trace_point(&self, mut num: i32, p1: glam::Vec3) -> LeafContent {
        while num >= 0 {
            let node = &self.nodes[num as usize];
            let plane = &self.planes[node.plane as usize];

            let distance = plane_diff(p1, plane);

            if distance < 0. {
                num = node.children[1] as i32;
            } else {
                num = node.children[0] as i32;
            }
        }

        return self.leaves[!num as usize].contents;
    }

    /// Do not use this for HULL POINT (HULL 0)
    ///
    /// Use [`Bsp::trace_point`] instead
    pub fn trace_point_hull(&self, mut num: i32, p1: glam::Vec3) -> LeafContent {
        while num >= 0 {
            let node = &self.clipnodes[num as usize];
            let plane = &self.planes[node.plane as usize];

            let distance = plane_diff(p1, plane);

            if distance < 0. {
                num = node.children[1] as i32;
            } else {
                num = node.children[0] as i32;
            }
        }

        return LeafContent::try_from(num).expect("unknown leaf content");
    }
}

#[cfg(test)]
mod test {}
