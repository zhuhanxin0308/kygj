"""协议校验必须在任何数值执行之前拒绝非法输入。"""

import copy
import json
import math
import re

import pytest

from gravity_engine.protocol import handle_line, handle_request


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("throatRadius", 0),
        ("throatRadius", -1),
        ("throatRadius", math.nan),
        ("initialRadius", math.inf),
        ("initialRadius", 0),
        ("sampleCount", 1),
        ("sampleCount", 100001),
        ("sampleCount", True),
        ("sampleCount", 2.5),
        ("impactParameters", []),
        ("impactParameters", [0.0] * 257),
        ("impactParameters", [math.inf]),
        ("impactParameters", [math.nan]),
        ("impactParameters", [True]),
        ("impactParameters", [math.sqrt(101)]),
        ("maxAffineParameter", 0),
        ("relativeTolerance", 0),
        ("absoluteTolerance", -1),
        ("relativeTolerance", 1.0),
        ("relativeTolerance", 1e-16),
    ],
)
def test_invalid_config_returns_structured_error(request_data, field, value):
    """有限性、严格类型、模板定义域和既定资源上限均须检查。"""
    request_data["config"][field] = value
    response = handle_request(request_data)
    assert response["type"] == "error"
    assert response["error"]["code"] == "invalid_request"
    assert response["requestId"] == request_data["requestId"]
    json.dumps(response, allow_nan=False)


@pytest.mark.parametrize("location", ["root", "config"])
def test_unknown_fields_are_rejected_without_echoing_values(request_data, location):
    """拒绝未声明字段，错误响应不得原样输出秘密值。"""
    target = request_data if location == "root" else request_data["config"]
    target["extra"] = "private-token-value"
    response = handle_request(request_data)
    assert response["error"]["code"] == "invalid_request"
    assert "private-token-value" not in json.dumps(response)


@pytest.mark.parametrize(
    ("field", "value", "code"),
    [
        ("protocolVersion", 2, "unsupported_version"),
        ("protocolVersion", True, "invalid_request"),
        ("action", "unknown", "unsupported_action"),
        ("requestId", "", "invalid_request"),
        ("requestId", None, "invalid_request"),
    ],
)
def test_envelope_validation(request_data, field, value, code):
    """协议版本、动作和请求身份保持不同错误类别。"""
    request_data[field] = value
    assert handle_request(request_data)["error"]["code"] == code


@pytest.mark.parametrize("payload", [None, [], 1, "request", {}, {"action": "describe"}])
def test_non_envelopes_do_not_raise(payload):
    """非法顶层结构也必须得到可解析响应而非进程堆栈。"""
    assert handle_request(payload)["error"]["code"] == "invalid_request"


@pytest.mark.parametrize("line", ["", "not json", "{} {}", '{"a":NaN}', '{"a":Infinity}'])
def test_malformed_json_is_structured(line):
    """禁止JSON扩展的NaN/Infinity，避免跨语言解析不一致。"""
    assert handle_line(line)["error"]["code"] == "invalid_request"


def test_duplicate_json_keys_are_rejected():
    """重复键不能在校验与执行之间覆盖配置。"""
    line = '{"protocolVersion":1,"requestId":"a","action":"describe","action":"bad"}'
    assert handle_line(line)["error"]["code"] == "invalid_request"


def test_describe_reports_actual_environment_and_stable_source_identity():
    """环境预检返回真实软件身份，不执行测地线。"""
    import numpy
    import scipy

    request = {"protocolVersion": 1, "requestId": "probe", "action": "describe"}
    result = handle_request(request)
    assert result["type"] == "capabilities"
    assert result["actions"] == ["traceEllis"]
    assert result["environment"]["numpyVersion"] == numpy.__version__
    assert result["environment"]["scipyVersion"] == scipy.__version__
    assert re.fullmatch("[0-9a-f]{64}", result["engineSourceHash"])
    assert result["engineSourceHash"] == handle_request(request)["engineSourceHash"]
    request["config"] = {}
    assert handle_request(request)["error"]["code"] == "invalid_request"


