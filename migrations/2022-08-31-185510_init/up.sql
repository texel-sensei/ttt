-- Your SQL goes here
CREATE TABLE frames (
	id INTEGER NOT NULL PRIMARY KEY,
	project INTEGER NOT NULL,
	start VARCHAR NOT NULL,
	end VARCHAR,
	FOREIGN KEY(project) REFERENCES projects(id)
);

CREATE TABLE projects (
	id INTEGER NOT NULL PRIMARY KEY,
	name VARCHAR NOT NULL UNIQUE,
	archived BOOLEAN NOT NULL DEFAULT 0,
	last_access_time VARCHAR NOT NULL
);