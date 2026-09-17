use fo3_esm::records::navm::{NavMeshRecord, NavMeshTriangle};
use glam::Vec3;
use std::collections::HashMap;
use fo3_esm::types::FormId;

#[derive(Clone, Debug)]
pub struct Node {
    pub navmesh_id: FormId,
    pub triangle_idx: u16,
    pub center: Vec3,
    pub triangle: NavMeshTriangle,
}

#[derive(Clone, Debug)]
pub struct NavGraph {
    /// キー: NavMeshのFormID、値: そのNavMesh内のポリゴンリスト
    pub nodes: HashMap<FormId, Vec<Node>>,
    /// 外部接続エッジの解決マップ
    /// (NavMesh FormId, Triangle Index, Edge 0/1/2) -> (Target NavMesh FormId, Target Triangle Index)
    pub external_links: HashMap<(FormId, u16, usize), (FormId, u16)>,
}

impl Default for NavGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl NavGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            external_links: HashMap::new(),
        }
    }

    pub fn add_navmesh(&mut self, record: &NavMeshRecord) {
        let mut mesh_nodes = Vec::with_capacity(record.triangles.len());
        
        let mut nvex_index = 0;

        for (i, tri) in record.triangles.iter().enumerate() {
            let v0 = record.vertices[tri.vertices[0] as usize];
            let v1 = record.vertices[tri.vertices[1] as usize];
            let v2 = record.vertices[tri.vertices[2] as usize];
            
            let center = Vec3::new(
                (v0[0] + v1[0] + v2[0]) / 3.0,
                (v0[1] + v1[1] + v2[1]) / 3.0,
                (v0[2] + v1[2] + v2[2]) / 3.0,
            );

            // 外部接続エッジとNVEXリストの相関を解決
            let external_flags = [
                NavMeshTriangle::EDGE0_EXTERNAL,
                NavMeshTriangle::EDGE1_EXTERNAL,
                NavMeshTriangle::EDGE2_EXTERNAL,
            ];

            for (edge_idx, &edge_neighbor) in tri.edges.iter().enumerate() {
                if edge_neighbor == -1 && (tri.flags & external_flags[edge_idx]) != 0 {
                    if let Some(ext) = record.external_connections.get(nvex_index) {
                        self.external_links.insert(
                            (record.form_id, i as u16, edge_idx),
                            (ext.target_navmesh, ext.triangle),
                        );
                        nvex_index += 1;
                    }
                }
            }

            mesh_nodes.push(Node {
                navmesh_id: record.form_id,
                triangle_idx: i as u16,
                center,
                triangle: *tri,
            });
        }
        self.nodes.insert(record.form_id, mesh_nodes);
    }
}
