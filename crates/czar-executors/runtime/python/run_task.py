from datetime import date, datetime, time
import importlib.util
import inspect
import json
import os
import re
import sys
import time as _time


_PARALLEL_SUFFIX_RE = re.compile(r"\[(\d+)\]$")


def _json_default(value):
    if isinstance(value, (datetime, date, time)):
        return value.isoformat()
    if hasattr(value, "model_dump"):
        return value.model_dump()
    if hasattr(value, "dict"):
        return value.dict()
    raise TypeError(f"Object of type {type(value).__name__} is not JSON serializable")


def _emit_progress(current, total, message=None):
    try:
        current_i = int(current)
        total_i = int(total)
    except Exception:
        return

    if total_i <= 0:
        return

    payload = {"current": current_i, "total": total_i}
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


def _env_int(name):
    raw = os.environ.get(name)
    if raw is None or raw == "":
        return None
    try:
        return int(raw)
    except Exception:
        return None


def _parallel_index_from_task_name(task_name):
    if not task_name:
        return None
    match = _PARALLEL_SUFFIX_RE.search(task_name)
    if not match:
        return None
    return int(match.group(1))


def _build_czar_tqdm_class(base_tqdm):
    class CzarTqdm(base_tqdm):
        _czar_patched = True

        def __init__(self, *args, **kwargs):
            # tqdm.__init__ may call refresh(); initialize bridge fields first.
            self._czar_last_emit_ts = 0.0
            self._czar_last_signature = None
            super().__init__(*args, **kwargs)
            self._czar_emit(force=True)

        def update(self, n=1):
            result = super().update(n)
            self._czar_emit(force=False)
            return result

        def __iter__(self):
            try:
                yield from super().__iter__()
            finally:
                # Some tqdm implementations do not guarantee close() runs before
                # user code returns. Force a final progress emission when the
                # iterator itself is exhausted so Czar sees terminal progress
                # deterministically instead of depending on timing/GC behavior.
                self._czar_emit(force=True)

        def refresh(self, *args, **kwargs):
            result = super().refresh(*args, **kwargs)
            self._czar_emit(force=False)
            return result

        def close(self):
            self._czar_emit(force=True)
            try:
                return super().close()
            except AttributeError:
                # tqdm may call close() during object finalization of a partially
                # initialized instance; ignore missing internal fields.
                return None

        def _czar_emit(self, force=False):
            total = getattr(self, "total", None)
            current = getattr(self, "n", None)
            if total is None:
                return
            try:
                total_num = float(total)
                current_num = float(current if current is not None else 0)
            except Exception:
                return
            if total_num <= 0:
                return

            current_i = int(round(max(0.0, min(current_num, total_num))))
            total_i = max(1, int(round(total_num)))
            desc = getattr(self, "desc", "")
            message = desc.strip() if isinstance(desc, str) else ""

            signature = (current_i, total_i, message)
            now = _time.monotonic()
            last_signature = getattr(self, "_czar_last_signature", None)
            last_emit_ts = float(getattr(self, "_czar_last_emit_ts", 0.0))
            if not force:
                if signature == last_signature:
                    return
                # Avoid very noisy updates from bars that refresh rapidly.
                if (now - last_emit_ts) < 0.1 and current_i < total_i:
                    return

            _emit_progress(current_i, total_i, message=message if message else None)
            self._czar_last_signature = signature
            self._czar_last_emit_ts = now

    CzarTqdm.__name__ = getattr(base_tqdm, "__name__", "tqdm")
    CzarTqdm.__qualname__ = getattr(base_tqdm, "__qualname__", CzarTqdm.__name__)
    CzarTqdm.__module__ = getattr(base_tqdm, "__module__", CzarTqdm.__module__)
    return CzarTqdm


def _install_tqdm_bridge():
    if os.environ.get("CZAR_TQDM_BRIDGE", "1").strip().lower() in {"0", "false", "off", "no"}:
        return

    try:
        import importlib
        tqdm_module = importlib.import_module("tqdm")
    except Exception:
        return

    module_names = ["tqdm", "tqdm.std", "tqdm.auto"]
    for module_name in module_names:
        try:
            module = importlib.import_module(module_name)
        except Exception:
            continue
        tqdm_cls = getattr(module, "tqdm", None)
        if not isinstance(tqdm_cls, type):
            continue
        if getattr(tqdm_cls, "_czar_patched", False):
            continue

        patched_cls = _build_czar_tqdm_class(tqdm_cls)
        setattr(module, "tqdm", patched_cls)

        def _trange(*args, __tqdm=patched_cls, **kwargs):
            return __tqdm(range(*args), **kwargs)

        setattr(module, "trange", _trange)

    # Keep top-level symbol in sync for "from tqdm import tqdm" imports.
    top_cls = getattr(tqdm_module, "tqdm", None)
    if isinstance(top_cls, type) and not getattr(top_cls, "_czar_patched", False):
        patched_top = _build_czar_tqdm_class(top_cls)
        setattr(tqdm_module, "tqdm", patched_top)

        def _trange_top(*args, __tqdm=patched_top, **kwargs):
            return __tqdm(range(*args), **kwargs)

        setattr(tqdm_module, "trange", _trange_top)


def _is_context_annotation(annotation):
    return getattr(annotation, "__name__", None) == "Context"


def _is_context_parameter(param, input_param_count):
    if _is_context_annotation(param.annotation):
        return True
    return (
        param.annotation is inspect._empty
        and input_param_count > 0
        and param.name in {"ctx", "context"}
    )


