"""验证完整方程以及真实求解失败的保留/拒绝边界。"""

import math
from types import SimpleNamespace

import numpy as np
import pytest
from scipy.integrate import IntegrationWarning

from gravity_engine import diagnostics, solver
from gravity_engine.model import geodesic_rhs
from gravity_engine.protocol import handle_request


def test_full_rhs_contains_non_equatorial_couplings():
    """离开赤道的测试点防止实现退化为四分量或平滑轨迹。"""
    state = np.array([0, 2, math.pi / 4, 0, 1, -0.5, 0.1, 0.2])
    result = geodesic_rhs(0, state, 1)
    np.testing.assert_allclose(result, [1, -0.5, 0.1, 0.2, 0, 0.06, 0.06, 0.04], atol=1e-14)


def test_failed_solver_keeps_real_partial_solution(request_data, monkeypatch):
    """用真实短积分构造失败退出，确认不会伪造剩余区间。"""
    original = solver.solve_ivp

    def fail_after_partial(fun, _span, initial, **kwargs):
        result = original(fun, (0, 0.5), initial, **kwargs)
        result.success = False
        result.status = -1
        result.message = "private solver detail"
        return result

    monkeypatch.setattr(solver, "solve_ivp", fail_after_partial)
    result = handle_request(request_data)
    assert result["type"] == "result"
    for ray in result["trajectories"]:
        assert ray["termination"] == "solver_failed"
        assert ray["validation"]["status"] == "failed"
        assert len(ray["samples"]) == request_data["config"]["sampleCount"]
        assert ray["samples"][-1]["affine"] == pytest.approx(0.5)


def test_solver_without_valid_trajectory_is_computation_error(request_data, monkeypatch):
    """没有正时间的有效轨迹，不能登记正常结果。"""
    monkeypatch.setattr(solver, "solve_ivp", lambda *a, **k: SimpleNamespace(sol=None, t=[0]))
    assert handle_request(request_data)["error"]["code"] == "computation_failed"


@pytest.mark.parametrize("error_type", [FloatingPointError, OverflowError, ValueError])
def test_solver_numerical_exception_is_computation_error(request_data, monkeypatch, error_type):
    """SciPy内部数值异常要分类为无法计算，且不泄漏原始异常内容。"""
    def fail(*_args, **_kwargs):
        raise error_type("private numerical detail")

    monkeypatch.setattr(solver, "solve_ivp", fail)
    response = handle_request(request_data)
    assert response["error"]["code"] == "computation_failed"
    assert "private numerical detail" not in response["error"]["message"]


def test_unrepresentable_step_scale_fails_without_false_success(request_data):
    """正但极小的喉尺度不能以零步长启动求解，也不能产生伪正常结果。"""
    request_data["config"].update(throatRadius=np.nextafter(0.0, 1.0).item())
    response = handle_request(request_data)
    assert response["error"]["code"] == "computation_failed"


def test_nonfinite_internal_steps_are_rejected(request_data, monkeypatch):
    """不仅序列化样本要有限，内部步与事件也不能漏检。"""
    original = solver.solve_ivp

    def nonfinite(*args, **kwargs):
        result = original(*args, **kwargs)
        result.y[0, 1] = np.inf
        return result

    monkeypatch.setattr(solver, "solve_ivp", nonfinite)
    assert handle_request(request_data)["error"]["code"] == "computation_failed"


def test_conservation_is_checked_on_internal_steps(request_data, monkeypatch):
    """输出只有两点时，中途守恒异常仍必须使验证失败。"""
    request_data["config"].update(sampleCount=2, impactParameters=[0.5])
    original = solver.solve_ivp

    def damaged_internal(*args, **kwargs):
        result = original(*args, **kwargs)
        result.y[4, 2] += 0.01
        return result

    monkeypatch.setattr(solver, "solve_ivp", damaged_internal)
    ray = handle_request(request_data)["trajectories"][0]
    assert ray["diagnostics"]["maxEnergyError"] >= 0.009
    assert ray["validation"]["status"] == "failed"


def test_independent_reference_failure_prevents_pass(request_data, monkeypatch):
    """独立参考失效不能凭主求解器成功就判通过。"""
    request_data["config"]["impactParameters"] = [0.5]

    def unavailable(*args, **kwargs):
        raise IntegrationWarning("reference not converged")

    monkeypatch.setattr(diagnostics, "quad", unavailable)
    ray = handle_request(request_data)["trajectories"][0]
    assert ray["termination"] == "through"
    assert ray["diagnostics"]["azimuthReferenceError"] is None
    assert ray["validation"]["status"] == "inconclusive"
