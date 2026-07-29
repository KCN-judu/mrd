//! Checked execution of externally supplied IPM directions.
//!
//! This module isolates the pure state transition and Detect accounting from
//! the still-missing direction-selection construction. It is intentionally not
//! a solver: callers must supply an exact direction and both certified
//! approximations for every step.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, ExactRatio,
    IpmApproximationCertificate, IpmDetectLedger, IpmUpdateMetrics, MinCostCirculationError,
    MinRatioEdgeId, SourceDynamicGraph, StableMinRatioError, StableMinRatioLedger,
    source_min_ratio::{
        candidate::Error as CandidateError,
        chain::{Chain, Shifts},
        cycle::{ArcBindings, Cycle, Error as CompactCycleError},
        query::decode_candidate,
        terminal::{Error as TerminalError, Tree as TerminalTree},
    },
};

/// One immutable, externally supplied Lemma 4.4 update request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    /// Certified approximate gradients for the current snapshot.
    pub approximate_gradients: Vec<ExactRatio>,
    /// Certified approximate lengths for the current snapshot.
    pub approximate_lengths: Vec<ExactRatio>,
    /// Source update-quality parameter.
    pub kappa: ExactRatio,
    /// Exact circulation direction selected by an external construction.
    pub direction: Vec<ExactRatio>,
}

/// Immutable input needed to decode one externally selected compact cycle.
///
/// This is a transport boundary only. It deliberately does not choose the
/// compact cycle or make a minimum-ratio query claim.
#[derive(Clone, Copy, Debug)]
pub struct CompactCandidate<'a> {
    /// Already validated hidden-stability state.
    pub ledger: &'a StableMinRatioLedger,
    /// Externally selected compact cycle.
    pub cycle: &'a Cycle,
    /// Source graph carrying stable edge provenance.
    pub graph: &'a SourceDynamicGraph,
    /// Checked source-tree chain.
    pub chain: &'a Chain,
    /// Immutable branch shifts for this candidate.
    pub shifts: &'a Shifts,
    /// Exact source-edge to circulation-arc bindings.
    pub bindings: &'a ArcBindings,
}

impl Step {
    /// Forms an exact update request from the best nonzero terminal-tree
    /// declaration in one checked source/IPM snapshot.
    ///
    /// The supplied approximation vectors must exactly equal the immutable
    /// coordinates that formed `terminal`; a merely compatible or reordered
    /// vector is rejected. The terminal heap supplies only source-declared
    /// fundamental tree cycles. It neither creates core/spanner candidates nor
    /// falls back to an enumerating cycle implementation.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched coordinates, a changed source snapshot,
    /// candidate evaluation failure, or compact-cycle decoding failure.
    pub fn from_terminal_candidate(
        ledger: &StableMinRatioLedger,
        terminal: &TerminalTree,
        network: &CirculationNetwork,
        approximate_gradients: Vec<ExactRatio>,
        approximate_lengths: Vec<ExactRatio>,
        kappa: ExactRatio,
    ) -> Result<Option<Self>, Error> {
        let expected_gradients = terminal
            .input()
            .arcs()
            .iter()
            .map(|arc| arc.gradient)
            .collect::<Vec<_>>();
        let expected_lengths = terminal
            .input()
            .arcs()
            .iter()
            .map(|arc| arc.length)
            .collect::<Vec<_>>();
        if approximate_gradients != expected_gradients || approximate_lengths != expected_lengths {
            return Err(Error::MismatchedTerminalCoordinates);
        }
        let mut registry = terminal.registry(network)?;
        let Some(choice) = registry.best()? else {
            return Ok(None);
        };
        Ok(Some(Self::from_compact_candidate(
            CompactCandidate {
                ledger,
                cycle: &choice.cycle,
                graph: &terminal.materialization().graph,
                chain: terminal.chain(),
                shifts: terminal.shifts(),
                bindings: &terminal.materialization().bindings,
            },
            network,
            approximate_gradients,
            approximate_lengths,
            kappa,
        )?))
    }

