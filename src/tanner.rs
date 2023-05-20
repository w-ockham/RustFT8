use std::{collections::{HashMap, HashSet}};

#[derive(Debug)]
pub enum NodeType {
    SymbolNode(usize),
    CheckNode(usize),
}

#[derive(Debug)]

pub struct TannerGraph {
    pub matrix: Vec<Vec<u32>>,
    nodes: Vec<NodeType>,
    checknode_len: usize,
    edges: HashMap<usize, Vec<usize>>,
}

impl<'a> TannerGraph {
    pub fn add_edge(h:&mut HashMap<usize,Vec<usize>>,k:usize, v:usize) {
         let Some(vl) = h.get_mut(&k)else {panic!("Node not found {}",k)};
         vl.push(v);
    }

    pub fn new(matrix: Vec<Vec<u32>>) -> TannerGraph {
        let mut nodes = vec![];
        let mut edges = HashMap::<usize,Vec<usize>>::new();
        let offset = matrix[0].len();
        let checknode_len = matrix.len();

        matrix[0].iter().enumerate().for_each(|(index, _)| {
            nodes.push(NodeType::SymbolNode(index));
            edges.insert(index,vec![]);
        });

        matrix.iter().enumerate().for_each(|(index, _)| {
            nodes.push(NodeType::CheckNode(index));
            edges.insert(index + offset,vec![]);
        });

        for (i, _) in matrix.iter().enumerate() {
            for (j, _) in matrix[i].iter().enumerate() {
                if matrix[i][j] == 1 {
                    TannerGraph::add_edge(&mut edges, i+ offset, j);
                    TannerGraph::add_edge(&mut edges, j, i + offset);
                }
            }
        }
        Self {
            matrix,
            nodes,
            checknode_len,
            edges,
        }
    }

    pub fn is_checknode(&self, node_id: usize) -> bool {
      matches!(self.nodes[node_id], NodeType::CheckNode(_))
    }

    pub fn get_symbolnode(&self, idx:usize) -> usize {
        let NodeType::SymbolNode(i) = self.nodes[idx] else { panic!("Node not found {}",idx)};
        i
    }
    
    pub fn checknode_edge_len(&self, idx: usize) -> usize {
        let Some(e) = self.edges.get(&idx) else { panic!("Node not found  {}",idx)};
        e.len()
    }

    pub fn get_checknode_with_lowest_degree(&self) -> usize {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(*n, NodeType::CheckNode(_)))
            .map(|(idx, _)| (idx, self.checknode_edge_len(idx)))
            .reduce(|(idx1, l1), (idx2, l2)| if l1 > l2 { (idx2, l2) } else { (idx1, l1) })
            .unwrap()
            .0
    }

    pub fn get_uncovered_checknodes(&self, hash: &HashSet<usize>) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(idx, n)| matches!(*n, NodeType::CheckNode(_)) && !hash.contains(idx))
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn create_edge(&mut self, symbol_idx: usize, check_idx: usize) {
        
        TannerGraph::add_edge(&mut self.edges, check_idx, symbol_idx);
        TannerGraph::add_edge(&mut self.edges, symbol_idx, check_idx);
        
        let NodeType::CheckNode(check_id)  = self.nodes[check_idx] else {panic!("Check node not found {}",check_idx)};
        let NodeType::SymbolNode(symbol_id)  = self.nodes[symbol_idx] else {panic!("Symbol node not found {}",symbol_idx);};
        self.matrix[check_id][symbol_id] = 1;
    }

    pub fn neighbors(&self, idx: usize) -> Vec<usize> {
        let Some(v) = self.edges.get(&idx) else {panic!("Node not found {}",idx)};
        v.clone()
    }

    pub fn get_subgraph(&'a self, symbol_idx: usize, level: usize) -> SubGraph {
        SubGraph::new(self, symbol_idx, level)
    }

    pub fn output_matrix(&self, nodeid: usize) {
        if cfg!(debug_assertions) {
            println!("Symbol({:?}) ", nodeid);
            self.matrix.iter().for_each(|m| println!("{:?}", m));
            println!("----------------------------")
        }
    }

    pub fn output_edges(&self) {
        println!("Result=");
        self.output_matrix(0);

        self.nodes.iter().enumerate()
        .for_each(|(idx, n)| {
            if let NodeType::SymbolNode(i) = n {
                print!("Symbol({:?}) ->", i);
                self.neighbors(idx).iter().for_each(|chk| {
                    let NodeType::CheckNode(idx) = self.nodes[*chk] else { panic!("Check node not found {}",*chk)};
                    print!("Check({:?}) ", idx)
                });
                println!()
            }
        });
    }
}

#[derive(Debug)]
pub struct RootNode {
    root_idx: usize,
    children: Vec<RootNode>,
}

pub struct SubGraph<'a> {
    tanner: &'a TannerGraph,
    pub level: usize,
    tree_root: RootNode,
}

impl<'a> SubGraph<'a> {
    fn bfs_tanner(
        tanner: &'a TannerGraph,
        root_idx: usize,
        mut level: usize,
        depth: usize,
        used: &mut HashSet<usize>,
    ) -> (usize, Vec<RootNode>) {
        level += 1;

        let mut children_node: Vec<_> = tanner
            .neighbors(root_idx)
            .iter()
            .filter(|c| !used.contains(*c))
            .map(|n| RootNode {
                root_idx: *n,
                children: vec![],
            })
            .collect();

        children_node.iter().for_each(|r| {
            used.insert(r.root_idx);
        });

        if level <= depth {
            children_node.iter_mut().for_each(|r| {
                (level, r.children) = Self::bfs_tanner(tanner, r.root_idx, level, depth, used)
            });
        };
        (level, children_node)
    }

    pub fn new(tanner: &'a TannerGraph, idx: usize, depth: usize) -> SubGraph<'a> {
        let mut used = HashSet::<usize>::new();
        let (level, children) = Self::bfs_tanner(tanner, idx, 0, depth, &mut used);
        Self {
            tanner,
            tree_root: RootNode {
                root_idx: idx,
                children,
            },
            level,
        }
    }

    fn dig_root_node(&self, root: &RootNode, hash: &mut HashSet<usize>) {
        root.children
            .iter()
            .for_each(|f| self.dig_root_node(f, hash));

        if self.tanner.is_checknode(root.root_idx) {
            hash.insert(root.root_idx);
        };
    }

    pub fn covered_checknodes(&self) -> HashSet<usize> {
        let mut hash =  HashSet::<usize>::new();
        self.dig_root_node(&self.tree_root, &mut hash);
        hash
    }

    pub fn all_checknodes_covered(&self) -> bool {
        self.tanner.checknode_len == self.covered_checknodes().len()
    }

    pub fn get_uc_checknode_with_lowest_degree(&self) -> Option<usize> {
        let covered = self.covered_checknodes();
        let uncovered = self.tanner.get_uncovered_checknodes(&covered);

        let Some((idx, _)) = 
        uncovered.iter().map(|i|(*i,self.tanner.checknode_edge_len(*i)))
        .reduce(|(i,l),(j,m)| if l > m {(j,m)} else {(i,l)}) else { return None;};

        Some(idx)
    }
}
