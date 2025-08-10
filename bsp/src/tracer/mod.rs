use glam::FloatExt;

use crate::{Bsp, ClipNode, LeafContent, Node, Plane};

mod types;

pub use types::{HullType, TraceResult};

const DIST_EPSILON: f32 = 1. / 32.;

#[inline]
fn plane_diff(p: glam::Vec3, plane: &Plane) -> f32 {
    if (plane.type_ as u32) < 3 {
        p[plane.type_ as usize] - plane.distance
    } else {
        plane.normal.dot(p.into()) - plane.distance
    }
}

// Type erasure shit so that we can do generic shit
enum NodesType<'a> {
    Nodes(&'a Vec<Node>),
    ClipNodes(&'a Vec<ClipNode>),
}

enum NodeType<'a> {
    Node(&'a Node),
    ClipNode(&'a ClipNode),
}

impl<'a> NodesType<'a> {
    fn len(&self) -> usize {
        match self {
            NodesType::Nodes(nodes) => nodes.len(),
            NodesType::ClipNodes(clip_nodes) => clip_nodes.len(),
        }
    }

    fn get_unchecked(&self, idx: usize) -> NodeType<'a> {
        match self {
            NodesType::Nodes(nodes) => NodeType::Node(&nodes[idx]),
            NodesType::ClipNodes(clip_nodes) => NodeType::ClipNode(&clip_nodes[idx]),
        }
    }
}

impl<'a> NodeType<'a> {
    fn plane(&self) -> u32 {
        match self {
            NodeType::Node(node) => node.plane,
            NodeType::ClipNode(clip_node) => clip_node.plane as u32,
        }
    }

    fn children(&self) -> [i16; 2] {
        match self {
            NodeType::Node(node) => node.children,
            NodeType::ClipNode(clip_node) => clip_node.children,
        }
    }
}

impl Bsp {
    fn interpret_leaf_content(&self, hull_type: HullType, num: i32) -> LeafContent {
        match hull_type {
            HullType::Point => self.leaves[!num as usize].contents,
            HullType::Stand | HullType::Monster | HullType::Duck => {
                LeafContent::try_from(num).expect("unknown leaf content")
            }
        }
    }

    pub fn trace_line_hull(
        &self,
        hull_type: HullType,
        p1: glam::Vec3,
        p2: glam::Vec3,
    ) -> TraceResult {
        let mut tr = TraceResult::default();
        let head_node = self.models[0].head_nodes[hull_type as i32 as usize];

        self.trace_line_hull_recursive(hull_type, head_node, p1, p2, 0., 1., &mut tr);

        return tr;
    }

    // code pullled from xash3d repo
    fn trace_line_hull_recursive(
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
            let leaf_content = self.interpret_leaf_content(hull_type, num);

            if !matches!(leaf_content, LeafContent::ContentsSolid) {
                trace.all_solid = false;
            } else {
                trace.start_solid = true;
            }

            return true;
        }

        // let node_index = (hull_start + num) as usize;
        let node_index = num as usize;

        let nodes = match hull_type {
            HullType::Point => NodesType::Nodes(&self.nodes),
            HullType::Stand | HullType::Monster | HullType::Duck => {
                NodesType::ClipNodes(&self.clipnodes)
            }
        };

        if node_index >= nodes.len() {
            return false;
        }

        let node = &nodes.get_unchecked(node_index);
        let plane = &self.planes[node.plane() as usize];

        let d1 = plane_diff(p1, plane);
        let d2 = plane_diff(p2, plane);

        // positive side
        if d1 >= 0. && d2 >= 0. {
            return self.trace_line_hull_recursive(
                hull_type,
                node.children()[0] as i32,
                p1,
                p2,
                p1f,
                p2f,
                trace,
            );
        }

        // negative side
        if d1 < 0. && d2 < 0. {
            return self.trace_line_hull_recursive(
                hull_type,
                node.children()[1] as i32,
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
        if !self.trace_line_hull_recursive(
            hull_type,
            node.children()[side] as i32,
            p1,
            mid,
            p1f,
            midf,
            trace,
        ) {
            return false;
        }

        if !matches!(
            self.trace_point_hull_internal(
                &nodes,
                hull_type,
                node.children()[side ^ 1] as i32,
                mid
            ),
            LeafContent::ContentsSolid
        ) {
            // if not solid then keep tracing with the latter half
            return self.trace_line_hull_recursive(
                hull_type,
                node.children()[side ^ 1] as i32,
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
            self.trace_point_hull_internal(&nodes, hull_type, head_node, mid),
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

    // for internal use because it looks very clean
    fn trace_point_hull_internal(
        &self,
        nodes: &NodesType,
        hull_type: HullType,
        mut num: i32,
        p1: glam::Vec3,
    ) -> LeafContent {
        while num >= 0 {
            let node = nodes.get_unchecked(num as usize);
            let plane = &self.planes[node.plane() as usize];

            let distance = plane_diff(p1, plane);

            if distance < 0. {
                num = node.children()[1] as i32;
            } else {
                num = node.children()[0] as i32;
            }
        }

        self.interpret_leaf_content(hull_type, num)
    }

    // code pulled from xash3d repo
    pub fn trace_point_hull(
        &self,
        hull_type: HullType,
        mut num: i32,
        p1: glam::Vec3,
    ) -> LeafContent {
        let num = match hull_type {
            // POINT hull uses "bsp.nodes"
            HullType::Point => {
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

                num
            }
            // STAND, MONSTER, DUCK use "bsp.clipnodes"
            HullType::Stand | HullType::Monster | HullType::Duck => {
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

                num
            }
        };

        self.interpret_leaf_content(hull_type, num)
    }
}

#[cfg(test)]
mod test {
    use crate::LeafContent;

    #[test]
    fn trace1() {
        let file = include_bytes!("../tests/normal.bsp");

        let bsp = crate::parse_bsp(file).unwrap();

        let res = bsp.trace_point_hull(crate::HullType::Point, 0, [0., 0., 0.].into());

        assert!(matches!(res, LeafContent::ContentsSolid));
    }
}
