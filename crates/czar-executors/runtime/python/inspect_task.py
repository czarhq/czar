import dataclasses
import datetime as dt
import importlib.util
import inspect
import json
import os
import pathlib
import sys
import types
import typing
from typing import Any, Dict, Optional


def main() -> int:
    """Entry point for the script."""
    if len(sys.argv) < 3:
        usage = {"error": "Usage: inspect_python_task.py <file_path> <function_name>"}
        print(json.dumps(usage), file=sys.stderr)
        return 1

    file_path = sys.argv[1]
    func_name = sys.argv[2]

    result = inspect_task(file_path, func_name)
    print(json.dumps(result))

    if "error" in result:
        print(result["error"], file=sys.stderr)
        return 1
    return 0


def inspect_task(path: str, func_name: str) -> Dict[str, Any]:
    """Inspects a Python task file and returns its signature."""
    _ensure_importable(path)

    module = _load_module(path)
    if isinstance(module, dict):
        return module

    function = _get_function(module, func_name)
    if isinstance(function, dict):
        return function

    return _extract_signature(function)


def _ensure_importable(path: str) -> None:
    """Adds the project root to sys.path to allow imports."""
    project_root = os.path.dirname(os.path.abspath(path))
    if project_root not in sys.path:
        sys.path.insert(0, project_root)


def _load_module(path: str) -> Any:
    """Loads a Python module from the given path."""
    spec = importlib.util.spec_from_file_location("czar_task_inspect", path)
    if not spec or not spec.loader:
        return {"error": f"Could not load file {path}"}

    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        return module
    except Exception as e:
        return {"error": f"Error importing module during task analysis: {e}"}


def _get_function(module: Any, func_name: str) -> Any:
    """Retrieves a function from the loaded module."""
    fn = getattr(module, func_name, None)
    if not fn:
        return {"error": f"Function {func_name} not found"}
    return fn


def _extract_signature(fn: Any) -> Dict[str, Any]:
    """Extracts input and output types from a function signature."""
    try:
        sig = inspect.signature(fn)
    except Exception as e:
        return {"error": f"Could not get signature: {e}"}

    try:
        hints = typing.get_type_hints(fn, include_extras=True)
    except Exception:
        hints = {}

    params = list(sig.parameters.items())
    non_context_params = [
        (name, param)
        for name, param in params
        if not _is_context_parameter(name, param, len(params) - 1, hints)
    ]

    inputs = {}
    for name, param in non_context_params:
        annotation = hints.get(name, param.annotation)
        try:
            schema = _annotation_to_schema(annotation, f"argument '{name}'")
        except ValueError as e:
            return {"error": str(e)}
        if schema is None:
            return {"error": f"Argument '{name}' is untyped"}
        inputs[name] = schema

    return_annotation = hints.get("return", sig.return_annotation)
    try:
        output = _annotation_to_schema(return_annotation, "return value")
    except ValueError as e:
        return {"error": str(e)}

    return {"inputs": inputs, "output": output}


def _is_context_parameter(
    name: str,
    param: inspect.Parameter,
    input_param_count: int,
    hints: Dict[str, Any],
) -> bool:
    annotation = hints.get(name, param.annotation)
    if getattr(annotation, "__name__", None) == "Context":
        return True
    return (
        annotation is inspect._empty
        and input_param_count > 0
        and name in {"ctx", "context"}
    )


def _annotation_to_schema(annotation: Any, context: str) -> Any:
    """Converts a Python type annotation into Czar's JSON schema format."""
    if annotation is inspect._empty:
        return None

    if annotation is Any:
        return "Any"

    if annotation in (str, pathlib.Path):
        return "str"
    if annotation is int:
        return "int"
    if annotation is float:
        return "float"
    if annotation is bool:
        return "bool"
    if annotation is dt.date:
        return "date"
    if annotation is dt.datetime:
        return "datetime_tz"

    origin = typing.get_origin(annotation)
    args = typing.get_args(annotation)

    if origin in (list, typing.List):
        if len(args) != 1:
            raise ValueError(f"{context} list annotations must include exactly one item type")
        return [_annotation_to_schema(args[0], f"{context} list item")]

    if origin in (typing.Union, types.UnionType):
        non_none_args = [arg for arg in args if arg is not type(None)]
        if len(non_none_args) == 1 and len(non_none_args) != len(args):
            return _annotation_to_schema(non_none_args[0], context)
        return _annotation_text(annotation)

    if origin in (dict, typing.Dict):
        return _annotation_text(annotation)

    if _is_typed_dict_type(annotation):
        hints = typing.get_type_hints(annotation, include_extras=True)
        return {
            field_name: _annotation_to_schema(field_annotation, f"{context}.{field_name}")
            for field_name, field_annotation in hints.items()
        }

    if _is_pydantic_model(annotation):
        return _pydantic_model_schema(annotation, context)

    if dataclasses.is_dataclass(annotation):
        return {
            field.name: _annotation_to_schema(field.type, f"{context}.{field.name}")
            for field in dataclasses.fields(annotation)
        }

    if isinstance(annotation, type) and getattr(annotation, "__annotations__", None):
        hints = typing.get_type_hints(annotation, include_extras=True)
        return {
            field_name: _annotation_to_schema(field_annotation, f"{context}.{field_name}")
            for field_name, field_annotation in hints.items()
        }

    if hasattr(annotation, "__name__"):
        return annotation.__name__

    return _annotation_text(annotation)


def _is_typed_dict_type(annotation: Any) -> bool:
    return (
        isinstance(annotation, type)
        and issubclass(annotation, dict)
        and hasattr(annotation, "__annotations__")
        and hasattr(annotation, "__total__")
    )


def _is_pydantic_model(annotation: Any) -> bool:
    return isinstance(annotation, type) and (
        hasattr(annotation, "model_fields") or hasattr(annotation, "__fields__")
    )


def _pydantic_model_schema(annotation: Any, context: str) -> Dict[str, Any]:
    fields: Dict[str, Any] = {}

    if hasattr(annotation, "model_fields"):
        for name, field in annotation.model_fields.items():
            field_annotation = getattr(field, "annotation", None)
            if field_annotation is None:
                raise ValueError(f"{context}.{name} is missing a type annotation")
            fields[name] = _annotation_to_schema(field_annotation, f"{context}.{name}")
        return fields

    if hasattr(annotation, "__fields__"):
        for name, field in annotation.__fields__.items():
            field_annotation = getattr(field, "outer_type_", None) or getattr(
                field, "type_", None
            )
            if field_annotation is None:
                raise ValueError(f"{context}.{name} is missing a type annotation")
            fields[name] = _annotation_to_schema(field_annotation, f"{context}.{name}")
        return fields

    raise ValueError(f"{context} is not a supported Pydantic model")


def _annotation_text(annotation: Any) -> str:
    return str(annotation).replace("typing.", "")


if __name__ == "__main__":
    sys.exit(main())
