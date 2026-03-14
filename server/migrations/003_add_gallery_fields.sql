ALTER TABLE projects ADD COLUMN is_public BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE projects ADD COLUMN thumbnail_gif TEXT;

CREATE INDEX idx_projects_is_public ON projects(is_public) WHERE is_public = TRUE;
