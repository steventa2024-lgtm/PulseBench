CREATE INDEX idx_runs_created ON benchmark_runs(created_at DESC);
CREATE INDEX idx_models_key ON benchmark_models(model_key, run_id);
CREATE INDEX idx_task_results_run ON task_results(run_id, model_key);
CREATE INDEX idx_telemetry_run ON telemetry_samples(run_id, model_key, ts_ms);
