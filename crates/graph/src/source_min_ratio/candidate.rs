//! Exact maintenance of source-declared fundamental cycle candidates.
//!
//! Algorithm 1's `FindCycle()` considers fundamental spanner cycles and
//! terminal-level fundamental tree cycles whose embeddings are already
//! maintained by the source tree chain. This module accepts only those declared
//! compact candidates, computes their exact current quality, and maintains a
//! deterministic binary heap. It deliberately never discovers candidates by
//! enumerating graph cycles.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::{CirculationArcId, CirculationNetwork, ExactRatio, SourceEdgeId, StableMinRatioError};

use super::{
    chain::{Chain, Shifts},
    cycle::{Cycle, Error as CycleError, Segment},
    input::{Error as InputError, Input, Materialization},
    model::LevelId,
};

/// Stable identity of one source-maintained fundamental candidate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CandidateId(pub usize);

/// The source operation that produced one compact fundamental cycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A rejected core edge together with its maintained spanner embedding.
    FundamentalSpanner {
        /// The stable source edge rejected by the selected sparsifier.
        rejected: SourceEdgeId,
    },
    /// A terminal-level non-tree edge and its unique tree path.
    FundamentalTree {
        /// The stable source edge added to the terminal tree.
        source: SourceEdgeId,
    },
}

impl Kind {
    const fn anchor(self) -> SourceEdgeId {
        match self {
            Self::FundamentalSpanner { rejected } => rejected,
            Self::FundamentalTree { source } => source,
        }
    }
}

/// One compact candidate supplied by source tree-chain/embedding maintenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fundamental {
    /// Stable candidate identity used for deterministic heap ties and updates.
    pub id: CandidateId,
    /// The source-maintained candidate population containing this cycle.
    pub kind: Kind,
    /// Its explicit compact source-edge representation.
    pub cycle: Cycle,
}

/// Immutable exact objects needed to evaluate supplied candidates in one
/// provenance-preserving coordinate snapshot.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    input: &'a Input,
    materialization: &'a Materialization,
    chain: &'a Chain,
    shifts: &'a Shifts,
    network: &'a CirculationNetwork,
}

impl<'a> Context<'a> {
    /// Validates that the supplied graph and bindings still materialize from
    /// the exact circulation projection for this network.
    ///
    /// # Errors
    ///
    /// Returns an error when the network changed or the source graph/bindings
    /// do not exactly match this input projection.
    pub fn new(
        input: &'a Input,
        materialization: &'a Materialization,
        chain: &'a Chain,
        shifts: &'a Shifts,
        network: &'a CirculationNetwork,
    ) -> Result<Self, Error> {
        if &input.materialize(network)? != materialization {
            return Err(Error::MismatchedMaterialization);
        }
        Ok(Self {
            input,
            materialization,
            chain,
            shifts,
            network,
        })
    }
}

/// Exact finite accounting for the maintained candidate heap.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Accounting {
    /// Candidate records inserted into the registry.
    pub inserted: u64,
    /// Existing candidate records replaced after an embedding update.
    pub replaced: u64,
    /// Candidate records retired by source maintenance.
    pub retired: u64,
    /// Superseded heap records discarded before a query result.
    pub stale: u64,
    /// Exact quality comparisons performed by heap maintenance.
    pub comparisons: u64,
    /// Exact-quality ties resolved by stable candidate ID.
    pub equal_quality_ties: u64,
    /// Largest encoded heap size observed in this snapshot.
    pub maximum_heap_size: u64,
}

/// A candidate selected by exact absolute quality and oriented for an IPM
/// descent update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choice {
    /// Selected stable candidate identity.
    pub id: CandidateId,
    /// Source candidate population containing the choice.
    pub kind: Kind,
    /// Compact cycle oriented so its gradient dot product is negative.
    pub cycle: Cycle,
    /// Exact absolute ratio `|<gradient, cycle>| / ||length * cycle||_1`.
    pub quality: ExactRatio,
    /// Exact negative gradient dot product of the oriented cycle.
    pub gradient_dot: ExactRatio,
    /// Exact positive weighted one-norm of the oriented cycle.
    pub length_norm: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    candidate: Fundamental,
    quality: ExactRatio,
    gradient_dot: ExactRatio,
    length_norm: ExactRatio,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HeapItem {
    id: CandidateId,
    generation: u64,
    quality: ExactRatio,
}

/// A deterministic heap over only source-declared fundamental candidates.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Registry {
    entries: BTreeMap<CandidateId, Entry>,
    heap: Vec<HeapItem>,
    accounting: Accounting,
}

