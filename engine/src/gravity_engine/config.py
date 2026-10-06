"""输入校验和冻结数值配置；所有字段显式声明。"""

import math
from dataclasses import dataclass

from .constants import (
    MAX_RAY_COUNT,
    MAX_SAMPLE_COUNT,
    MAX_TOTAL_SAMPLE_COUNT,
    MIN_RELATIVE_TOLERANCE,
    MIN_SAMPLE_COUNT,
)
from .errors import InvalidRequest

CONFIG_FIELDS = frozenset({
    "throatRadius", "initialRadius", "impactParameters", "maxAffineParameter",
    "sampleCount", "relativeTolerance", "absoluteTolerance",
})


def finite_number(value: object, name: str, *, positive: bool = False) -> float:
    """bool不是数值输入，过大的整数也不得溢出到进程异常。"""
    if type(value) not in (int, float):
        raise InvalidRequest(f"{name}必须是有限数值")
    try:
        number = float(value)
    except (OverflowError, ValueError):
        raise InvalidRequest(f"{name}超出可表示数值范围") from None
    if not math.isfinite(number) or (positive and number <= 0):
        raise InvalidRequest(f"{name}必须是{'严格正的' if positive else ''}有限数值")
    return number


@dataclass(frozen=True)
class TraceConfig:
    """内部只读配置，响应仍回传原始JSON数值而不是改写配置。"""

    throat_radius: float
    initial_radius: float
    impacts: tuple[float, ...]
    max_affine: float
    sample_count: int
    rtol: float
    atol: float

    @classmethod
    def from_dict(cls, value: object) -> "TraceConfig":
        """完整配置必填；多余键与缺失键都拒绝。"""
        if not isinstance(value, dict) or set(value) != CONFIG_FIELDS:
            raise InvalidRequest("config字段缺失或包含未声明字段")
        a = finite_number(value["throatRadius"], "throatRadius", positive=True)
        radius = finite_number(value["initialRadius"], "initialRadius", positive=True)
        budget = finite_number(value["maxAffineParameter"], "maxAffineParameter", positive=True)
        rtol = finite_number(value["relativeTolerance"], "relativeTolerance", positive=True)
        atol = finite_number(value["absoluteTolerance"], "absoluteTolerance", positive=True)
        if not MIN_RELATIVE_TOLERANCE <= rtol < 1:
            raise InvalidRequest("relativeTolerance须不低于100倍双精度eps且严格小于1")
        count = value["sampleCount"]
        if type(count) is not int or not MIN_SAMPLE_COUNT <= count <= MAX_SAMPLE_COUNT:
            raise InvalidRequest("sampleCount必须为2至100000的整数")
        impacts = value["impactParameters"]
        if not isinstance(impacts, list) or not 1 <= len(impacts) <= MAX_RAY_COUNT:
            raise InvalidRequest("impactParameters必须包含1至256个数值")
        if count * len(impacts) > MAX_TOTAL_SAMPLE_COUNT:
            raise InvalidRequest("每批总输出采样点数不能超过100000")
        parsed = tuple(finite_number(item, "impactParameter") for item in impacts)
        squared_radius = radius * radius + a * a
        if not math.isfinite(squared_radius) or squared_radius <= 0:
            raise InvalidRequest("径向尺度超出双精度度规可表示范围")
        for impact in parsed:
            # 与宿主和初态构造采用相同顺序，先开方再比较会误拒绝接近切向的合法输入。
            impact_squared = impact * impact
            radial_norm = 1 - impact_squared / squared_radius
            angular_velocity = impact / squared_radius
            if not all(math.isfinite(item) for item in (
                impact_squared, radial_norm, angular_velocity
            )) or radial_norm <= 0:
                raise InvalidRequest("初始径向切向量的根号内必须严格为正")
        return cls(a, radius, parsed, budget, count, rtol, atol)
