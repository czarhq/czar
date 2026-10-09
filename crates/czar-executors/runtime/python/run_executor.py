from datetime import date, datetime, time
import importlib
import inspect
import json
import os
import sys


def _json_default(value):
    if isinstance(value, (datetime, date, time)):
        return value.isoformat()
    if hasattr(value, "model_dump"):
        return value.model_dump()
    if hasattr(value, "dict"):
        return value.dict()
    if hasattr(value, "__dict__"):
        return value.__dict__
    raise TypeError(f"Object of type {type(value).__name__} is not JSON serializable")


def _emit_progress(current, total, message=None):
    payload = {"current": int(current), "total": int(total)}
    if isinstance(message, str) and message.strip():
        payload["message"] = message.strip()
    sys.stdout.write(f"CZAR_PROGRESS:{json.dumps(payload, default=_json_default)}\n")
    sys.stdout.flush()


def _emit_metric(name, value, labels=None):
    payload = {"name": str(name), "value": value}
    if labels:
        payload["labels"] = labels
    sys.stdout.write(f"CZAR_METRIC:{json.dumps(payload, default=_json_default)}\n")
    sys.stdout.flush()


class _FallbackContext:
    class _Logger:
        def __call__(self, message):
            self.info(message)

        def debug(self, message):
            print(f"DEBUG: {message}", flush=True)

        def info(self, message):
            print(message, flush=True)

        def warning(self, message):
            print(f"WARNING: {message}", flush=True)

        def error(self, message):
            print(f"ERROR: {message}", flush=True)

    class _Metrics:
        def record(self, name, value, labels=None):
            _emit_metric(name, value, labels=labels)

    def __init__(self, raw_input, upstream):
        self.run_id = os.environ.get("CZAR_RUN_ID")
        self.workflow_name = os.environ.get("CZAR_WORKFLOW_NAME")
        self.task_name = os.environ.get("CZAR_TASK_NAME")
        self.task_id = os.environ.get("CZAR_TASK_ID")
        self.execution_name = os.environ.get("CZAR_EXECUTION_NAME")
        self.raw_input = raw_input
        self.upstream = upstream
        self.log = self._Logger()
        self.metrics = self._Metrics()

    def progress(self, current, total, message=None):
        _emit_progress(current, total, message=message)


def _build_context(input_data):
    upstream = input_data.get("upstream") if isinstance(input_data, dict) else None
    try:
        from czar import Context

        return Context.from_env(raw_input=input_data, upstream=upstream)
    except Exception:
        return _FallbackContext(input_data, upstream)


def _load_input():
    input_json = os.environ.get("CZAR_INPUT_JSON", "{}")
    data = json.loads(input_json) if input_json else {}
    upstream_json = os.environ.get("CZAR_UPSTREAM_JSON")
    if upstream_json and isinstance(data, dict) and "upstream" not in data:
        data["upstream"] = json.loads(upstream_json)
    return data


def _load_executor_spec():
    raw = os.environ.get("CZAR_CUSTOM_EXECUTOR_JSON")
    if not raw:
        raise RuntimeError("CZAR_CUSTOM_EXECUTOR_JSON not set")
    spec = json.loads(raw)
    target = spec.get("target") or spec.get("type")
    if not isinstance(target, str) or ":" not in target:
        raise RuntimeError("custom executor target must be 'module:ClassName'")
    spec["target"] = target
    return spec


def _instantiate_executor(spec):
    module_name, class_name = spec["target"].split(":", 1)
    module = importlib.import_module(module_name)
    cls = getattr(module, class_name, None)
    if cls is None:
        raise RuntimeError(f"custom executor class not found: {spec['target']}")

    config = spec.get("config")
    if config is None:
        config = {}
    try:
        return cls(config)
    except TypeError:
        if config in ({}, None):
            return cls()
        raise


def _invoke_run(instance, input_data, ctx):
    run_method = getattr(instance, "run", None)
    if run_method is None:
        raise RuntimeError(f"{type(instance).__name__} does not define run(...)")

    sig = inspect.signature(run_method)
    kwargs = {}
    config_file = os.environ.get("CZAR_TASK_CONFIG_FILE")
    upstream = input_data.get("upstream") if isinstance(input_data, dict) else None

    for name, _param in sig.parameters.items():
        if name in {"config_file", "config_path"}:
            kwargs[name] = config_file
        elif name in {"ctx", "context"}:
            kwargs[name] = ctx
        elif name == "upstream":
            kwargs[name] = upstream or {}
        elif name in {"task_input", "input", "params", "payload"}:
            kwargs[name] = input_data

    return run_method(**kwargs)


def main():
    try:
        input_data = _load_input()
        ctx = _build_context(input_data)
        spec = _load_executor_spec()
        instance = _instantiate_executor(spec)
        result = _invoke_run(instance, input_data, ctx)
        print(f"CZAR_OUTPUT:{json.dumps(result, default=_json_default)}")
        return 0
    except Exception as err:
        print(f"{type(err).__name__}: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