impl Registry {
    /// Evaluates and registers an explicit initial source candidate population.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate IDs, an invalid fundamental shape, a
    /// nondecodable compact cycle, or exact arithmetic overflow.
    pub fn new(context: &Context<'_>, candidates: Vec<Fundamental>) -> Result<Self, Error> {
        let mut registry = Self::default();
        for candidate in candidates {
            registry.insert(context, candidate)?;
        }
        Ok(registry)
    }

    /// Registers one newly maintained source candidate without discovering any
    /// candidate from the graph.
    ///
    /// # Errors
    ///
    /// Returns an error when the stable candidate ID is already present or its
    /// supplied compact source representation is invalid.
    pub fn insert(&mut self, context: &Context<'_>, candidate: Fundamental) -> Result<(), Error> {
        if self.entries.contains_key(&candidate.id) {
            return Err(Error::DuplicateCandidate(candidate.id));
        }
        let entry = evaluate(context, candidate, 0)?;
        self.entries.insert(entry.candidate.id, entry.clone());
        self.push(HeapItem {
            id: entry.candidate.id,
            generation: entry.generation,
            quality: entry.quality,
        })?;
        self.accounting.inserted = self
            .accounting
            .inserted
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        Ok(())
    }

    /// Replaces a maintained candidate after its source embedding changes.
    ///
    /// The old heap record is deliberately retained as stale evidence until a
    /// later exact query discards it.
    ///
    /// # Errors
    ///
    /// Returns an error when the candidate is unknown, the replacement does
    /// not decode, or exact accounting overflows.
    pub fn replace(&mut self, context: &Context<'_>, candidate: Fundamental) -> Result<(), Error> {
        let prior = self
            .entries
            .get(&candidate.id)
            .ok_or(Error::UnknownCandidate(candidate.id))?;
        let generation = prior.generation.checked_add(1).ok_or(Error::Overflow)?;
        let entry = evaluate(context, candidate, generation)?;
        self.entries.insert(entry.candidate.id, entry.clone());
        self.push(HeapItem {
            id: entry.candidate.id,
            generation: entry.generation,
            quality: entry.quality,
        })?;
        self.accounting.replaced = self
            .accounting
            .replaced
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        Ok(())
    }

    /// Retires one source candidate whose maintained embedding no longer
    /// exists. The stale heap record remains auditable until queried.
    ///
    /// # Errors
    ///
    /// Returns an error when no active candidate has this stable identity.
    pub fn retire(&mut self, id: CandidateId) -> Result<(), Error> {
        if self.entries.remove(&id).is_none() {
            return Err(Error::UnknownCandidate(id));
        }
        self.accounting.retired = self
            .accounting
            .retired
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        Ok(())
    }

    /// Returns the current best nonzero-quality candidate, oriented for a
    /// negative IPM gradient dot product.
    ///
    /// A registry containing no candidate with nonzero quality returns `None`;
    /// it does not enumerate a replacement cycle.
    ///
    /// # Errors
    ///
    /// Returns an error only when exact stale-record cleanup or arithmetic
    /// accounting overflows.
    pub fn best(&mut self) -> Result<Option<Choice>, Error> {
        loop {
            let Some(item) = self.heap.first().copied() else {
                return Ok(None);
            };
            let Some(entry) = self.entries.get(&item.id) else {
                self.discard_stale()?;
                continue;
            };
            if entry.generation != item.generation || entry.quality != item.quality {
                self.discard_stale()?;
                continue;
            }
            if entry.gradient_dot.is_zero() {
                return Ok(None);
            }
            let (cycle, gradient_dot) = if entry.gradient_dot.is_positive() {
                (
                    entry.candidate.cycle.reversed(),
                    entry.gradient_dot.checked_neg()?,
                )
            } else {
                (entry.candidate.cycle.clone(), entry.gradient_dot)
            };
            return Ok(Some(Choice {
                id: entry.candidate.id,
                kind: entry.candidate.kind,
                cycle,
                quality: entry.quality,
                gradient_dot,
                length_norm: entry.length_norm,
            }));
        }
    }

    /// Returns the currently active candidate records in stable-ID order.
    #[must_use]
    pub fn candidates(&self) -> Vec<&Fundamental> {
        self.entries
            .values()
            .map(|entry| &entry.candidate)
            .collect()
    }

    /// Returns finite observed maintenance counters, not an amortized bound.
    #[must_use]
    pub const fn accounting(&self) -> Accounting {
        self.accounting
    }

