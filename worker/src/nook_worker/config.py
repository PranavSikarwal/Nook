import sys
from pathlib import Path

from pydantic import Field, SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict

_ROOT_DIR = Path(__file__).resolve().parents[3]
_ENV_FILE = _ROOT_DIR / ".env"
_SOURCE_ENV_FILE = (
    _ENV_FILE
    if not getattr(sys, "frozen", False) and _ENV_FILE.is_file()
    else None
)


class WorkerConfig(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="NOOK_",
        env_file=_SOURCE_ENV_FILE,
        env_file_encoding="utf-8",
        extra="ignore",
    )

    base_url: str = Field(
        default="",
        description="Model endpoint base URL",
    )
    api_key: SecretStr = Field(
        default=SecretStr(""),
        description="API key for model endpoint",
    )
    model: str = Field(
        default="",
        description="Model name",
    )
    max_input_tokens: int = Field(
        default=1_000_000,
        description="Maximum input tokens for model profile",
    )
    summarize_at_tokens: int = Field(
        default=750_000,
        description="Token threshold to trigger summarization",
    )
    database_url: str = Field(
        default="postgresql:///nook",
        description="Postgres connection string",
    )
