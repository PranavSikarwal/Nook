import json
import logging
from typing import Any

from jsonschema.exceptions import SchemaError
from jsonschema.validators import validator_for
from langchain_core.tools import StructuredTool

from nook_worker.authorization import default_authorization_manager
from nook_worker.protocol import ToolDescriptor

logger = logging.getLogger("nook_worker.remote_tools")


def build_remote_tool(descriptor: ToolDescriptor) -> StructuredTool:
    """Builds a tool that the Daemon authorizes and runs."""
    schema = descriptor.input_schema
    validator = validator_for(schema)(schema)

    async def run_remote_tool(**kwargs: Any) -> str:
        error = next(iter(validator.iter_errors(kwargs)), None)
        if error is not None:
            return f"Error: invalid arguments. {error.message}"
        outcome = await default_authorization_manager.execute(
            descriptor.id, json.dumps(kwargs)
        )
        if outcome.status == "denied":
            reason = f": {outcome.output}" if outcome.output else ""
            return f"Error: request denied{reason}"
        return outcome.output

    return StructuredTool.from_function(
        coroutine=run_remote_tool,
        name=descriptor.name,
        description=descriptor.description,
        args_schema=schema,
    )


def build_remote_tools(descriptors: list[ToolDescriptor]) -> list[StructuredTool]:
    tools: list[StructuredTool] = []
    for descriptor in descriptors:
        try:
            tools.append(build_remote_tool(descriptor))
        except SchemaError:
            logger.warning("skipped tool %s: its schema is invalid", descriptor.id)
    return tools