    fn discard_stale(&mut self) -> Result<(), Error> {
        let _ = self.pop()?;
        self.accounting.stale = self
            .accounting
            .stale
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        Ok(())
    }

    fn push(&mut self, item: HeapItem) -> Result<(), Error> {
        self.heap.push(item);
        self.accounting.maximum_heap_size = self
            .accounting
            .maximum_heap_size
            .max(u64::try_from(self.heap.len()).map_err(|_| Error::Overflow)?);
        let mut index = self.heap.len() - 1;
        while index > 0 {
            let parent = (index - 1) / 2;
            if !self.better(self.heap[index], self.heap[parent])? {
                break;
            }
            self.heap.swap(index, parent);
            index = parent;
        }
        Ok(())
    }

    fn pop(&mut self) -> Result<Option<HeapItem>, Error> {
        let Some(last) = self.heap.pop() else {
            return Ok(None);
        };
        if self.heap.is_empty() {
            return Ok(Some(last));
        }
        let best = std::mem::replace(&mut self.heap[0], last);
        let mut index = 0_usize;
        loop {
            let left = index
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or(Error::Overflow)?;
            if left >= self.heap.len() {
                break;
            }
            let right = left.checked_add(1).ok_or(Error::Overflow)?;
            let mut child = left;
            if right < self.heap.len() && self.better(self.heap[right], self.heap[left])? {
                child = right;
            }
            if !self.better(self.heap[child], self.heap[index])? {
                break;
            }
            self.heap.swap(index, child);
            index = child;
        }
        Ok(Some(best))
    }

    fn better(&mut self, first: HeapItem, second: HeapItem) -> Result<bool, Error> {
        self.accounting.comparisons = self
            .accounting
            .comparisons
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        if first.quality == second.quality {
            self.accounting.equal_quality_ties = self
                .accounting
                .equal_quality_ties
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            return Ok(first.id < second.id);
        }
        Ok(first.quality.at_least(second.quality)?)
    }
}

fn evaluate(
    context: &Context<'_>,
    candidate: Fundamental,
    generation: u64,
) -> Result<Entry, Error> {
    validate_shape(&candidate, context.chain)?;
    let decoded = candidate.cycle.decode(
        &context.materialization.graph,
        context.chain,
        context.shifts,
        &context.materialization.bindings,
        context.network,
    )?;
    let direction = aggregate(decoded)?;
    let zero = ExactRatio::new(0, 1)?;
    let (gradient_dot, length_norm) =
        direction
            .into_iter()
            .try_fold((zero, zero), |(dot, norm), (arc, coefficient)| {
                let coordinate = context
                    .input
                    .arc(arc)
                    .ok_or(Error::MissingCoordinate(arc))?;
                let scale = ExactRatio::new(coefficient, 1)?;
                Ok::<_, Error>((
                    dot.checked_add(coordinate.gradient.checked_mul(scale)?)?,
                    norm.checked_add(coordinate.length.checked_mul(scale.abs()?)?)?,
                ))
            })?;
    if !length_norm.is_positive() {
        return Err(Error::ZeroDirection(candidate.id));
    }
    let quality = gradient_dot.abs()?.checked_mul(length_norm.reciprocal()?)?;
    Ok(Entry {
        candidate,
        quality,
        gradient_dot,
        length_norm,
        generation,
    })
}

fn validate_shape(candidate: &Fundamental, chain: &Chain) -> Result<(), Error> {
    let anchor_count = candidate
        .cycle
        .segments
        .iter()
        .filter(|segment| {
            matches!(
                segment,
                Segment::OffTree { source, .. } if *source == candidate.kind.anchor()
            )
        })
        .count();
    if anchor_count != 1 {
        return Err(Error::InvalidAnchor(candidate.id));
    }
    let Kind::FundamentalTree { .. } = candidate.kind else {
        return Ok(());
    };
    let terminal = chain
        .levels()
        .last()
        .map(super::model::Level::id)
        .ok_or(Error::MissingTerminalLevel)?;
    let tree_paths = candidate
        .cycle
        .segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::TreePath { selection, .. } => Some(selection.level),
            Segment::OffTree { .. } => None,
        })
        .collect::<Vec<LevelId>>();
    if candidate.cycle.segments.len() != 2 || tree_paths.as_slice() != [terminal] {
        return Err(Error::InvalidTerminalTree(candidate.id));
    }
    Ok(())
}

