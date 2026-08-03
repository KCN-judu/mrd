//! Deterministic, bounded execution for independent verification tasks.

use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The execution policy for one ordered batch of independent components.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ComponentExecutionPolicy {
    /// Evaluate each component in input order on the calling thread.
    Sequential,
    /// Evaluate independent components concurrently, then restore input order.
    DeterministicParallel {
        /// The fixed upper bound on active component computations.
        worker_count: NonZeroUsize,
    },
}

impl ComponentExecutionPolicy {
    /// Builds a policy from the explicit CLI worker count.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionPolicyError::ZeroWorkers`] when `worker_count` is zero.
    pub fn from_component_workers(worker_count: usize) -> Result<Self, ExecutionPolicyError> {
        let worker_count =
            NonZeroUsize::new(worker_count).ok_or(ExecutionPolicyError::ZeroWorkers)?;
        Ok(if worker_count.get() == 1 {
            Self::Sequential
        } else {
            Self::DeterministicParallel { worker_count }
        })
    }

    #[must_use]
    pub const fn worker_limit(self) -> usize {
        match self {
            Self::Sequential => 1,
            Self::DeterministicParallel { worker_count } => worker_count.get(),
        }
    }
}

/// A rejected component execution policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ExecutionPolicyError {
    #[error("component worker count must be positive")]
    ZeroWorkers,
}

/// Ordered batch results and bounded scheduler counters.
#[derive(Debug)]
pub struct OrderedMap<T> {
    /// Outputs in the same order as their inputs.
    pub values: Vec<T>,
    /// The maximum number of submitted tasks not yet received by the coordinator.
    pub max_in_flight: usize,
    /// The maximum number of completed results awaiting an earlier input result.
    pub max_reorder_buffered: usize,
}

/// Applies an independent fallible operation under a deterministic execution policy.
///
/// The parallel variant keeps at most `worker_count` submitted tasks and at
/// most `worker_count` completed out-of-order values beyond the returned
/// output vector. Outputs and failures are observed in input order.
///
/// # Errors
///
/// Returns the first operation failure in input order after every already
/// submitted task has finished. It never starts a later task once that failure
/// becomes observable in input order.
pub fn ordered_map<Inputs, Input, Output, Failure, Operation>(
    inputs: Inputs,
    policy: ComponentExecutionPolicy,
    operation: Operation,
) -> Result<OrderedMap<Output>, Failure>
where
    Inputs: IntoIterator<Item = Input>,
    Input: Send,
    Output: Send,
    Failure: Send,
    Operation: Fn(Input) -> Result<Output, Failure> + Sync,
{
    match policy {
        ComponentExecutionPolicy::Sequential => ordered_map_sequential(inputs, operation),
        ComponentExecutionPolicy::DeterministicParallel { worker_count } => {
            ordered_map_parallel(inputs, worker_count, operation)
        }
    }
}

fn ordered_map_sequential<Inputs, Input, Output, Failure, Operation>(
    inputs: Inputs,
    operation: Operation,
) -> Result<OrderedMap<Output>, Failure>
where
    Inputs: IntoIterator<Item = Input>,
    Operation: Fn(Input) -> Result<Output, Failure>,
{
    let mut values = Vec::new();
    let mut max_in_flight = 0;
    for input in inputs {
        max_in_flight = 1;
        values.push(operation(input)?);
    }
    Ok(OrderedMap {
        values,
        max_in_flight,
        max_reorder_buffered: 0,
    })
}

