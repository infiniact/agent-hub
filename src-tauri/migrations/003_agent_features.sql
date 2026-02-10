ALTER TABLE agents ADD COLUMN max_concurrency INTEGER NOT NULL DEFAULT 1;
ALTER TABLE agents ADD COLUMN available_models_json TEXT;