    /// Forms an exact update request from an externally selected compact cycle.
    ///
    /// The source query boundary decodes the compact cycle into signed
    /// circulation arcs. This conversion sums those signed occurrences into a
    /// full exact direction vector and rechecks circulation before the later
    /// Lemma 4.4 transition.
    ///
    /// # Errors
    ///
    /// Returns an error when compact decoding, exact vector construction, or
    /// circulation validation fails.
    pub fn from_compact_candidate(
        compact: CompactCandidate<'_>,
        network: &CirculationNetwork,
        approximate_gradients: Vec<ExactRatio>,
        approximate_lengths: Vec<ExactRatio>,
        kappa: ExactRatio,
    ) -> Result<Self, Error> {
        let decoded = decode_candidate(
            compact.ledger,
            compact.cycle,
            compact.graph,
            compact.chain,
            compact.shifts,
            compact.bindings,
            network,
        )?;
        let zero = ExactRatio::new(0, 1)?;
        let mut direction = vec![zero; network.arc_count()];
        for (arc, sign) in decoded.arcs {
            let slot = direction.get_mut(arc.0).ok_or(Error::InvalidArc)?;
            *slot = slot.checked_add(ExactRatio::new(i128::from(sign), 1)?)?;
        }
        network.verify_fractional_circulation(&direction)?;
        Ok(Self {
            approximate_gradients,
            approximate_lengths,
            kappa,
            direction,
        })
    }
}

/// Observable result of one accepted transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Outcome {
    /// Certified source step size.
    pub eta: ExactRatio,
    /// Approximation checks accepted before the transition.
    pub approximation: IpmApproximationCertificate,
    /// Cumulative snapshot update metrics after the transition.
    pub metrics: IpmUpdateMetrics,
}

/// Session-local IPM state and explicit Detect accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    snapshot: CertifiedIpmSnapshot,
    detect: IpmDetectLedger,
}

impl Session {
    /// Creates a session from an already certified snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot's bounded fixed-point configuration
    /// cannot initialize the Detect ledger.
    pub fn new(snapshot: CertifiedIpmSnapshot) -> Result<Self, Error> {
        let detect = IpmDetectLedger::new(&snapshot)?;
        Ok(Self { snapshot, detect })
    }

    /// Returns the current immutable certified snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &CertifiedIpmSnapshot {
        &self.snapshot
    }

    /// Applies one externally supplied direction after complete local checks.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied step fails any Lemma 4.4 condition
    /// or Detect accounting cannot record the accepted transition.
    pub fn apply(&mut self, network: &CirculationNetwork, step: &Step) -> Result<Outcome, Error> {
        let update = self.snapshot.apply_lemma_44_update(
            network,
            &step.approximate_gradients,
            &step.approximate_lengths,
            step.kappa,
            &step.direction,
        )?;
        self.detect
            .record_update(&self.snapshot, update.eta, &step.direction)?;
        self.snapshot = update.next_snapshot;
        Ok(Outcome {
            eta: update.eta,
            approximation: update.approximation,
            metrics: self.snapshot.update_metrics(),
        })
    }

    /// Runs the exact source Detect threshold over accumulated updates.
    ///
    /// # Errors
    ///
    /// Returns an error when the threshold is invalid or cannot be certified.
    pub fn detect(&mut self, epsilon: ExactRatio) -> Result<Vec<MinRatioEdgeId>, Error> {
        Ok(self.detect.detect(epsilon)?)
    }

    /// Returns the observed Detect counters for this session.
    #[must_use]
    pub const fn detect_metrics(&self) -> IpmUpdateMetrics {
        self.detect.metrics()
    }
}