fn ordered_map_parallel<Inputs, Input, Output, Failure, Operation>(
    inputs: Inputs,
    worker_count: NonZeroUsize,
    operation: Operation,
) -> Result<OrderedMap<Output>, Failure>
where
    Inputs: IntoIterator<Item = Input>,
    Input: Send,
    Output: Send,
    Failure: Send,
    Operation: Fn(Input) -> Result<Output, Failure> + Sync,
{
    let worker_count = worker_count.get();
    thread::scope(|scope| {
        let (task_sender, task_receiver) = mpsc::sync_channel(worker_count);
        let (result_sender, result_receiver) = mpsc::sync_channel(worker_count);
        let task_receiver = Arc::new(Mutex::new(task_receiver));
        let operation = &operation;

        for _ in 0..worker_count {
            let task_receiver = Arc::clone(&task_receiver);
            let result_sender = result_sender.clone();
            scope.spawn(move || {
                loop {
                    let task = match task_receiver.lock() {
                        Ok(receiver) => receiver.recv(),
                        Err(_) => return,
                    };
                    let Ok((index, input)) = task else {
                        return;
                    };
                    if result_sender.send((index, operation(input))).is_err() {
                        return;
                    }
                }
            });
        }
        drop(result_sender);

        let mut inputs = inputs.into_iter().enumerate();
        let mut task_sender = Some(task_sender);
        let mut submitted = 0;
        let mut in_flight = 0;
        let mut max_in_flight = 0;
        for _ in 0..worker_count {
            let Some(input) = inputs.next() else {
                break;
            };
            task_sender
                .as_ref()
                .expect("task sender exists while submitting initial work")
                .send(input)
                .expect("workers remain alive while the scope owns the result receiver");
            submitted += 1;
            in_flight += 1;
            max_in_flight = max_in_flight.max(in_flight);
        }

        let mut values = Vec::new();
        let mut pending = BTreeMap::new();
        let mut next_input = 0;
        let mut first_failure = None;
        let mut max_reorder_buffered = 0;

        while in_flight != 0 {
            let (index, result) = result_receiver
                .recv()
                .expect("submitted task must send one result while the scope is active");
            in_flight -= 1;
            pending.insert(index, result);

            while let Some(result) = pending.remove(&next_input) {
                next_input += 1;
                if first_failure.is_none() {
                    match result {
                        Ok(value) => values.push(value),
                        Err(error) => {
                            first_failure = Some(error);
                            task_sender.take();
                        }
                    }
                }
            }
            max_reorder_buffered = max_reorder_buffered.max(pending.len());

            if first_failure.is_none() {
                if let Some(input) = inputs.next() {
                    task_sender
                        .as_ref()
                        .expect("task sender exists until input is exhausted")
                        .send(input)
                        .expect("workers remain alive while the scope owns the result receiver");
                    submitted += 1;
                    in_flight += 1;
                    max_in_flight = max_in_flight.max(in_flight);
                } else {
                    task_sender.take();
                }
            }
        }

        debug_assert_eq!(submitted, next_input);
        if let Some(error) = first_failure {
            Err(error)
        } else {
            Ok(OrderedMap {
                values,
                max_in_flight,
                max_reorder_buffered,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::{ComponentExecutionPolicy, ExecutionPolicyError, ordered_map};

    #[test]
    fn ordered_parallel_execution_restores_input_order_with_fixed_bounds() {
        let policy = ComponentExecutionPolicy::from_component_workers(3).unwrap();
        let report = ordered_map(0..9, policy, |input| {
            thread::sleep(Duration::from_millis(u64::try_from(9 - input).unwrap()));
            Ok::<_, ()>(input * 2)
        })
        .unwrap();

        assert_eq!(
            report.values,
            (0..9).map(|input| input * 2).collect::<Vec<_>>()
        );
        assert_eq!(report.max_in_flight, 3);
        assert!(report.max_reorder_buffered <= 3);
    }

    #[test]
    fn ordered_parallel_execution_reports_the_earliest_input_failure() {
        let policy = ComponentExecutionPolicy::from_component_workers(2).unwrap();
        let failure = ordered_map(0..4, policy, |input| {
            if input == 0 {
                thread::sleep(Duration::from_millis(8));
                Ok(input)
            } else if input == 1 {
                Err(input)
            } else {
                Ok(input)
            }
        })
        .unwrap_err();

        assert_eq!(failure, 1);
    }

    #[test]
    fn zero_component_workers_are_rejected() {
        assert_eq!(
            ComponentExecutionPolicy::from_component_workers(0),
            Err(ExecutionPolicyError::ZeroWorkers)
        );
    }
}
