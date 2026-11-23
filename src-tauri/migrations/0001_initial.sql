-- NexaTerm schema v1. Secrets are NEVER stored here: only flags saying a secret exists in
-- Windows Credential Manager (target "NexaTerm/<session-id>/<kind>").

CREATE TABLE session_groups (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    parent_id   TEXT REFERENCES session_groups(id) ON DELETE CASCADE,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);

CREATE TABLE sessions (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    group_id          TEXT REFERENCES session_groups(id) ON DELETE SET NULL,
    protocol          TEXT NOT NULL CHECK (protocol IN ('ssh','local','telnet','rdp','vnc','serial')),
    host              TEXT,
    port              INTEGER,
    username          TEXT,
    auth_method       TEXT CHECK (auth_method IN ('password','key','agent','keyboardInteractive')),
    key_path          TEXT,
    has_password      INTEGER NOT NULL DEFAULT 0,
    has_passphrase    INTEGER NOT NULL DEFAULT 0,
    jump_session_id   TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    options_json      TEXT NOT NULL DEFAULT '{}',
    favorite          INTEGER NOT NULL DEFAULT 0,
    sort_order        INTEGER NOT NULL DEFAULT 0,
    notes             TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    last_connected_at TEXT
);
CREATE INDEX idx_sessions_group ON sessions(group_id);
CREATE INDEX idx_sessions_name ON sessions(name COLLATE NOCASE);
CREATE INDEX idx_sessions_last ON sessions(last_connected_at);

CREATE TABLE session_tags (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    tag        TEXT NOT NULL,
    PRIMARY KEY (session_id, tag)
);
CREATE INDEX idx_session_tags_tag ON session_tags(tag);

CREATE TABLE known_hosts (
    host        TEXT NOT NULL,
    port        INTEGER NOT NULL,
    key_type    TEXT NOT NULL,
    public_key  TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    added_at    TEXT NOT NULL,
    PRIMARY KEY (host, port)
);

CREATE TABLE ssh_keys (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    private_path TEXT NOT NULL UNIQUE,
    key_type    TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    comment     TEXT,
    encrypted   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);

CREATE TABLE tunnel_profiles (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL CHECK (kind IN ('local','remote','dynamic')),
    bind_address TEXT NOT NULL DEFAULT '127.0.0.1',
    bind_port    INTEGER NOT NULL,
    dest_host    TEXT,
    dest_port    INTEGER,
    auto_start   INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL
);

CREATE TABLE command_snippets (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'General',
    command     TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX idx_snippets_category ON command_snippets(category);

CREATE TABLE macro_profiles (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    steps_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE application_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE connection_history (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    label        TEXT NOT NULL,
    protocol     TEXT NOT NULL,
    host         TEXT,
    connected_at TEXT NOT NULL,
    outcome      TEXT NOT NULL
);
CREATE INDEX idx_history_time ON connection_history(connected_at);

CREATE TABLE terminal_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    file_path  TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at   TEXT
);

-- Sample snippets (ตัวอย่างคำสั่งที่ใช้บ่อยสำหรับผู้ดูแลระบบ)
INSERT INTO command_snippets (id, name, category, command, description, created_at, updated_at) VALUES
 ('seed-1','Disk usage','Linux Administration','df -h','Show filesystem usage', datetime('now'), datetime('now')),
 ('seed-2','Top memory processes','Linux Administration','ps aux --sort=-%mem | head -n {{count:10}}','Largest processes by memory', datetime('now'), datetime('now')),
 ('seed-3','Service status','Service Management','systemctl status {{service}}','Status of a systemd unit', datetime('now'), datetime('now')),
 ('seed-4','Follow journal','Logs','journalctl -u {{service}} -f','Follow logs of a unit', datetime('now'), datetime('now')),
 ('seed-5','Listening ports','Network','ss -tulpn','Listening TCP/UDP sockets', datetime('now'), datetime('now')),
 ('seed-6','Docker containers','Docker','docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"','Running containers', datetime('now'), datetime('now'));
