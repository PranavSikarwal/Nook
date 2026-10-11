import os
from unittest import mock

from pydantic_settings import SettingsConfigDict

from nook_worker.config import WorkerConfig


def test_worker_config_from_env():
    env = {
        "NOOK_BASE_URL": "http://localhost:8000/v1",
        "NOOK_API_KEY": "secret-key",
        "NOOK_MODEL": "test-model",
        "NOOK_MAX_INPUT_TOKENS": "1000000",
        "NOOK_SUMMARIZE_AT_TOKENS": "750000",
        "NOOK_DATABASE_URL": "postgresql://localhost:5432/nook",
    }
    with mock.patch.dict(os.environ, env, clear=True):
        cfg = WorkerConfig()
        assert cfg.base_url == "http://localhost:8000/v1"
        assert cfg.api_key.get_secret_value() == "secret-key"
        assert cfg.model == "test-model"
        assert cfg.max_input_tokens == 1000000
        assert cfg.summarize_at_tokens == 750000
        assert cfg.database_url == "postgresql://localhost:5432/nook"


def test_worker_config_ignores_ambient_dotenv(tmp_path, monkeypatch):
    (tmp_path / ".env").write_text("NOOK_MODEL=dotenv-model\n")
    monkeypatch.chdir(tmp_path)
    with mock.patch.dict(os.environ, {}, clear=True):
        config = WorkerConfig()
    assert config.model == ""


def test_worker_config_loads_explicit_env_file(tmp_path):
    env_file = tmp_path / ".env"
    env_file.write_text("NOOK_MODEL=dotenv-model\n")

    class EnvFileWorkerConfig(WorkerConfig):
        model_config = SettingsConfigDict(env_prefix="NOOK_", env_file=env_file)

    with mock.patch.dict(os.environ, {}, clear=True):
        config = EnvFileWorkerConfig()
    assert config.model == "dotenv-model"


def test_worker_config_defaults():
    env = {
        "NOOK_BASE_URL": "http://localhost:8000/v1",
        "NOOK_API_KEY": "secret-key",
        "NOOK_MODEL": "test-model",
    }
    with mock.patch.dict(os.environ, env, clear=True):
        cfg = WorkerConfig()
        assert cfg.max_input_tokens == 1000000
        assert cfg.summarize_at_tokens == 750000
        assert cfg.database_url == "postgresql:///nook"
