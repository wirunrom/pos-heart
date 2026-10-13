CREATE TABLE activity_events (
    number INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at BIGINT NOT NULL,
    kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_json TEXT NOT NULL
);
CREATE INDEX activity_events_kind_number ON activity_events(kind, number DESC);
PRAGMA user_version = 14;
