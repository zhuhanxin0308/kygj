//! 退出门控与窗口实现分离，便于验证重复关闭和失败重试。

use std::sync::atomic::{AtomicU8, Ordering};

#[repr(u8)]
enum Phase { Idle, Stopping, Approved }

pub struct ShutdownGate { phase: AtomicU8 }

impl Default for ShutdownGate {
    fn default() -> Self { Self { phase: AtomicU8::new(Phase::Idle as u8) } }
}

impl ShutdownGate {
    pub fn begin(&self) -> bool {
        self.phase.compare_exchange(Phase::Idle as u8, Phase::Stopping as u8, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }
    pub fn complete(&self, success: bool) {
        let next = if success { Phase::Approved } else { Phase::Idle };
        // 仅当前收尾任务可推进状态，重复或过期回调不覆写已批准结果。
        let _ = self.phase.compare_exchange(Phase::Stopping as u8, next as u8, Ordering::AcqRel, Ordering::Acquire);
    }
    pub fn approved(&self) -> bool { self.phase.load(Ordering::Acquire) == Phase::Approved as u8 }
}

#[cfg(test)]
mod tests {
    use super::ShutdownGate;
    use std::sync::Arc;

    #[test]
    fn successful_shutdown_is_the_only_path_to_exit_approval() {
        let gate = ShutdownGate::default();
        gate.complete(true);
        assert!(!gate.approved(), "未开始关闭时不能凭回调批准退出");
        assert!(gate.begin());
        assert!(!gate.approved());
        assert!(!gate.begin(), "重复窗口事件不能重复取消进程");
        gate.complete(true);
        assert!(gate.approved());
        assert!(!gate.begin());
    }

    #[test]
    fn failed_shutdown_keeps_the_window_and_allows_retry() {
        let gate = ShutdownGate::default();
        assert!(gate.begin());
        gate.complete(false);
        assert!(!gate.approved());
        assert!(gate.begin());
        gate.complete(true);
        gate.complete(false);
        assert!(gate.approved(), "过期失败回调不能撤销已确认的退出");
    }

    #[test]
    fn concurrent_exit_events_have_a_single_owner() {
        let gate = Arc::new(ShutdownGate::default());
        let handles: Vec<_> = (0..16).map(|_| {
            let gate = Arc::clone(&gate);
            std::thread::spawn(move || usize::from(gate.begin()))
        }).collect();
        let owners: usize = handles.into_iter().map(|handle| handle.join().unwrap()).sum();
        assert_eq!(owners, 1, "只允许一个任务收尾宿主和计算进程");
    }
}
