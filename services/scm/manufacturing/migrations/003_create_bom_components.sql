CREATE TABLE bom_components (id TEXT PRIMARY KEY, bom_id TEXT NOT NULL REFERENCES boms(id), component_item_id TEXT NOT NULL, quantity_required INTEGER NOT NULL);