/// One supplied iteration step could not be certified or accounted for.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Ipm(#[from] CertifiedIpmError),
    #[error("compact source-cycle decoding failed: {0}")]
    Compact(#[from] CompactCycleError),
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
    #[error(transparent)]
    Ratio(#[from] StableMinRatioError),
    #[error("terminal source candidate failed: {0}")]
    Terminal(#[from] TerminalError),
    #[error("terminal candidate heap failed: {0}")]
    Candidate(#[from] CandidateError),
    #[error("caller approximation coordinates differ from the terminal source input")]
    MismatchedTerminalCoordinates,
    #[error("decoded compact cycle refers to an invalid circulation arc")]
    InvalidArc,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{CompactCandidate, Error, Step};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId,
        SourceWeightedEdge, StableEdge, StableMinRatioLedger, StableWitness,
        source_min_ratio::{
            chain::Chain,
            cycle::{ArcBindings, Cycle, Direction, Segment},
            input::Input,
            model::{Branch, BranchId, Level, LevelId, Tree},
            terminal::Tree as TerminalTree,
        },
    };

    #[test]
    fn converts_a_compact_cycle_to_a_full_exact_direction() {
        let edge = |first, second| SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        };
        let graph =
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap();
        let chain = Chain::new(
            &graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(
                        FlowNodeId(0),
                        BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
                    ),
                )],
            )],
        )
        .unwrap();
        let mut network = CirculationNetwork::new(3);
        let first = network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 0).unwrap();
        let second = network.add_arc(FlowNodeId(1), FlowNodeId(2), 2, 0).unwrap();
        let off_tree = network.add_arc(FlowNodeId(0), FlowNodeId(2), 2, 0).unwrap();
        let bindings = ArcBindings::new(
            &graph,
            &network,
            vec![
                (SourceEdgeId(0), first),
                (SourceEdgeId(1), second),
                (SourceEdgeId(2), off_tree),
            ],
        )
        .unwrap();
        let ledger = StableMinRatioLedger::new(
            2,
            vec![
                StableEdge {
                    from: FlowNodeId(0),
                    to: FlowNodeId(1),
                    gradient: -1,
                    length: 1,
                },
                StableEdge {
                    from: FlowNodeId(1),
                    to: FlowNodeId(0),
                    gradient: 0,
                    length: 1,
                },
            ],
            ExactRatio::new(1, 4).unwrap(),
            ExactRatio::new(1, 2).unwrap(),
            StableWitness {
                circulation: vec![1, 1],
                upper_bounds: vec![1, 1],
            },
        )
        .unwrap();
        let shifts = chain.initial_shifts();
        let selection = chain.select(&shifts).unwrap()[0];
        let cycle = Cycle {
            segments: vec![
                Segment::TreePath {
                    selection,
                    from: FlowNodeId(0),
                    to: FlowNodeId(2),
                },
                Segment::OffTree {
                    source: SourceEdgeId(2),
                    direction: Direction::Reverse,
                },
            ],
        };
        let step = Step::from_compact_candidate(
            CompactCandidate {
                ledger: &ledger,
                cycle: &cycle,
                graph: &graph,
                chain: &chain,
                shifts: &shifts,
                bindings: &bindings,
            },
            &network,
            vec![ExactRatio::new(1, 1).unwrap(); 3],
            vec![ExactRatio::new(1, 1).unwrap(); 3],
            ExactRatio::new(1, 2).unwrap(),
        )
        .unwrap();
        assert_eq!(
            step.direction,
            vec![
                ExactRatio::new(1, 1).unwrap(),
                ExactRatio::new(1, 1).unwrap(),
                ExactRatio::new(-1, 1).unwrap(),
            ]
        );
    }

    #[test]
    fn converts_the_checked_terminal_choice_only_when_coordinates_match() {
        let mut network = CirculationNetwork::new(3);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 0).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(2), 2, 0).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(2), 2, 0).unwrap();
        let gradients = vec![
            ExactRatio::new(1, 1).unwrap(),
            ExactRatio::new(4, 1).unwrap(),
            ExactRatio::new(16, 1).unwrap(),
        ];
        let lengths = vec![ExactRatio::new(1, 1).unwrap(); 3];
        let input = Input::new(&network, &gradients, &lengths, &lengths).unwrap();
        let terminal = TerminalTree::build(input, &network, FlowNodeId(0)).unwrap();
        let ledger = StableMinRatioLedger::new(
            2,
            vec![
                StableEdge {
                    from: FlowNodeId(0),
                    to: FlowNodeId(1),
                    gradient: -1,
                    length: 1,
                },
                StableEdge {
                    from: FlowNodeId(1),
                    to: FlowNodeId(0),
                    gradient: 0,
                    length: 1,
                },
            ],
            ExactRatio::new(1, 4).unwrap(),
            ExactRatio::new(1, 2).unwrap(),
            StableWitness {
                circulation: vec![1, 1],
                upper_bounds: vec![1, 1],
            },
        )
        .unwrap();

        let step = Step::from_terminal_candidate(
            &ledger,
            &terminal,
            &network,
            gradients.clone(),
            lengths.clone(),
            ExactRatio::new(1, 2).unwrap(),
        )
        .unwrap()
        .unwrap();
        assert!(
            step.direction
                .iter()
                .any(|coefficient| !coefficient.is_zero())
        );
        assert_eq!(
            Step::from_terminal_candidate(
                &ledger,
                &terminal,
                &network,
                vec![ExactRatio::new(0, 1).unwrap(), gradients[1], gradients[2]],
                lengths,
                ExactRatio::new(1, 2).unwrap(),
            ),
            Err(Error::MismatchedTerminalCoordinates)
        );
    }
}
