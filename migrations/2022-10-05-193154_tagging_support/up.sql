-- Your SQL goes here


CREATE TABLE tags (
	id INTEGER NOT NULL PRIMARY KEY,
	name VARCHAR NOT NULL,
	archived BOOLEAN NOT NULL DEFAULT 0,
	last_access_time VARCHAR NOT NULL
);


CREATE TABLE tags_per_project(
	project_id INTEGER NOT NULL,
	tag_id INTEGER NOT NULL,
	FOREIGN KEY(project_id) REFERENCES projects(id),
	FOREIGN KEY(tag_id) REFERENCES tags(id),
	PRIMARY KEY(project_id, tag_id)
);