def _build_fallback_context(input_data):
    task_name = os.environ.get("CZAR_TASK_NAME")

    class _FallbackContext:
        class _Logger:
            def __call__(self, message):
                self.info(message)

            def debug(self, message):
                text = str(message)
                if text.endswith("\n"):
                    print(f"DEBUG: {text}", end="", flush=True)
                else:
                    print(f"DEBUG: {text}", flush=True)

            def info(self, message):
                text = str(message)
                if text.endswith("\n"):
                    print(text, end="", flush=True)
                else:
                    print(text, flush=True)

            def warning(self, message):
                text = str(message)
                if text.endswith("\n"):
                    print(f"WARNING: {text}", end="", flush=True)
                else:
                    print(f"WARNING: {text}", flush=True)

            def error(self, message):
                text = str(message)
                if text.endswith("\n"):
                    print(f"ERROR: {text}", end="", flush=True)
                else:
                    print(f"ERROR: {text}", flush=True)

        class _Metrics:
            def record(self, name, value, labels=None):
                _emit_metric(name, value, labels=labels)

        def __init__(self):
            self.run_id = os.environ.get("CZAR_RUN_ID")
            self.workflow_name = os.environ.get("CZAR_WORKFLOW_NAME")
            self.task_name = task_name
            self.task_id = os.environ.get("CZAR_TASK_ID")
            self.execution_name = os.environ.get("CZAR_EXECUTION_NAME")
            self.attempt = _env_int("CZAR_ATTEMPT")
            self.parallel_index = _env_int("CZAR_PARALLEL_INDEX") or _parallel_index_from_task_name(task_name)
            self.parallel_count = _env_int("CZAR_PARALLEL_COUNT")
            self.raw_input = input_data
            self.upstream = input_data.get("upstream") if isinstance(input_data, dict) else None
            self.log = self._Logger()
            self.metrics = self._Metrics()

        def progress(self, current, total, message=None):
            _emit_progress(current, total, message=message)

    return _FallbackContext()


def _build_context(input_data):
    upstream = input_data.get("upstream") if isinstance(input_data, dict) else None

    try:
        from czar import Context as CzarContext

        return CzarContext.from_env(raw_input=input_data, upstream=upstream)
    except Exception:
        return _build_fallback_context(input_data)


def main() -> int:
    _install_tqdm_bridge()

    path = os.environ.get("CZAR_TASK_FILE")
    module_name = os.environ.get("CZAR_TASK_MODULE")
    function_name = os.environ.get("CZAR_TASK_FUNCTION", "main")
    if not path and not module_name:
        print("CZAR_TASK_FILE or CZAR_TASK_MODULE not set", file=sys.stderr)
        return 1

    input_json = os.environ.get("CZAR_INPUT_JSON", "{}")
    upstream_json = os.environ.get("CZAR_UPSTREAM_JSON")
    try:
        input_data = json.loads(input_json) if input_json else {}
    except Exception as err:
        print(f"Failed to parse CZAR_INPUT_JSON: {err}", file=sys.stderr)
        return 1

    if upstream_json:
        try:
            upstream_data = json.loads(upstream_json)
            if isinstance(input_data, dict) and "upstream" not in input_data:
                input_data["upstream"] = upstream_data
        except Exception as err:
            print(f"Failed to parse CZAR_UPSTREAM_JSON: {err}", file=sys.stderr)

    if path:
        spec = importlib.util.spec_from_file_location("czar_task", path)
        if spec is None or spec.loader is None:
            print(f"Failed to load task file: {path}", file=sys.stderr)
            return 1
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    else:
        try:
            module = importlib.import_module(module_name)
        except Exception as err:
            print(f"Failed to import module {module_name}: {err}", file=sys.stderr)
            return 1

    fn = getattr(module, function_name, None)
    if fn is None:
        print(f"Task function not found: {function_name}", file=sys.stderr)
        return 1

    sig = inspect.signature(fn)
    args = []
    kwargs = {}
    all_params = list(sig.parameters.values())
    context_params = []
    input_params = []
    for param in all_params:
        if _is_context_parameter(param, len(all_params) - 1):
            context_params.append(param)
        else:
            input_params.append(param)

    if context_params:
        ctx = _build_context(input_data)
        for param in context_params:
            kwargs[param.name] = ctx

    # improved argument injection
    if isinstance(input_data, dict):
        # Check if we should unpack kwargs directly
        # Criterion: Function has > 1 params, OR the single param's name exists in input data
        should_unpack = False

        if len(input_params) > 1:
            should_unpack = True
        elif len(input_params) == 1:
            param_name = input_params[0].name
            if param_name in input_data:
                should_unpack = True

        if should_unpack:
            for param in input_params:
                if param.name in input_data:
                    kwargs[param.name] = input_data[param.name]
        else:
            # Fallback to single-argument model injection (legacy support)
            if len(input_params) > 0:
                param = input_params[0]
                ann = param.annotation
                if ann is not inspect._empty:
                    try:
                        input_obj = ann(**input_data)
                    except Exception:
                        input_obj = input_data
                else:
                    input_obj = input_data
                args.append(input_obj)

    # Handle non-dict input (must be legacy single arg)
    elif len(input_params) > 0:
        param = input_params[0]
        ann = param.annotation
        if ann is not inspect._empty:
             try:
                 input_obj = ann(input_data)
             except Exception:
                 input_obj = input_data
        else:
             input_obj = input_data
        args.append(input_obj)

    result = fn(*args, **kwargs)

    if result is None:
        return 0

    if hasattr(result, "model_dump"):
        result = result.model_dump()
    elif hasattr(result, "dict"):
        result = result.dict()

    # Prefix output with magic string to distinguish from debug prints
    print(f"CZAR_OUTPUT:{json.dumps(result, default=_json_default)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
