from pydantic import Field, SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict


class WorkerConfig(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="NOOK_",
        env_file=None,
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
