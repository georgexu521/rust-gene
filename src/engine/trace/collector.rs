use super::{TraceEvent, TurnStatus, TurnTrace, DEFAULT_MAX_TRACES};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone)]
pub struct TraceCollector {
    inner: Arc<Mutex<TurnTrace>>,
}

impl TraceCollector {
    pub fn new(trace: TurnTrace) -> Self {
        Self {
            inner: Arc::new(Mutex::new(trace)),
        }
    }

    pub fn record(&self, event: TraceEvent) {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.events.push(event);
    }

    pub fn snapshot(&self) -> TurnTrace {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn finish(&self, status: TurnStatus) -> TurnTrace {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.finish(status);
        trace.clone()
    }
}

#[derive(Debug)]
pub struct TraceStore {
    max_traces: usize,
    traces: RwLock<VecDeque<TurnTrace>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TRACES)
    }
}

impl TraceStore {
    pub fn new(max_traces: usize) -> Self {
        Self {
            max_traces: max_traces.max(1),
            traces: RwLock::new(VecDeque::new()),
        }
    }

    pub fn push(&self, trace: TurnTrace) {
        let mut traces = self.traces.write().unwrap_or_else(|e| e.into_inner());
        traces.push_back(trace);
        while traces.len() > self.max_traces {
            traces.pop_front();
        }
    }

    pub fn latest(&self) -> Option<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .back()
            .cloned()
    }

    pub fn recent(&self, limit: usize) -> Vec<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.traces.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
