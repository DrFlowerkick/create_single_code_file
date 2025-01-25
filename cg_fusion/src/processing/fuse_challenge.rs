// fuse all item required by challenge into a new binary crate in challenge tree

// Example snippet of how to place items inside a mode item
/*
let mut inline_mod = item_mod.to_owned();
let inline_items: Vec<Item> = self
    .iter_syn_item_neighbors(item_mod_index)
    .map(|(_, i)| i.to_owned())
    .collect();
inline_mod.content = Some((Brace::default(), inline_items));
if let Some(node_weight) = self.tree.node_weight_mut(item_mod_index) {
    *node_weight = NodeType::SynItem(Item::Mod(inline_mod));
} */
