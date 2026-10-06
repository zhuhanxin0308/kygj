"""严格JSON请求与结构化响应；不写文件、不启动网络服务。"""

import copy
import hashlib
import json
import platform
import sys
from pathlib import Path

import numpy
import scipy

from . import __version__
from .config import TraceConfig
from .constants import MAX_INPUT_LINE_BYTES, MAX_REQUEST_ID_LENGTH, PROTOCOL_VERSION
from .errors import EngineError, InvalidRequest
from .solver import trace_ellis


def environment() -> dict:
    """从当前进程取得真实运行版本。"""
    return {"pythonVersion": platform.python_version(), "numpyVersion": numpy.__version__,
            "scipyVersion": scipy.__version__}


def source_hash() -> str:
    """按模块相对路径排序，以路径与源码内容共同生成SHA256。"""
    root = Path(__file__).parent
    digest = hashlib.sha256()
    for source in sorted(root.rglob("*.py"), key=lambda path: path.relative_to(root).as_posix()):
        digest.update(source.relative_to(root).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(source.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def error_response(request_id: str, code: str, message: str) -> dict:
    """不回显输入全文、异常细节或堆栈。"""
    return {"protocolVersion": PROTOCOL_VERSION, "requestId": request_id, "type": "error",
            "error": {"code": code, "message": message, "details": {}}}


def handle_request(request: object) -> dict:
    """一次冻结请求对应一个响应；错误也保留合法关联标识。"""
    request_id = "unidentified"
    try:
        if not isinstance(request, dict):
            raise InvalidRequest("请求必须是JSON对象")
        candidate = request.get("requestId")
        if not isinstance(candidate, str) or not candidate.strip():
            raise InvalidRequest("requestId必须是非空字符串")
        if len(candidate) > MAX_REQUEST_ID_LENGTH:
            raise InvalidRequest("requestId不能超过128个字符")
        candidate.encode("utf-8", errors="strict")
        request_id = candidate
        version = request.get("protocolVersion")
        if type(version) is not int:
            raise InvalidRequest("protocolVersion必须是整数")
        if version != PROTOCOL_VERSION:
            raise EngineError("unsupported_version", "不支持此协议版本")
        action = request.get("action")
        if not isinstance(action, str):
            raise InvalidRequest("action必须是字符串")
        if action not in ("describe", "traceEllis"):
            raise EngineError("unsupported_action", "不支持此计算动作")
        allowed = {"protocolVersion", "requestId", "action"}
        if action == "traceEllis":
            allowed.add("config")
        if set(request) != allowed:
            raise InvalidRequest("请求字段缺失或包含未声明字段")
        common = {"protocolVersion": PROTOCOL_VERSION, "requestId": request_id}
        if action == "describe":
            return {**common, "type": "capabilities", "engineVersion": __version__,
                    "engineSourceHash": source_hash(), "actions": ["traceEllis"],
                    "environment": environment()}
        frozen = copy.deepcopy(request["config"])
        config = TraceConfig.from_dict(frozen)
        return {**common, "type": "result", "config": frozen,
                "trajectories": trace_ellis(config), "environment": environment()}
    except EngineError as error:
        return error_response(request_id, error.code, error.message)
    except (UnicodeError, RecursionError):
        return error_response(request_id, "invalid_request", "请求字符或嵌套结构无效")
    except Exception:
        # 完整异常可能包含路径或用户内容，只向stderr输出固定诊断。
        print("科研内核出现未预期错误，未登记成功结果", file=sys.stderr)
        return error_response(request_id, "internal_error", "计算内核出现内部错误")


def _unique_object(pairs):
    """严格拒绝重复键，避免解析器之间存在覆盖语义差异。"""
    result = {}
    for key, value in pairs:
        if key in result:
            raise InvalidRequest("JSON包含重复字段")
        result[key] = value
    return result


def _reject_constant(_value):
    """NaN/Infinity不属于可传递的协议数值。"""
    raise InvalidRequest("JSON包含非有限数值")


def handle_line(line: str) -> dict:
    """只接受一个JSON对象，不执行文本中的任何指令。"""
    try:
        if not isinstance(line, str) or len(line.encode("utf-8")) > MAX_INPUT_LINE_BYTES:
            raise InvalidRequest("JSON请求行不能超过1MiB")
        request = json.loads(
            line, object_pairs_hook=_unique_object, parse_constant=_reject_constant
        )
    except (ValueError, TypeError, UnicodeError, RecursionError, EngineError):
        return error_response("unidentified", "invalid_request", "无效的JSON请求")
    return handle_request(request)
