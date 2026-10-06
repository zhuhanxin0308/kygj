-- 迁移用固定v1兼容性夹具，保留本批之前已发布给本地验收的数据库约束。
CREATE TABLE projects(id TEXT PRIMARY KEY,record_json TEXT NOT NULL);
CREATE TABLE models(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),label TEXT NOT NULL,created_at TEXT NOT NULL,record_json TEXT NOT NULL);
CREATE TABLE preflights(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),record_json TEXT NOT NULL,consumed INTEGER NOT NULL DEFAULT 0 CHECK(consumed IN (0,1)));
CREATE TABLE runs(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),model_id TEXT NOT NULL REFERENCES models(id),created_at TEXT NOT NULL,state TEXT NOT NULL,request_json TEXT NOT NULL,environment_json TEXT NOT NULL,record_json TEXT NOT NULL);
CREATE TRIGGER models_no_update BEFORE UPDATE ON models BEGIN SELECT RAISE(ABORT,'immutable model'); END;
CREATE TRIGGER models_no_delete BEFORE DELETE ON models BEGIN SELECT RAISE(ABORT,'immutable model'); END;
CREATE TRIGGER runs_no_delete BEFORE DELETE ON runs BEGIN SELECT RAISE(ABORT,'immutable run'); END;
CREATE TRIGGER runs_frozen BEFORE UPDATE ON runs WHEN OLD.id!=NEW.id OR OLD.project_id!=NEW.project_id OR OLD.model_id!=NEW.model_id OR OLD.created_at!=NEW.created_at OR OLD.request_json!=NEW.request_json OR OLD.environment_json!=NEW.environment_json OR json_extract(OLD.record_json,'$.request')!=json_extract(NEW.record_json,'$.request') OR json_extract(OLD.record_json,'$.environment')!=json_extract(NEW.record_json,'$.environment') BEGIN SELECT RAISE(ABORT,'immutable snapshot'); END;
CREATE TRIGGER runs_transitions BEFORE UPDATE ON runs WHEN NOT ((OLD.state='queued' AND NEW.state IN ('running','failed','cancelling','unknown')) OR (OLD.state='running' AND NEW.state IN ('completed','failed','cancelling','unknown')) OR (OLD.state='cancelling' AND NEW.state IN ('cancelled','unknown'))) BEGIN SELECT RAISE(ABORT,'invalid transition'); END;
CREATE TRIGGER preflight_frozen BEFORE UPDATE ON preflights WHEN OLD.id!=NEW.id OR OLD.project_id!=NEW.project_id OR OLD.record_json!=NEW.record_json OR OLD.consumed!=0 OR NEW.consumed!=1 BEGIN SELECT RAISE(ABORT,'immutable preflight'); END;
PRAGMA application_id=1196578626;
PRAGMA user_version=1;
