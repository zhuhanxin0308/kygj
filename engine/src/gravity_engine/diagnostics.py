"""SCI07守恒诊断、解析径向解和独立一维求积。"""

import math
import warnings

import numpy as np
from scipy.integrate import IntegrationWarning, quad

from .config import TraceConfig
from .constants import (
    ANGULAR_ERROR_LIMIT,
    AZIMUTH_ERROR_LIMIT,
    CRITICAL_BRANCH_ERROR_LIMIT,
    ENERGY_ERROR_LIMIT,
    EQUATORIAL_ERROR_LIMIT,
    INITIAL_ENERGY,
    NULL_ERROR_LIMIT,
    QUADRATURE_ERROR_LIMIT,
    QUADRATURE_RELATIVE_TOLERANCE,
    QUADRATURE_SUBDIVISION_LIMIT,
    RADIAL_ERROR_LIMIT,
    TURNING_ERROR_LIMIT,
)
from .errors import ComputationFailed


def azimuth_reference(config: TraceConfig, impact: float, termination: str) -> tuple[float, float]:
    """积分第一积分关系，与八分量ODE路径独立；返回支消除转向奇异性。"""
    beta = impact / config.throat_radius
    outer = config.initial_radius / config.throat_radius
    if impact == 0:
        return 0.0, 0.0
    if termination == "through":
        def integrand(radius):
            return beta / math.sqrt((radius * radius + 1) * (radius * radius + 1 - beta**2))
        upper = outer
    else:
        turning = math.sqrt(beta * beta - 1)
        upper = math.acosh(outer / turning)

        def integrand(coordinate):
            return beta / math.hypot(turning * math.cosh(coordinate), 1)

    # 两半轨道的误差预算相加；警告不能被静默当作参考可信。
    with warnings.catch_warnings():
        warnings.simplefilter("error", IntegrationWarning)
        value, error = quad(
            integrand, 0, upper, epsabs=QUADRATURE_ERROR_LIMIT / 2,
            epsrel=QUADRATURE_RELATIVE_TOLERANCE, limit=QUADRATURE_SUBDIVISION_LIMIT,
        )
    return 2 * value, 2 * error


def build_diagnostics(
    config: TraceConfig, impact: float, affine: np.ndarray, states: np.ndarray,
    events: list[dict], termination: str, final_phi: float,
) -> tuple[dict, dict]:
    """同时检查内部接受步、请求样本与事件点，不能只检查稀疏绘图采样。"""
    _, radius, theta, phi, energy, kr, ktheta, kphi = states
    with np.errstate(over="raise", invalid="raise", divide="raise"):
        try:
            area_squared = radius * radius + config.throat_radius**2
            angular = area_squared * np.sin(theta)**2 * kphi
            null = -energy**2 + kr**2 + area_squared * (ktheta**2 + np.sin(theta)**2 * kphi**2)
            diagnostics = {
                "maxEnergyError": float(np.max(np.abs(energy - INITIAL_ENERGY))),
                "maxAngularMomentumError": float(
                    np.max(np.abs(angular - impact)) / config.throat_radius
                ),
                "maxNullError": float(np.max(np.abs(null))),
                "maxEquatorialError": float(np.max(np.abs(theta - math.pi / 2))),
                "turningRadiusError": None,
                "radialAnalyticError": None,
                "azimuthReferenceError": None,
            }
        except FloatingPointError:
            raise ComputationFailed("守恒诊断超出有限数值范围") from None
    if any(value is not None and not math.isfinite(value) for value in diagnostics.values()):
        raise ComputationFailed("守恒诊断产生非有限数值")

    limits = {
        "maxEnergyError": ("能量守恒误差", ENERGY_ERROR_LIMIT),
        "maxAngularMomentumError": ("角动量守恒误差", ANGULAR_ERROR_LIMIT),
        "maxNullError": ("零模约束误差", NULL_ERROR_LIMIT),
        "maxEquatorialError": ("赤道面偏离", EQUATORIAL_ERROR_LIMIT),
    }
    checks = []

    def check(name, actual, threshold):
        """检查结果只写有限数值，名称和门槛一起保存。"""
        if not math.isfinite(actual):
            raise ComputationFailed("验证指标不是有限数值")
        checks.append({"name": name, "actual": float(actual), "threshold": threshold,
                       "passed": bool(actual <= threshold)})

    for field, (name, threshold) in limits.items():
        check(name, diagnostics[field], threshold)
    if abs(impact) == config.throat_radius:
        # 临界入射分离支满足kl=-l/sqrt(l²+a²)，有限仿射时间内留在正侧。
        # 守恒残差小不能证明分离支正确，数值扰动可使它错误地反弹或穿越。
        separation = float(np.max(np.abs(kr + radius / np.hypot(radius, config.throat_radius))))
        outward = float(max(0.0, np.max(kr)))
        negative_side = float(max(0.0, -np.min(radius)) / config.throat_radius)
        check("临界分离关系偏离", separation, CRITICAL_BRANCH_ERROR_LIMIT)
        check("临界入射方向偏离", outward, CRITICAL_BRANCH_ERROR_LIMIT)
        check("临界正侧位置偏离", negative_side, CRITICAL_BRANCH_ERROR_LIMIT)
    turning_events = [event for event in events if event["kind"] == "turning"]
    if turning_events and abs(impact) > config.throat_radius:
        expected = math.sqrt(impact**2 - config.throat_radius**2)
        value = (
            max(abs(event["radius"] - expected) for event in turning_events) / config.throat_radius
        )
        diagnostics["turningRadiusError"] = value
        check("解析转向半径误差", value, TURNING_ERROR_LIMIT)
    if impact == 0:
        radial_error = float(
            np.max(np.abs(radius - (config.initial_radius - affine))) / config.throat_radius
        )
        time_error = float(np.max(np.abs(states[0] - affine)) / config.throat_radius)
        diagnostics["radialAnalyticError"] = max(
            radial_error, time_error, float(np.max(np.abs(phi)))
        )
        check("径向解析解误差", diagnostics["radialAnalyticError"], RADIAL_ERROR_LIMIT)
        for event in events:
            expected_time = config.initial_radius * (1 if event["kind"] == "throat" else 2)
            check(
                "穿喉时刻误差" if event["kind"] == "throat" else "负侧出口时刻误差",
                abs(event["affine"] - expected_time) / config.throat_radius,
                RADIAL_ERROR_LIMIT,
            )

    reference_available = termination in ("through", "returned")
    if reference_available:
        try:
            reference, uncertainty = azimuth_reference(config, impact, termination)
            diagnostics["azimuthReferenceError"] = abs(final_phi - reference)
            check(
                "独立方位角参考误差", diagnostics["azimuthReferenceError"], AZIMUTH_ERROR_LIMIT
            )
            check("独立求积误差估计", uncertainty, QUADRATURE_ERROR_LIMIT)
        except (IntegrationWarning, ValueError, OverflowError, ZeroDivisionError):
            reference_available = False
            diagnostics["azimuthReferenceError"] = None
    if termination == "budget_exhausted":
        status = "inconclusive"
    elif termination == "solver_failed" or not all(item["passed"] for item in checks):
        status = "failed"
    elif not reference_available:
        status = "inconclusive"
    else:
        status = "passed"
    return diagnostics, {"status": status, "checks": checks}
