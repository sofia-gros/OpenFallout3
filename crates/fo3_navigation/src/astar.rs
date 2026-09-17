use crate::graph::NavGraph;
use crate::path::NavPath;
use fo3_esm::types::FormId;
use glam::Vec3;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

#[derive(Copy, Clone, PartialEq)]
struct State {
    cost: f32,
    navmesh_id: FormId,
    triangle_idx: u16,
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn find_path(
    graph: &NavGraph,
    start_mesh: FormId,
    start_tri: u16,
    end_mesh: FormId,
    end_tri: u16,
) -> Option<NavPath> {
    let mut distances = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut came_from = HashMap::new();

    distances.insert((start_mesh, start_tri), 0.0f32);
    heap.push(State {
        cost: 0.0,
        navmesh_id: start_mesh,
        triangle_idx: start_tri,
    });

    let end_center = graph.nodes.get(&end_mesh)?.get(end_tri as usize)?.center;

    while let Some(State { cost: _, navmesh_id, triangle_idx }) = heap.pop() {
        if navmesh_id == end_mesh && triangle_idx == end_tri {
            let mut path_nodes = Vec::new();
            let mut curr = (navmesh_id, triangle_idx);
            
            while let Some(&prev) = came_from.get(&curr) {
                path_nodes.push(curr);
                curr = prev;
            }
            path_nodes.push((start_mesh, start_tri));
            path_nodes.reverse();
            
            let mut points = Vec::new();
            for &(mesh, tri) in &path_nodes {
                if let Some(nodes) = graph.nodes.get(&mesh) {
                    if let Some(node) = nodes.get(tri as usize) {
                        points.push(node.center);
                    }
                }
            }
            
            return Some(NavPath { points, nodes: path_nodes });
        }

        let curr_dist = *distances.get(&(navmesh_id, triangle_idx)).unwrap();
        let curr_node = &graph.nodes.get(&navmesh_id)?[triangle_idx as usize];

        for (edge_idx, &edge_neighbor) in curr_node.triangle.edges.iter().enumerate() {
            let mut neighbor = None;
            
            if edge_neighbor >= 0 {
                // 同じNavMesh内の隣接ポリゴン
                neighbor = Some((navmesh_id, edge_neighbor as u16));
            } else {
                // 外部接続エッジの解決 (NVEX)
                if let Some(&(target_mesh, target_tri)) = graph.external_links.get(&(navmesh_id, triangle_idx, edge_idx)) {
                    neighbor = Some((target_mesh, target_tri));
                }
            }

            if let Some((next_mesh, next_tri)) = neighbor {
                if let Some(next_mesh_nodes) = graph.nodes.get(&next_mesh) {
                    if let Some(neighbor_node) = next_mesh_nodes.get(next_tri as usize) {
                        let dist = curr_node.center.distance(neighbor_node.center);
                        let next_dist = curr_dist + dist;
                        
                        let is_better = match distances.get(&(next_mesh, next_tri)) {
                            Some(&d) => next_dist < d,
                            None => true,
                        };

                        if is_better {
                            distances.insert((next_mesh, next_tri), next_dist);
                            came_from.insert((next_mesh, next_tri), (navmesh_id, triangle_idx));
                            
                            let h = neighbor_node.center.distance(end_center);
                            heap.push(State {
                                cost: next_dist + h,
                                navmesh_id: next_mesh,
                                triangle_idx: next_tri,
                            });
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NavGraph;
    use fo3_esm::records::navm::{NavMeshRecord, NavMeshTriangle, NavMeshExternalConnection};
    use fo3_esm::types::FormId;
    use glam::Vec3;

    #[test]
    fn test_external_connection_astar() {
        let mut graph = NavGraph::new();
        
        let mut navm1 = NavMeshRecord {
            form_id: FormId(1),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            triangles: vec![
                NavMeshTriangle {
                    vertices: [0, 1, 2],
                    edges: [-1, -1, -1],
                    flags: NavMeshTriangle::EDGE1_EXTERNAL,
                }
            ],
            external_connections: vec![
                NavMeshExternalConnection {
                    target_navmesh: FormId(2),
                    triangle: 0,
                }
            ],
            ..Default::default()
        };
        
        let mut navm2 = NavMeshRecord {
            form_id: FormId(2),
            vertices: vec![
                [10.0, 0.0, 0.0],
                [20.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
            ],
            triangles: vec![
                NavMeshTriangle {
                    vertices: [0, 1, 2],
                    edges: [-1, -1, -1],
                    flags: NavMeshTriangle::EDGE0_EXTERNAL,
                }
            ],
            external_connections: vec![
                NavMeshExternalConnection {
                    target_navmesh: FormId(1),
                    triangle: 0,
                }
            ],
            ..Default::default()
        };

        graph.add_navmesh(&navm1);
        graph.add_navmesh(&navm2);

        let path = find_path(&graph, FormId(1), 0, FormId(2), 0);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.nodes.len(), 2);
        assert_eq!(path.nodes[0], (FormId(1), 0));
        assert_eq!(path.nodes[1], (FormId(2), 0));
    }
}

