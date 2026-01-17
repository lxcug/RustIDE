use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

pub fn build_tree(root: &Path) -> TreeNode {
    let mut builder = NodeMap {
        name: root
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| root.to_string_lossy().to_string()),
        path: root.to_path_buf(),
        is_dir: true,
        children: BTreeMap::new(),
    };

    for entry in ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .follow_links(false)
        .build()
        .flatten()
    {
        let path = entry.path();
        if path == root {
            continue;
        }

        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let comps: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if comps.is_empty() {
            continue;
        }

        let is_dir = entry
            .file_type()
            .map(|t| t.is_dir())
            .unwrap_or_else(|| path.is_dir());
        builder.insert(&comps, path.to_path_buf(), is_dir);
    }

    builder.into_tree()
}

#[derive(Debug, Clone)]
struct NodeMap {
    name: String,
    path: PathBuf,
    is_dir: bool,
    children: BTreeMap<String, NodeMap>,
}

impl NodeMap {
    fn insert(&mut self, comps: &[String], full_path: PathBuf, is_dir: bool) {
        let mut cur = self;
        for (idx, name) in comps.iter().enumerate() {
            let at_end = idx + 1 == comps.len();
            cur = cur.children.entry(name.clone()).or_insert_with(|| NodeMap {
                name: name.clone(),
                path: cur.path.join(name),
                is_dir: true,
                children: BTreeMap::new(),
            });
            if at_end {
                cur.path = full_path.clone();
                cur.is_dir = is_dir;
            }
        }
    }

    fn into_tree(self) -> TreeNode {
        TreeNode {
            name: self.name,
            path: self.path,
            is_dir: self.is_dir,
            children: self
                .children
                .into_values()
                .map(NodeMap::into_tree)
                .collect(),
        }
    }
}
