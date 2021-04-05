struct TrieMapNode<T> {
    label: u8,
    value: Option<T>,
    children: Vec<TrieMapNode<T>>,
}

impl<T> TrieMapNode<T> {
    fn find_child(&self, label: u8) -> Option<usize> {
        for (idx, child) in self.children.iter().enumerate() {
            if child.label == label {
                return Some(idx);
            }
        }
        return None;
    }

    fn traverse_trie_mut<'a, 'b>(
        &'a mut self,
        prefix: &'b [u8],
    ) -> (&'a mut TrieMapNode<T>, &'b [u8]) {
        if prefix.len() == 0 {
            return (self, prefix);
        }

        if let Some(idx) = self.find_child(prefix[0]) {
            self.children[idx].traverse_trie_mut(&prefix[1..])
        } else {
            (self, prefix)
        }
    }

    fn traverse_trie_for_value<'a, 'b>(
        &'a self,
        prefix: &'b [u8],
        mut last_value: Option<&'a T>,
    ) -> (&'a TrieMapNode<T>, Option<&'a T>, &'b [u8]) {
        if self.value.is_some() {
            last_value = self.value.as_ref();
        }

        if prefix.len() == 0 {
            return (self, last_value, prefix);
        }

        if let Some(idx) = self.find_child(prefix[0]) {
            self.children[idx].traverse_trie_for_value(&prefix[1..], last_value)
        } else {
            (self, last_value, prefix)
        }
    }
}

// A Map implemented with a trie, so that when a (K, V) pair is
// inserted into the map, any key whose prefix matches K will also
// be mapped to V.
// The prefix match is greedy, i.e. if multiple key prefixes match
// one key, then the mapped value is the value of the longest prefix
pub struct TrieMap<T> {
    root: TrieMapNode<T>,
}

impl<T> TrieMap<T> {
    pub fn new() -> TrieMap<T> {
        TrieMap {
            root: TrieMapNode {
                label: 0,
                value: None,
                children: Vec::new(),
            },
        }
    }

    pub fn put_prefix(&mut self, prefix: impl AsRef<[u8]>, value: impl Into<T>) {
        let (mut node, remaining_prefix) = self.root.traverse_trie_mut(prefix.as_ref());
        for b in remaining_prefix {
            let new_node = TrieMapNode {
                label: *b,
                value: None,
                children: Vec::new(),
            };
            node.children.push(new_node);
            node = node.children.last_mut().unwrap();
        }
        node.value = Some(value.into());
    }

    pub fn get_prefix(&self, prefix: impl AsRef<[u8]>) -> Option<&T> {
        let (_, value, _) = self.root.traverse_trie_for_value(prefix.as_ref(), None);
        value
    }
}
