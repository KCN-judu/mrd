use crate::{ExactRatio, MinRatioEdgeId, StableEdge};

use super::{Error, Query};

pub(super) fn minimum_ratio_cycle(edges: &[StableEdge]) -> Result<(Option<Query>, u64), Error> {
    let node_count = edges
        .iter()
        .flat_map(|edge| [edge.from.0, edge.to.0])
        .max()
        .map_or(0, |node| node + 1);
    let mut adjacency = vec![Vec::<(usize, MinRatioEdgeId, i8)>::new(); node_count];
    for (index, edge) in edges.iter().enumerate() {
        let id = MinRatioEdgeId(index);
        adjacency[edge.from.0].push((edge.to.0, id, 1));
        adjacency[edge.to.0].push((edge.from.0, id, -1));
    }
    let mut best = None;
    let mut candidates = 0_u64;
    for start in 0..node_count {
        let mut seen = vec![false; node_count];
        seen[start] = true;
        enumerate_cycles(
            edges,
            &adjacency,
            start,
            start,
            &mut seen,
            &mut Vec::new(),
            0,
            0,
            &mut best,
            &mut candidates,
        )?;
    }
    Ok((best, candidates))
}

#[allow(clippy::too_many_arguments)]
fn enumerate_cycles(
    edges: &[StableEdge],
    adjacency: &[Vec<(usize, MinRatioEdgeId, i8)>],
    start: usize,
    node: usize,
    seen: &mut [bool],
    path: &mut Vec<(MinRatioEdgeId, i8)>,
    gradient: i128,
    length: i128,
    best: &mut Option<Query>,
    candidates: &mut u64,
) -> Result<(), Error> {
    for (next, id, direction) in &adjacency[node] {
        if path.iter().any(|(previous, _)| previous == id) {
            continue;
        }
        let edge = edges.get(id.0).ok_or(Error::Overflow)?;
        let signed_gradient = edge
            .gradient
            .checked_mul(i128::from(*direction))
            .ok_or(Error::Overflow)?;
        let next_gradient = gradient
            .checked_add(signed_gradient)
            .ok_or(Error::Overflow)?;
        let next_length = length.checked_add(edge.length).ok_or(Error::Overflow)?;
        if *next == start {
            *candidates = candidates.checked_add(1).ok_or(Error::Overflow)?;
            let mut cycle = path.clone();
            cycle.push((*id, *direction));
            let ratio = ExactRatio::new(next_gradient, next_length).map_err(|_| Error::Overflow)?;
            let candidate = Query {
                cycle,
                gradient_sum: next_gradient,
                length_sum: next_length,
                ratio,
            };
            let replace = match best.as_ref() {
                None => true,
                Some(current) => {
                    current
                        .ratio
                        .at_least(&candidate.ratio)
                        .map_err(|_| Error::Overflow)?
                        && current.ratio != candidate.ratio
                }
            };
            if replace {
                *best = Some(candidate);
            }
        } else if !seen[*next] {
            seen[*next] = true;
            path.push((*id, *direction));
            enumerate_cycles(
                edges,
                adjacency,
                start,
                *next,
                seen,
                path,
                next_gradient,
                next_length,
                best,
                candidates,
            )?;
            path.pop();
            seen[*next] = false;
        }
    }
    Ok(())
}