fn aggregate(
    decoded: Vec<(CirculationArcId, i8)>,
) -> Result<BTreeMap<CirculationArcId, i128>, Error> {
    let mut result = BTreeMap::new();
    for (arc, sign) in decoded {
        let coefficient = result.entry(arc).or_insert(0_i128);
        *coefficient = coefficient
            .checked_add(i128::from(sign))
            .ok_or(Error::Overflow)?;
    }
    result.retain(|_, coefficient| *coefficient != 0);
    Ok(result)
}

/// A source-declared candidate could not be evaluated or maintained.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("source candidate {0:?} is duplicated")]
    DuplicateCandidate(CandidateId),
    #[error("source candidate {0:?} is not active")]
    UnknownCandidate(CandidateId),
    #[error("source candidate {0:?} does not contain its unique fundamental edge")]
    InvalidAnchor(CandidateId),
    #[error(
        "terminal fundamental tree candidate {0:?} is not one terminal tree path plus one edge"
    )]
    InvalidTerminalTree(CandidateId),
    #[error("tree chain has no terminal level")]
    MissingTerminalLevel,
    #[error("source input no longer materializes the supplied graph and bindings")]
    MismatchedMaterialization,
    #[error("source candidate {0:?} decodes to the zero direction")]
    ZeroDirection(CandidateId),
    #[error("circulation arc {0:?} has no exact input coordinate")]
    MissingCoordinate(CirculationArcId),
    #[error("candidate input projection failed: {0}")]
    Input(#[from] InputError),
    #[error("candidate compact-cycle decoding failed: {0}")]
    Cycle(#[from] CycleError),
    #[error("candidate exact arithmetic failed: {0}")]
    Ratio(#[from] StableMinRatioError),
    #[error("candidate maintenance accounting overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{CandidateId, Choice, Context, Error, Fundamental, Kind, Registry};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, SourceEdgeId,
        source_min_ratio::{
            chain::Chain,
            cycle::{Cycle, Direction, Segment},
            input::Input,
            model::{Branch, BranchId, Level, LevelId, Tree},
        },
    };

    fn ratio(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    fn network() -> CirculationNetwork {
        let mut network = CirculationNetwork::new(4);
        for (from, to) in [(0, 1), (1, 2), (2, 3), (0, 3), (0, 2)] {
            network
                .add_arc(FlowNodeId(from), FlowNodeId(to), 2, 0)
                .unwrap();
        }
        network
    }

    fn setup(
        gradients: &[ExactRatio],
    ) -> (
        Input,
        super::Materialization,
        Chain,
        super::Shifts,
        CirculationNetwork,
    ) {
        let network = network();
        let input = Input::new(&network, gradients, &[ratio(1); 5], &[ratio(1); 5]).unwrap();
        let materialization = input.materialize(&network).unwrap();
        let chain = Chain::new(
            &materialization.graph,
            vec![
                Level::new(
                    LevelId(0),
                    vec![Branch::new(
                        BranchId(0),
                        0,
                        Tree::new(
                            FlowNodeId(0),
                            BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1), SourceEdgeId(2)]),
                        ),
                    )],
                ),
                Level::new(
                    LevelId(1),
                    vec![Branch::new(
                        BranchId(1),
                        0,
                        Tree::new(
                            FlowNodeId(0),
                            BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1), SourceEdgeId(3)]),
                        ),
                    )],
                ),
            ],
        )
        .unwrap();
        let shifts = chain.initial_shifts();
        (input, materialization, chain, shifts, network)
    }

    fn candidates(chain: &Chain, shifts: &super::Shifts) -> (Fundamental, Fundamental) {
        let selected = chain.select(shifts).unwrap();
        (
            Fundamental {
                id: CandidateId(3),
                kind: Kind::FundamentalSpanner {
                    rejected: SourceEdgeId(3),
                },
                cycle: Cycle {
                    segments: vec![
                        Segment::TreePath {
                            selection: selected[0],
                            from: FlowNodeId(0),
                            to: FlowNodeId(3),
                        },
                        Segment::OffTree {
                            source: SourceEdgeId(3),
                            direction: Direction::Reverse,
                        },
                    ],
                },
            },
            Fundamental {
                id: CandidateId(9),
                kind: Kind::FundamentalTree {
                    source: SourceEdgeId(4),
                },
                cycle: Cycle {
                    segments: vec![
                        Segment::TreePath {
                            selection: selected[1],
                            from: FlowNodeId(0),
                            to: FlowNodeId(2),
                        },
                        Segment::OffTree {
                            source: SourceEdgeId(4),
                            direction: Direction::Reverse,
                        },
                    ],
                },
            },
        )
    }

    #[test]
    fn chooses_the_best_declared_fundamental_cycle_without_enumeration() {
        let (input, materialization, chain, shifts, network) =
            setup(&[ratio(-1), ratio(-1), ratio(-5), ratio(0), ratio(0)]);
        let context = Context::new(&input, &materialization, &chain, &shifts, &network).unwrap();
        let (spanner, terminal) = candidates(&chain, &shifts);
        let mut registry = Registry::new(&context, vec![spanner, terminal]).unwrap();

        assert_eq!(registry.candidates().len(), 2);
        assert_eq!(
            registry.best().unwrap(),
            Some(Choice {
                id: CandidateId(3),
                kind: Kind::FundamentalSpanner {
                    rejected: SourceEdgeId(3),
                },
                cycle: candidates(&chain, &shifts).0.cycle,
                quality: ExactRatio::new(7, 4).unwrap(),
                gradient_dot: ratio(-7),
                length_norm: ratio(4),
            })
        );
    }

    #[test]
    fn reverses_a_positive_candidate_for_an_ipm_descent_direction() {
        let (input, materialization, chain, shifts, network) =
            setup(&[ratio(1), ratio(1), ratio(5), ratio(0), ratio(0)]);
        let context = Context::new(&input, &materialization, &chain, &shifts, &network).unwrap();
        let (spanner, _) = candidates(&chain, &shifts);
        let mut registry = Registry::new(&context, vec![spanner]).unwrap();

        let choice = registry.best().unwrap().unwrap();
        assert_eq!(choice.gradient_dot, ratio(-7));
        assert_eq!(
            choice
                .cycle
                .decode(
                    &materialization.graph,
                    &chain,
                    &shifts,
                    &materialization.bindings,
                    &network,
                )
                .unwrap(),
            vec![
                (crate::CirculationArcId(3), 1),
                (crate::CirculationArcId(2), -1),
                (crate::CirculationArcId(1), -1),
                (crate::CirculationArcId(0), -1),
            ]
        );
    }

    #[test]
    fn records_stale_heap_entries_after_a_source_candidate_replacement() {
        let (input, materialization, chain, shifts, network) =
            setup(&[ratio(-1), ratio(-1), ratio(-5), ratio(0), ratio(0)]);
        let context = Context::new(&input, &materialization, &chain, &shifts, &network).unwrap();
        let (spanner, _) = candidates(&chain, &shifts);
        let mut registry = Registry::new(&context, vec![spanner.clone()]).unwrap();
        registry.replace(&context, spanner).unwrap();

        assert_eq!(registry.best().unwrap().unwrap().id, CandidateId(3));
        assert_eq!(registry.accounting().replaced, 1);
        assert_eq!(registry.accounting().stale, 1);
    }

    #[test]
    fn rejects_duplicate_unanchored_and_nonterminal_declarations() {
        let (input, materialization, chain, shifts, network) =
            setup(&[ratio(-1), ratio(-1), ratio(-5), ratio(0), ratio(0)]);
        let context = Context::new(&input, &materialization, &chain, &shifts, &network).unwrap();
        let (spanner, terminal) = candidates(&chain, &shifts);
        assert_eq!(
            Registry::new(&context, vec![spanner.clone(), spanner.clone()]),
            Err(Error::DuplicateCandidate(CandidateId(3)))
        );
        let mut unanchored = spanner.clone();
        unanchored.cycle = terminal.cycle.clone();
        assert_eq!(
            Registry::new(&context, vec![unanchored]),
            Err(Error::InvalidAnchor(CandidateId(3)))
        );
        let mut nonterminal = terminal;
        let Segment::TreePath { selection, .. } = &mut nonterminal.cycle.segments[0] else {
            unreachable!();
        };
        *selection = chain.select(&shifts).unwrap()[0];
        assert_eq!(
            Registry::new(&context, vec![nonterminal]),
            Err(Error::InvalidTerminalTree(CandidateId(9)))
        );
    }

    #[test]
    fn leaves_an_empty_source_candidate_population_empty() {
        let (input, materialization, chain, shifts, network) =
            setup(&[ratio(-1), ratio(-1), ratio(-5), ratio(0), ratio(0)]);
        let context = Context::new(&input, &materialization, &chain, &shifts, &network).unwrap();
        let mut registry = Registry::new(&context, Vec::new()).unwrap();
        assert_eq!(registry.best().unwrap(), None);
    }
}
