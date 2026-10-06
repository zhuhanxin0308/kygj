"""超静态Ellis几何的完整八分量测地线方程。"""

import math

import numpy as np

from .config import TraceConfig
from .errors import ComputationFailed


def initial_state(config: TraceConfig, impact: float) -> np.ndarray:
    """生成反变切向分量；L=b并不等于kPhi。"""
    # 明确保留契约的乘加与除法顺序，和输入校验共享同一浮点定义域。
    squared_radius = (
        config.initial_radius * config.initial_radius + config.throat_radius * config.throat_radius
    )
    radial_norm = 1 - (impact * impact) / squared_radius
    return np.array([
        0.0, config.initial_radius, math.pi / 2, 0.0,
        1.0, -math.sqrt(radial_norm), 0.0, impact / squared_radius,
    ], dtype=np.float64)


def geodesic_rhs(affine: float, state: np.ndarray, throat_radius: float) -> np.ndarray:
    """从联络得到x点=k、k点=-Γkk，不以解析或预制轨迹替代积分。"""
    del affine  # 静态度规不显含仿射参数，但保持SciPy回调签名。
    _, radius, theta, _, kt, kr, ktheta, kphi = state
    with np.errstate(over="raise", divide="raise", invalid="raise"):
        try:
            radius_squared = radius * radius + throat_radius * throat_radius
            sine, cosine = np.sin(theta), np.cos(theta)
            radial_connection = radius / radius_squared
            result = np.array([
                kt, kr, ktheta, kphi,
                0.0,
                radius * (ktheta * ktheta + sine * sine * kphi * kphi),
                -2 * radial_connection * kr * ktheta + sine * cosine * kphi * kphi,
                -2 * radial_connection * kr * kphi - 2 * cosine / sine * ktheta * kphi,
            ])
        except FloatingPointError:
            raise ComputationFailed("测地线计算超出有限数值范围") from None
    if not np.isfinite(result).all():
        raise ComputationFailed("测地线右端产生非有限数值")
    return result
