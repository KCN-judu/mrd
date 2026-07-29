//! Pure recomputation helpers for finite graph certificates.

use crate::ExactRatio;

use super::{
    super::model::{EdgeId, Graph},
    domain::Error,
};

pub(super) const MAX_EXHAUSTIVE_NODES: usize = 20;

pub(super) fn degrees(graph: &Graph) -> Result<Vec<u64>, Error> {
    let mut degrees = vec![0_u64; graph.node_count()];
    for index in 0..graph.edge_count() {
        let edge = graph.edge(EdgeId(index)).ok_or(Error::InvalidCertificate)?;
        degrees[edge.first.0] = degrees[edge.first.0]
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        degrees[edge.second.0] = degrees[edge.second.0]
            .checked_add(1)
            .ok_or(Error::Overflow)?;
    }
    Ok(degrees)
}

pub(super) fn expansion(graph: &Graph) -> Result<(ExactRatio, u64), Error> {
    let nodes = graph.node_count();
    let all = (1_u64 << nodes) - 1;
    let mut minimum: Option<ExactRatio> = None;
    let mut checked = 0_u64;
    for mask in 1..all {
        let complement = all ^ mask;
        if complement == 0 {
            continue;
        }
        let mut cut = 0_u64;
        let mut left_volume = 0_u64;
        let mut right_volume = 0_u64;
        for index in 0..graph.edge_count() {
            let edge = graph.edge(EdgeId(index)).ok_or(Error::InvalidCertificate)?;
            let first = mask & (1_u64 << edge.first.0) != 0;
            let second = mask & (1_u64 << edge.second.0) != 0;
            if first {
                left_volume = left_volume.checked_add(1).ok_or(Error::Overflow)?;
            } else {
                right_volume = right_volume.checked_add(1).ok_or(Error::Overflow)?;
            }
            if second {
                left_volume = left_volume.checked_add(1).ok_or(Error::Overflow)?;
            } else {
                right_volume = right_volume.checked_add(1).ok_or(Error::Overflow)?;
            }
            if first != second {
                cut = cut.checked_add(1).ok_or(Error::Overflow)?;
            }
        }
        let denominator = left_volume.min(right_volume);
        if denominator == 0 {
            return Err(Error::InvalidCertificate);
        }
        let value = ExactRatio::new(i128::from(cut), i128::from(denominator)).map_err(map_ratio)?;
        if minimum.is_none_or(|current| {
            current
                .at_least(value)
                .is_ok_and(|greater| greater && current != value)
        }) {
            minimum = Some(value);
        }
        checked = checked.checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok((minimum.ok_or(Error::InvalidCertificate)?, checked))
}

pub(super) fn connected(graph: &Graph) -> Result<bool, Error> {
    let mut seen = vec![false; graph.node_count()];
    let mut queue = std::collections::VecDeque::from([0_usize]);
    seen[0] = true;
    while let Some(node) = queue.pop_front() {
        for index in 0..graph.edge_count() {
            let edge = graph.edge(EdgeId(index)).ok_or(Error::InvalidCertificate)?;
            let next = if edge.first.0 == node {
                edge.second.0
            } else if edge.second.0 == node {
                edge.first.0
            } else {
                continue;
            };
            if !seen[next] {
                seen[next] = true;
                queue.push_back(next);
            }
        }
    }
    Ok(!seen.contains(&false))
}

pub(super) fn ceil_log2(value: usize) -> u32 {
    if value <= 1 {
        0
    } else {
        usize::BITS - (value - 1).leading_zeros()
    }
}

pub(super) fn map_ratio(_: crate::StableMinRatioError) -> Error {
    Error::Overflow
}
