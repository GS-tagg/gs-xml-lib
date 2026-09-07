#[cfg(test)]
mod tests {
    use crate::{parse, tokenize, Node};
    
    const XML: &str = r#"
    <root>
      <position x="10" y="20" z="30" />
    </root>
    "#;

    #[test]
    fn position_tag_test() {
        let tokens = tokenize(XML).unwrap();
        let nodes = parse(&tokens).unwrap();
        let mut position_nodes: Vec<&Node> = Vec::new();
        for n in &nodes {
            position_nodes.extend(n.find_by_tag("position"));
        }
        assert!(!position_nodes.is_empty(), "no position node found");

        if let Node::Element { ref attributes, .. } = *position_nodes[0] {
            let vec: Vec<f32> = ["x", "y", "z"]
                .iter()
                .filter_map(|&key| attributes.get(key).and_then(|v| v.parse::<f32>().ok()))
                .collect();

            assert_eq!(vec, vec![10.0, 20.0, 30.0]);
        } else {
            panic!("position node is not an element");
        }
    }
}
