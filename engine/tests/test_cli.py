"""真实子进程确认JSONL与stdout/stderr边界，不启动服务。"""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest


def invoke(payload: bytes):
    """测试显式配置包路径；正式宿主使用已安装包和隔离启动。"""
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(Path(__file__).parents[1] / "src")
    result = subprocess.run(
        [sys.executable, "-m", "gravity_engine"], input=payload,
        capture_output=True, timeout=30, env=environment, check=False,
    )
    assert result.returncode == 0, result.stderr.decode("utf-8", errors="replace")
    lines = result.stdout.decode("utf-8").splitlines()
    assert len(lines) == 1
    return json.loads(lines[0]), result


def test_utf8_single_request_and_one_response(request_data):
    """中文ID完整返回，后续行不会变成第二个请求。"""
    request_data["requestId"] = "中文事件测试"
    payload = (json.dumps(request_data, ensure_ascii=False) + "\nnot-json\n").encode()
    response, result = invoke(payload)
    assert response["requestId"] == "中文事件测试"
    assert response["type"] == "result"
    assert b"Traceback" not in result.stdout


@pytest.mark.parametrize("payload", [
    b"\xff\n", b"", b"not-json\n", b" " * (1024 * 1024 + 1),
    b'{"protocolVersion":1,"requestId":"p","action":"describe"}' + b" " * (1024 * 1024),
], ids=["invalid-utf8", "empty", "invalid-json", "oversized-whitespace", "oversized-json"])
def test_invalid_bytes_and_bounded_lines(payload):
    """非UTF-8、空输入及过大输入必须可控退出。"""
    response, _ = invoke(payload)
    assert response["type"] == "error"
    assert response["error"]["code"] == "invalid_request"
