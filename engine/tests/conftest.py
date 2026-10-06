"""各测试共享公开SCI07输入，每次测试取得独立配置。"""

import pytest


@pytest.fixture
def request_data():
    """冻结结构来自engine-v1，不把夹具作为生产输出。"""
    return {
        "protocolVersion": 1,
        "requestId": "sci07-test",
        "action": "traceEllis",
        "config": {
            "throatRadius": 1.0,
            "initialRadius": 10.0,
            "impactParameters": [0.0, 0.5, 2.0],
            "maxAffineParameter": 40.0,
            "sampleCount": 101,
            "relativeTolerance": 1e-10,
            "absoluteTolerance": 1e-12,
        },
    }
