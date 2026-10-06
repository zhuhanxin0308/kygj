"""一次进程只读一个JSONL请求，stdout只输出一条最终协议。"""

import json
import sys

from .constants import MAX_INPUT_LINE_BYTES
from .protocol import error_response, handle_line


def main() -> int:
    """UTF-8编解码独立于Windows终端代码页；无HTTP或后台服务。"""
    try:
        raw = sys.stdin.buffer.readline(MAX_INPUT_LINE_BYTES + 1)
        if len(raw) > MAX_INPUT_LINE_BYTES:
            response = error_response("unidentified", "invalid_request", "JSON请求行不能超过1MiB")
        else:
            line = raw.decode("utf-8", errors="strict")
            response = handle_line(line)
    except UnicodeError:
        response = error_response("unidentified", "invalid_request", "请求必须为UTF-8")
    payload = json.dumps(response, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
    sys.stdout.buffer.write((payload + "\n").encode("utf-8"))
    sys.stdout.buffer.flush()
    return 0
