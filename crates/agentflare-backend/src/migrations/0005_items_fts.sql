CREATE VIRTUAL TABLE IF NOT EXISTS items_fts
  USING fts5(name, description, metadata,
    content='items', content_rowid='rowid',
    tokenize='porter unicode61');

CREATE TRIGGER IF NOT EXISTS items_fts_ai AFTER INSERT ON items BEGIN
  INSERT INTO items_fts(rowid, name, description, metadata)
  VALUES (new.rowid, new.name, new.description, new.metadata);
END;

CREATE TRIGGER IF NOT EXISTS items_fts_ad AFTER DELETE ON items BEGIN
  INSERT INTO items_fts(items_fts, rowid, name, description, metadata)
  VALUES('delete', old.rowid, old.name, old.description, old.metadata);
END;

CREATE TRIGGER IF NOT EXISTS items_fts_au AFTER UPDATE ON items BEGIN
  INSERT INTO items_fts(items_fts, rowid, name, description, metadata)
  VALUES('delete', old.rowid, old.name, old.description, old.metadata);
  INSERT INTO items_fts(rowid, name, description, metadata)
  VALUES (new.rowid, new.name, new.description, new.metadata);
END;
