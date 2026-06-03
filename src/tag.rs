#[derive(Debug, Clone)]
pub struct TaggedNode {
    pub node: String,
    pub source: String,
}

impl TaggedNode {
    pub fn new(node: String, source: impl Into<String>) -> Self {
        Self {
            node,
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaggedChunk {
    pub body: String,
    pub source: String,
}

impl TaggedChunk {
    pub fn new(body: String, source: impl Into<String>) -> Self {
        Self {
            body,
            source: source.into(),
        }
    }
}

pub fn nodes_only(tagged: &[TaggedNode]) -> Vec<String> {
    tagged.iter().map(|t| t.node.clone()).collect()
}
