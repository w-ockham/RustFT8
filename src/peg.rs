use crate::tanner::TannerGraph;

pub fn ldpc_peg(
    checknode_number: usize,
    symbolnode_number: usize,
    symbolnode_degree: Vec<usize>,
) -> Vec<Vec<u32>> {
    let paritycheck_matrix = vec![vec![0; symbolnode_number]; checknode_number];

    let mut tanner = TannerGraph::new(paritycheck_matrix);
    print!("Symbol=");
    for (idx, degree) in symbolnode_degree.iter().enumerate() {
        //ビットノード毎に検査ノードへのエッジを張る
        let symbol_idx = tanner.get_symbolnode(idx);

        print!("{} ", symbol_idx);
        if symbol_idx % 20 == 0 {
            println!()
        };

        //ビットノードにつながるエッジの個数分繰り返し
        for i in 0..*degree {
            if i == 0 {
                //最初のエッジはタナーグラフ上で
                //もっとも接続が少ない検査ノードと接続
                let lowest = tanner.get_checknode_with_lowest_degree();
                tanner.create_edge(symbol_idx, lowest);

                tanner.output_matrix(symbol_idx);
            } else {
                let mut depth = 0;
                //現在のシンボルノードから幅優先探索で指定された
                //深さでノードをたどり部分グラフを作る
                let mut current_subgraph = tanner.get_subgraph(symbol_idx, depth);
                loop {
                    //全ての検査ノードが包含されている場合
                    if current_subgraph.all_checknodes_covered() {
                        //一つ前の部分グラフで最も接続が少ない検査ノードと接続
                        let previous = tanner.get_subgraph(symbol_idx, depth - 1);
                        let lowest = previous.get_uc_checknode_with_lowest_degree().unwrap();
                        tanner.create_edge(symbol_idx, lowest);
                        tanner.output_matrix(symbol_idx);

                        break;
                    };
                    //深さを一段深くして部分グラフを作り
                    //一番遠い位置にある検査ノードを探索
                    depth += 1;
                    let next_subgraph = tanner.get_subgraph(symbol_idx, depth);
                    if next_subgraph.level == current_subgraph.level {
                        let lowest = current_subgraph
                            .get_uc_checknode_with_lowest_degree()
                            .unwrap();
                        tanner.create_edge(symbol_idx, lowest);
                        tanner.output_matrix(symbol_idx);
                        break;
                    };
                    //更に深く部分グラフを探索
                    current_subgraph = next_subgraph;
                }
            }
        }
    }
    println!();
    tanner.output_edges();

    tanner.matrix
}
