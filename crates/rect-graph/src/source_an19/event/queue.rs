use super::{engine::ArcWitness, trace::StaleReason};
use crate::{ExactRatio, FlowNodeId, source_an19::petal::Error};

use super::super::petal::ratio_less;

#[derive(Clone)]
pub(super) struct Item {
    pub(super) distance: ExactRatio,
    pub(super) vertex: FlowNodeId,
    pub(super) insertion_sequence: u64,
    pub(super) predecessor: Option<ArcWitness>,
}

#[derive(Clone)]
pub(super) struct Observation {
    pub(super) item: Item,
    pub(super) pop_sequence: Option<u64>,
    pub(super) stale_reason: Option<StaleReason>,
    pub(super) insertion: bool,
}

#[derive(Default)]
pub(super) struct Statistics {
    pub(super) inserted: u64,
    pub(super) popped: u64,
    pub(super) stale: u64,
    pub(super) comparisons: u64,
    pub(super) heap_push_comparisons: u64,
    pub(super) heap_pop_comparisons: u64,
    pub(super) relaxation_label_comparisons: u64,
    pub(super) replacements: u64,
    pub(super) equal_key_ties: u64,
    pub(super) maximum_size: u64,
}

pub(super) fn push(
    heap: &mut Vec<Item>,
    item: Item,
    statistics: &mut Statistics,
) -> Result<(), Error> {
    heap.push(item);
    let mut index = heap.len() - 1;
    while index > 0 {
        let parent = (index - 1) / 2;
        if !push_item_less(&heap[index], &heap[parent], statistics)? {
            break;
        }
        heap.swap(index, parent);
        index = parent;
    }
    Ok(())
}

pub(super) fn pop(
    heap: &mut Vec<Item>,
    statistics: &mut Statistics,
) -> Result<Option<Item>, Error> {
    let Some(last) = heap.pop() else {
        return Ok(None);
    };
    if heap.is_empty() {
        return Ok(Some(last));
    }
    let minimum = std::mem::replace(&mut heap[0], last);
    let mut index = 0_usize;
    loop {
        let left = index
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(Error::Overflow)?;
        if left >= heap.len() {
            break;
        }
        let right = left.checked_add(1).ok_or(Error::Overflow)?;
        let mut child = left;
        if right < heap.len() && pop_item_less(&heap[right], &heap[left], statistics)? {
            child = right;
        }
        if !pop_item_less(&heap[child], &heap[index], statistics)? {
            break;
        }
        heap.swap(index, child);
        index = child;
    }
    Ok(Some(minimum))
}

pub(super) fn less(
    first: &Item,
    second: &Item,
    statistics: &mut Statistics,
) -> Result<bool, Error> {
    if first.distance == second.distance {
        statistics.equal_key_ties = statistics
            .equal_key_ties
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        return Ok(
            (first.vertex, first.insertion_sequence) < (second.vertex, second.insertion_sequence)
        );
    }
    ratio_less(first.distance, second.distance)
}

fn push_item_less(first: &Item, second: &Item, statistics: &mut Statistics) -> Result<bool, Error> {
    statistics.comparisons = statistics
        .comparisons
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    statistics.heap_push_comparisons = statistics
        .heap_push_comparisons
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    less(first, second, statistics)
}

fn pop_item_less(first: &Item, second: &Item, statistics: &mut Statistics) -> Result<bool, Error> {
    statistics.comparisons = statistics
        .comparisons
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    statistics.heap_pop_comparisons = statistics
        .heap_pop_comparisons
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    less(first, second, statistics)
}