def test_request_is_not_mutated_and_config_is_echoed(request_data):
    """宿主冻结输入不能被求解或解析步骤静默改写。"""
    original = copy.deepcopy(request_data)
    result = handle_line(json.dumps(request_data, ensure_ascii=False))
    assert result["type"] == "result"
    assert result["config"] == original["config"]
    assert request_data == original
    assert result["config"] is not request_data["config"]
    json.dumps(result, allow_nan=False)


def test_total_samples_budget_rejected_before_computation(request_data):
    """单条与总量都要限制，不静默减采样或丢光线。"""
    request_data["config"].update(sampleCount=50001, impactParameters=[0, 0.5])
    assert handle_request(request_data)["error"]["code"] == "invalid_request"


def test_near_tangent_initial_state_uses_contract_operation_order(request_data):
    """平方根先舍入会误拒绝此合法正根，三层必须使用契约原式。"""
    from gravity_engine.config import TraceConfig
    from gravity_engine.model import initial_state

    request_data["config"].update(
        throatRadius=0.1, initialRadius=1.0, impactParameters=[1.004987562112089]
    )
    config = TraceConfig.from_dict(request_data["config"])
    impact = config.impacts[0]
    denominator = (
        config.initial_radius * config.initial_radius + config.throat_radius * config.throat_radius
    )
    radial = 1 - (impact * impact) / denominator
    assert radial > 0
    state = initial_state(config, impact)
    assert state[5] == -math.sqrt(radial)
    assert state[7] == impact / denominator


@pytest.mark.parametrize("scale", [1e-200, math.nextafter(0.0, 1.0)])
def test_metric_denominator_underflow_is_explicit_input_error(request_data, scale):
    """有限正输入的度规分母若下溢为零，应拒绝且不启动求解。"""
    request_data["config"].update(throatRadius=scale, initialRadius=scale, impactParameters=[0])
    assert handle_request(request_data)["error"]["code"] == "invalid_request"


def test_request_id_and_line_size_limits():
    """关联ID和输入字节都有明确上限，不能只看字符数量。"""
    request = {"protocolVersion": 1, "requestId": "x" * 129, "action": "describe"}
    assert handle_request(request)["error"]["code"] == "invalid_request"
    line = json.dumps({"protocolVersion": 1, "requestId": "ok", "action": "describe"})
    assert handle_line(line + " " * (1024 * 1024))["error"]["code"] == "invalid_request"
    request["requestId"] = "x" * 128
    assert handle_request(request)["type"] == "capabilities"


def test_source_hash_is_a_hash_of_the_actual_sorted_modules():
    """重新独立计算源身份，防止稳定但伪造的常量哈希。"""
    import hashlib
    from pathlib import Path

    import gravity_engine

    root = Path(gravity_engine.__file__).parent
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*.py"), key=lambda item: item.relative_to(root).as_posix()):
        digest.update(path.relative_to(root).as_posix().encode("utf-8") + b"\0")
        digest.update(path.read_bytes() + b"\0")
    result = handle_request({"protocolVersion": 1, "requestId": "hash", "action": "describe"})
    assert result["engineSourceHash"] == digest.hexdigest()


def test_internal_error_does_not_leak_details(request_data, monkeypatch, capsys):
    """内部异常分类与stderr诊断都不得回显内部秘密。"""
    from gravity_engine import protocol

    def broken(_config):
        raise RuntimeError("credential-private-content")

    monkeypatch.setattr(protocol, "trace_ellis", broken)
    result = handle_request(request_data)
    captured = capsys.readouterr()
    assert result["error"]["code"] == "internal_error"
    assert captured.out == ""
    assert "credential-private-content" not in captured.err + json.dumps(result)
