#![allow(dead_code)]
pub mod selector;
pub mod serialize;
pub mod tree;
pub mod tree_sink;

pub use tree::{AttachShadowError, DomTree, NodeData, NodeId, ShadowRootMode};
pub use tree_sink::{parse_fragment, parse_fragment_with_context, parse_html};

#[cfg(test)]
mod tests;
