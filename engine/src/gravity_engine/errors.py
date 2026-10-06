"""跨进程错误只暴露分类与可读信息，不外泄内部异常。"""


class EngineError(Exception):
    """可安全放入协议的明确失败。"""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code
        self.message = message


class InvalidRequest(EngineError):
    """拒绝不符合约定的输入。"""

    def __init__(self, message: str):
        super().__init__("invalid_request", message)


class ComputationFailed(EngineError):
    """未能产生合法有限结果，不能登记正常运行。"""

    def __init__(self, message: str):
        super().__init__("computation_failed", message)
