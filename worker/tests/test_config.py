import os
from unittest import mock

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
