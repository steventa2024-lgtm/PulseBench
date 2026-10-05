-- PulseBench schema v1

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE providers (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    name       TEXT NOT NULL,
    base_url   TEXT NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 1,
    api_key    TEXT,
    position   INTEGER NOT NULL DEFAULT 0
);

-- One row per run. result_json is the authoritative pulsebench-result-v1 document;
-- the other columns exist for listing and querying.
CREATE TABLE benchmark_runs (
    id                 TEXT PRIMARY KEY,
    created_at         TEXT NOT NULL,
    finished_at        TEXT,
    status             TEXT NOT NULL,
    suite_id           TEXT NOT NULL,
    suite_name         TEXT NOT NULL,
    suite_version      TEXT NOT NULL,
    suite_hash         TEXT NOT NULL,
    app_version        TEXT NOT NULL,
    os                 TEXT NOT NULL,
    cpu                TEXT NOT NULL,
    gpu                TEXT,
    ram_mb             INTEGER NOT NULL,
    standard_settings  INTEGER NOT NULL,
    schema             TEXT NOT NULL,
    config_json        TEXT NOT NULL,
    result_json        TEXT NOT NULL
);

CREATE TABLE benchmark_models (
    run_id         TEXT NOT NULL REFERENCES benchmark_runs(id) ON DELETE CASCADE,
    model_key      TEXT NOT NULL,
    provider_id    TEXT NOT NULL,
    model_id       TEXT NOT NULL,
    display_name   TEXT NOT NULL,
    status         TEXT NOT NULL,
    score          INTEGER,
    pass_rate      REAL NOT NULL,
    tasks_passed   INTEGER NOT NULL,
    tasks_total    INTEGER NOT NULL,
    avg_tps        REAL,
    peak_vram_mb   INTEGER,
    total_seconds  REAL NOT NULL,
    meta_json      TEXT NOT NULL,
    PRIMARY KEY (run_id, model_key)
);

-- Catalogue of tasks seen, per suite version.
CREATE TABLE benchmark_tasks (
    suite_id      TEXT NOT NULL,
    suite_version TEXT NOT NULL,
    task_id       TEXT NOT NULL,
    title         TEXT NOT NULL,
    category      TEXT NOT NULL,
    language      TEXT NOT NULL,
    difficulty    TEXT NOT NULL,
    PRIMARY KEY (suite_id, suite_version, task_id)
);

CREATE TABLE task_results (
    run_id       TEXT NOT NULL REFERENCES benchmark_runs(id) ON DELETE CASCADE,
    model_key    TEXT NOT NULL,
    task_id      TEXT NOT NULL,
    status       TEXT NOT NULL,
    failure      TEXT,
    score        REAL,
    attempts     INTEGER NOT NULL,
    duration_ms  INTEGER NOT NULL,
    gen_ms       INTEGER,
    completion_tokens INTEGER,
    tokens_per_second REAL,
    result_json  TEXT NOT NULL,
    PRIMARY KEY (run_id, model_key, task_id)
);

CREATE TABLE telemetry_samples (
    run_id        TEXT NOT NULL REFERENCES benchmark_runs(id) ON DELETE CASCADE,
    model_key     TEXT NOT NULL,
    ts_ms         INTEGER NOT NULL,
    cpu_percent   REAL,
    ram_used_mb   INTEGER NOT NULL,
    gpu_percent   REAL,
    vram_used_mb  INTEGER,
    gpu_temp_c    REAL
);
