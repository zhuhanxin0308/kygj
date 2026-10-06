"""SciPy八分量积分、方向事件与终止时刻采样。"""

import numpy as np
from scipy.integrate import solve_ivp

from .config import TraceConfig
from .constants import MAX_STEP_IN_THROAT_UNITS, STATE_NAMES
from .diagnostics import build_diagnostics
from .errors import ComputationFailed
from .model import geodesic_rhs, initial_state


def make_events(initial_radius: float):
    """正侧返回只检测向外穿越，排除初始l=R造成的误终止。"""
    def throat(_affine, state):
        return state[1]

    def turning(_affine, state):
        return state[5]

    def exit_negative(_affine, state):
        return state[1] + initial_radius

    def return_positive(_affine, state):
        return state[1] - initial_radius

    throat.direction, throat.terminal = -1, False
    turning.direction, turning.terminal = 1, False
    exit_negative.direction, exit_negative.terminal = -1, True
    return_positive.direction, return_positive.terminal = 1, True
    return (throat, turning, exit_negative, return_positive)


def trace_one(config: TraceConfig, impact: float) -> dict:
    """临界轨道只给有限预算近似，不把舍入误差触发的事件当作物理结论。"""
    critical = abs(impact) == config.throat_radius
    event_functions = None if critical else make_events(config.initial_radius)
    try:
        with np.errstate(over="raise", invalid="raise", divide="raise"):
            solution = solve_ivp(
                lambda affine, state: geodesic_rhs(affine, state, config.throat_radius),
                (0.0, config.max_affine), initial_state(config, impact),
                method="DOP853", rtol=config.rtol, atol=config.atol, dense_output=True,
                events=event_functions, max_step=config.throat_radius * MAX_STEP_IN_THROAT_UNITS,
            )
    except (FloatingPointError, OverflowError, ValueError):
        # SciPy可能在返回OdeResult前发现不可表示的步长或数值，不能登记正常结果。
        raise ComputationFailed("求解器无法产生可表示的有限数值轨迹") from None
    if solution.sol is None or len(solution.t) < 2 or not solution.t[-1] > 0:
        raise ComputationFailed("求解器未产生可用的有限轨迹")
    events = []
    event_states = []
    if event_functions is not None:
        for index, kind in enumerate(("throat", "turning", "exit_negative", "return_positive")):
            for affine, state in zip(
                solution.t_events[index], solution.y_events[index], strict=True
            ):
                events.append({"kind": kind, "affine": float(affine), "radius": float(state[1])})
                event_states.append(state)
    events.sort(key=lambda event: event["affine"])
    termination = "budget_exhausted"
    if not solution.success:
        termination = "solver_failed"
    elif events and events[-1]["kind"] in ("exit_negative", "return_positive"):
        termination = "through" if events[-1]["kind"] == "exit_negative" else "returned"

    affine = np.linspace(0.0, float(solution.t[-1]), config.sample_count)
    sampled = solution.sol(affine)
    # 诊断集合包含内部已接受步与真实事件根，sampleCount=2也不能漏掉错误。
    all_times = np.concatenate((solution.t, affine, [event["affine"] for event in events]))
    state_groups = [solution.y, sampled]
    if event_states:
        state_groups.append(np.array(event_states).T)
    all_states = np.concatenate(state_groups, axis=1)
    if not np.isfinite(all_states).all() or not np.isfinite(all_times).all():
        raise ComputationFailed("求解轨迹包含非有限值")
    diagnostics, validation = build_diagnostics(
        config, impact, all_times, all_states, events, termination, float(sampled[3, -1]),
    )
    samples = [
        {"affine": float(time), **dict(zip(STATE_NAMES, map(float, state), strict=True))}
        for time, state in zip(affine, sampled.T, strict=True)
    ]
    return {"impactParameter": impact, "termination": termination, "samples": samples,
            "events": events, "diagnostics": diagnostics, "validation": validation}


def trace_ellis(config: TraceConfig) -> list[dict]:
    """输入顺序稳定，负冲量方向保留，不把重复光线擅自去重。"""
    return [trace_one(config, impact) for impact in config.impacts]
