"""解析、事件、守恒和独立方位角共同约束真实八分量积分。"""

import math

import numpy as np
import pytest

from gravity_engine.protocol import handle_request


def run_ray(request_data, impact, **changes):
    """测试仅改变明确参数，不注入预制结果。"""
    request_data["config"]["impactParameters"] = [impact]
    request_data["config"].update(changes)
    result = handle_request(request_data)
    assert result["type"] == "result", result
    return result["trajectories"][0]


def test_radial_ray_matches_all_eight_components_and_events(request_data):
    """b=0的精确解检查坐标、速度、采样和事件根。"""
    ray = run_ray(request_data, 0)
    assert ray["termination"] == "through"
    assert len(ray["samples"]) == 101
    times = np.array([item["affine"] for item in ray["samples"]])
    np.testing.assert_allclose(times, np.linspace(0, 20, 101), atol=1e-8)
    for item in ray["samples"]:
        assert item["t"] == pytest.approx(item["affine"], abs=1e-8)
        assert item["l"] == pytest.approx(10 - item["affine"], abs=1e-8)
        assert item["theta"] == pytest.approx(math.pi / 2, abs=1e-12)
        assert item["phi"] == pytest.approx(0, abs=1e-12)
        assert item["kt"] == pytest.approx(1)
        assert item["kl"] == pytest.approx(-1)
        assert item["kTheta"] == pytest.approx(0, abs=1e-12)
        assert item["kPhi"] == pytest.approx(0, abs=1e-12)
    events = {item["kind"]: item for item in ray["events"]}
    assert events["throat"]["affine"] == pytest.approx(10, abs=1e-6)
    assert events["exit_negative"]["affine"] == pytest.approx(20, abs=1e-6)
    assert "return_positive" not in events
    assert ray["validation"]["status"] == "passed"
    assert ray["diagnostics"]["maxAngularMomentumError"] < 1e-8


def test_nonradial_through_ray_has_verified_azimuth(request_data):
    """实际非径向曲线必须穿喉且与独立求积吻合。"""
    ray = run_ray(request_data, 0.5)
    assert ray["termination"] == "through"
    assert ray["samples"][-1]["l"] == pytest.approx(-10, abs=1e-7)
    assert ray["samples"][-1]["phi"] > 0
    assert {e["kind"] for e in ray["events"]} == {"throat", "exit_negative"}
    assert ray["diagnostics"]["azimuthReferenceError"] < 1e-6
    assert ray["validation"]["status"] == "passed"


def test_turning_point_is_event_root_not_sample_minimum(request_data):
    """只输出两个样本仍需准确发现中途转向，避免用采样点代替事件。"""
    ray = run_ray(request_data, 2, sampleCount=2)
    assert ray["termination"] == "returned"
    assert len(ray["samples"]) == 2
    assert ray["samples"][-1]["l"] == pytest.approx(10, abs=1e-7)
    assert ray["samples"][-1]["kl"] > 0
    event = next(e for e in ray["events"] if e["kind"] == "turning")
    assert event["radius"] == pytest.approx(math.sqrt(3), abs=1e-6)
    assert event["affine"] > 0
    assert "throat" not in {e["kind"] for e in ray["events"]}
    assert ray["diagnostics"]["turningRadiusError"] < 1e-6
    assert ray["diagnostics"]["azimuthReferenceError"] < 1e-6
    assert ray["validation"]["status"] == "passed"


@pytest.mark.parametrize("impact", [1.0, -1.0])
def test_critical_ray_never_claims_completed_passage(request_data, impact):
    """临界轨道的有限预算不产生伪穿越或伪返回结论。"""
    ray = run_ray(request_data, impact)
    assert ray["termination"] == "budget_exhausted"
    assert ray["validation"]["status"] == "inconclusive"
    assert ray["samples"][-1]["affine"] == pytest.approx(40)
    assert ray["events"] == []
    assert ray["diagnostics"]["azimuthReferenceError"] is None
    critical_checks = [item for item in ray["validation"]["checks"] if "临界" in item["name"]]
    assert critical_checks
    assert any(not item["passed"] for item in critical_checks)


def test_short_critical_trajectory_has_independent_branch_checks(request_data):
    """临界分离支短程尚可信时检查通过，有限预算仍不能宣称完成。"""
    ray = run_ray(request_data, 1, maxAffineParameter=1)
    critical_checks = [item for item in ray["validation"]["checks"] if "临界" in item["name"]]
    assert len(critical_checks) == 3
    assert all(item["passed"] for item in critical_checks)
    assert ray["validation"]["status"] == "inconclusive"


@pytest.mark.parametrize("impact", [0.0, 0.5, 2.0])
def test_small_budget_retains_partial_finite_trajectory(request_data, impact):
    """预算终止保留实际已积分结果，不伪装完整运行。"""
    ray = run_ray(request_data, impact, maxAffineParameter=0.5)
    assert ray["termination"] == "budget_exhausted"
    assert ray["validation"]["status"] == "inconclusive"
    assert ray["samples"][-1]["affine"] == pytest.approx(0.5)
    assert ray["samples"][-1]["l"] > 0


@pytest.mark.parametrize("impact", [0.5, 2.0])
def test_negative_impact_preserves_angular_orientation(request_data, impact):
    """球对称性允许反向角运动，但不能把负b改成绝对值。"""
    positive = run_ray(request_data, impact)
    negative = run_ray(request_data, -impact)
    assert negative["termination"] == positive["termination"]
    for left, right in zip(positive["samples"], negative["samples"], strict=True):
        assert left["l"] == pytest.approx(right["l"], abs=1e-7)
        assert left["phi"] == pytest.approx(-right["phi"], abs=1e-7)
        assert left["kPhi"] == pytest.approx(-right["kPhi"], abs=1e-7)
    assert negative["validation"]["status"] == "passed"


def test_scale_covariance_and_uniform_sampling(request_data):
    """喉尺度改变必须缩放径向与仿射量，不能把a硬编码为1。"""
    ray = run_ray(
        request_data, 4, throatRadius=2, initialRadius=20,
        maxAffineParameter=80, sampleCount=37,
    )
    assert ray["termination"] == "returned"
    turning = next(e for e in ray["events"] if e["kind"] == "turning")
    assert turning["radius"] == pytest.approx(2 * math.sqrt(3), abs=2e-6)
    affine = np.array([item["affine"] for item in ray["samples"]])
    np.testing.assert_allclose(np.diff(affine), np.diff(affine)[0], rtol=1e-12)
    assert len(affine) == 37
    assert ray["validation"]["status"] == "passed"


def test_validation_checks_have_chinese_display_names(request_data):
    """验证表直接面向中文界面，显示名称不可泄漏内部字段键。"""
    result = handle_request(request_data)
    for ray in result["trajectories"]:
        assert ray["validation"]["checks"]
        for check in ray["validation"]["checks"]:
            assert any("\u4e00" <= character <= "\u9fff" for character in check["name"])
