//! Checked execution of externally supplied IPM directions.
//!
//! This module isolates the pure state transition and Detect accounting from
//! the still-missing direction-selection construction. It is intentionally not
//! a solver: callers must supply an exact direction and both certified
//! approximations for every step.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::{
    CertifiedFixedPoint, CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, ExactRatio,
    IpmApproximationCertificate, IpmDetectLedger, IpmTerminationCertificate, IpmUpdateMetrics,
    MinCostCirculationError, MinRatioEdgeId, SourceDynamicGraph, StableMinRatioError,
    StableMinRatioLedger,
    source_min_ratio::{
        candidate::{CandidateId, Choice, Error as CandidateError, Registry},
        chain::{Chain, Shifts},
        cycle::{ArcBindings, Cycle, Error as CompactCycleError},
        input::Input,
        query::decode_candidate,
        spanner::{
            Error as SpannerError, Parameters as SpannerParameters, Snapshot as SpannerSnapshot,
        },
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

/// One explicit request to select and apply a source-maintained direction.
///
/// Exact source coordinates belong to `input`, not to the certified IPM
/// intervals. The session independently certifies those supplied coordinates
/// before its state can change.
#[derive(Clone, Copy, Debug)]
pub struct SourceSelected<'a> {
    /// The exact snapshot this source projection was prepared for.
    pub snapshot: &'a CertifiedIpmSnapshot,
    /// Exact current source/IPM coordinates with stable arc provenance.
    pub input: &'a Input,
    /// Already validated hidden-stability state.
    pub ledger: &'a StableMinRatioLedger,
    /// Source-maintained terminal candidate population.
    pub terminal: &'a TerminalTree,
    /// Source-maintained rejected-core candidate population.
    pub spanner: &'a SpannerSnapshot,
    /// Source update-quality parameter.
    pub kappa: ExactRatio,
}

/// One exact source projection, certified for one immutable IPM snapshot.
///
/// The caller owns construction of its exact coordinates. This type only
/// accepts them after certifying the Theorem 4.3 approximation hypotheses and
/// rebuilding both maintained candidate populations. It never guesses an
/// exact coordinate from a fixed-point interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Projection {
    snapshot: CertifiedIpmSnapshot,
    input: Input,
    ledger: StableMinRatioLedger,
    terminal: TerminalTree,
    spanner: SpannerSnapshot,
    kappa: ExactRatio,
    approximation: IpmApproximationCertificate,
}

impl Projection {
    /// Certifies one externally prepared exact source projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot belongs to another network, the
    /// exact coordinates cannot certify Theorem 4.3, or either maintained
    /// candidate population differs from the supplied source input.
    pub fn new(
        snapshot: CertifiedIpmSnapshot,
        input: Input,
        ledger: StableMinRatioLedger,
        terminal: TerminalTree,
        spanner: SpannerSnapshot,
        kappa: ExactRatio,
        network: &CirculationNetwork,
    ) -> Result<Self, Error> {
        snapshot.verify_network(network)?;
        if input != *terminal.input() || input != *spanner.input() {
            return Err(Error::MismatchedPreparedInput);
        }
        let gradients = input
            .arcs()
            .iter()
            .map(|arc| arc.gradient)
            .collect::<Vec<_>>();
        let lengths = input
            .arcs()
            .iter()
            .map(|arc| arc.length)
            .collect::<Vec<_>>();
        let mut arithmetic = CertifiedFixedPoint::new(snapshot.fixed_point_config())
            .map_err(CertifiedIpmError::from)?;
        let approximation =
            snapshot.certify_approximations(&gradients, &lengths, kappa, &mut arithmetic)?;
        terminal.verify(network)?;
        spanner.verify(network)?;
        Ok(Self {
            snapshot,
            input,
            ledger,
            terminal,
            spanner,
            kappa,
            approximation,
        })
    }

    /// Returns the immutable snapshot this source state was prepared for.
    #[must_use]
    pub const fn snapshot(&self) -> &CertifiedIpmSnapshot {
        &self.snapshot
    }

    /// Returns the exact coordinate projection used by this source state.
    #[must_use]
    pub const fn input(&self) -> &Input {
        &self.input
    }

    /// Returns the pre-selection Theorem 4.3 certificate.
    #[must_use]
    pub const fn approximation(&self) -> IpmApproximationCertificate {
        self.approximation
    }

    fn selected(&self) -> SourceSelected<'_> {
        SourceSelected {
            snapshot: &self.snapshot,
            input: &self.input,
            ledger: &self.ledger,
            terminal: &self.terminal,
            spanner: &self.spanner,
            kappa: self.kappa,
        }
    }
}

/// External effect boundary that prepares source state for the current IPM
/// snapshot.
///
/// A factory may maintain source data structures outside this module, but it
/// must return a fresh [`Projection`] for every requested snapshot. Returning
/// a projection for an earlier snapshot is rejected before the session changes.
pub trait Factory {
    /// Prepares one exact source projection for `snapshot` and `network`.
    ///
    /// # Errors
    ///
    /// Returns an error when the external source state cannot prepare a fresh
    /// exact projection that satisfies [`Projection::new`].
    fn prepare(
        &mut self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<Projection, Error>;
}

impl<F> Factory for F
where
    F: FnMut(&CertifiedIpmSnapshot, &CirculationNetwork) -> Result<Projection, Error>,
{
    fn prepare(
        &mut self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<Projection, Error> {
        self(snapshot, network)
    }
}

/// Rebuilds a fresh source projection from one externally supplied immutable
/// exact coordinate set.
///
/// This is a deliberately finite policy boundary. It does not infer exact
/// coordinates from IPM intervals, update the coordinate set after a session
/// transition, or promise that a bounded driver will terminate. It does ensure
/// that every snapshot it accepts gets a newly built terminal tree, spanner
/// snapshot, and Theorem 4.3 certificate before candidate selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixedProjectionFactory {
    input: Input,
    ledger: StableMinRatioLedger,
    parameters: SpannerParameters,
    kappa: ExactRatio,
    preparations: u64,
}

impl FixedProjectionFactory {
    /// Creates a policy that rebuilds source state from one fixed exact input.
    #[must_use]
    pub const fn new(
        input: Input,
        ledger: StableMinRatioLedger,
        parameters: SpannerParameters,
        kappa: ExactRatio,
    ) -> Self {
        Self {
            input,
            ledger,
            parameters,
            kappa,
            preparations: 0,
        }
    }

    /// Returns the number of successful snapshot-bound preparations.
    #[must_use]
    pub const fn preparation_count(&self) -> u64 {
        self.preparations
    }
}

impl Factory for FixedProjectionFactory {
    fn prepare(
        &mut self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<Projection, Error> {
        let terminal = TerminalTree::build(self.input.clone(), network, self.parameters.root)?;
        let spanner = SpannerSnapshot::build(self.input.clone(), network, self.parameters)?;
        let projection = Projection::new(
            snapshot.clone(),
            self.input.clone(),
            self.ledger.clone(),
            terminal,
            spanner,
            self.kappa,
            network,
        )?;
        self.preparations = self
            .preparations
            .checked_add(1)
            .ok_or(Error::IterationCountOverflow)?;
        Ok(projection)
    }
}

/// One accepted transition in a source-iteration run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    /// Zero-based accepted-transition index.
    pub sequence: u64,
    /// Exact snapshot that the source projection certified before this update.
    pub snapshot: CertifiedIpmSnapshot,
    /// Exact source coordinates that produced the selected direction.
    pub input: Input,
    /// Independently checked Theorem 4.3 approximation evidence.
    pub approximation: IpmApproximationCertificate,
    /// Selected compact direction and the accepted certified update.
    pub selected: SelectedOutcome,
}

/// Additive-half termination evidence and the complete accepted driver trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion {
    /// Certificate for the final session snapshot.
    pub termination: IpmTerminationCertificate,
    /// Every source-selected transition accepted before termination.
    pub records: Vec<Record>,
}

/// Bounded execution of fresh source projections over one IPM session.
#[derive(Debug)]
pub struct Driver<F> {
    session: Session,
    factory: F,
    maximum_iterations: u64,
    records: Vec<Record>,
}

impl<F> Driver<F> {
    /// Starts a bounded source-iteration run from an existing session.
    #[must_use]
    pub fn new(session: Session, factory: F, maximum_iterations: u64) -> Self {
        Self {
            session,
            factory,
            maximum_iterations,
            records: Vec::new(),
        }
    }

    /// Returns the current session, including any accepted transitions.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// Returns accepted transitions in their deterministic execution order.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Returns the source-projection policy and its immutable state.
    #[must_use]
    pub const fn factory(&self) -> &F {
        &self.factory
    }

    /// Returns the session after the caller has finished inspecting the run.
    #[must_use]
    pub fn into_session(self) -> Session {
        self.session
    }
}

impl<F: Factory> Driver<F> {
    /// Drives fresh source projections until additive-half termination.
    ///
    /// The explicit iteration limit prevents an uncertified nonterminating
    /// source policy from being mistaken for a solver. Factory and selection
    /// failures leave the session at its last accepted snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid terminal certificate, projection,
    /// source selection, certified update, or exhausted iteration limit.
    pub fn run(&mut self, network: &CirculationNetwork) -> Result<Completion, Error> {
        loop {
            match self
                .session
                .snapshot()
                .certify_additive_half_termination(network)
            {
                Ok(termination) => {
                    return Ok(Completion {
                        termination,
                        records: self.records.clone(),
                    });
                }
                Err(CertifiedIpmError::NotAtAdditiveHalfBoundary) => {}
                Err(error) => return Err(Error::Ipm(error)),
            }
            if u64::try_from(self.records.len()).map_err(|_| Error::IterationCountOverflow)?
                >= self.maximum_iterations
            {
                return Err(Error::IterationLimit {
                    maximum_iterations: self.maximum_iterations,
                });
            }

            let projection = self.factory.prepare(self.session.snapshot(), network)?;
            let sequence =
                u64::try_from(self.records.len()).map_err(|_| Error::IterationCountOverflow)?;
            let record = Record {
                sequence,
                snapshot: projection.snapshot.clone(),
                input: projection.input.clone(),
                approximation: projection.approximation,
                selected: self
                    .session
                    .apply_source_selected(network, projection.selected())?,
            };
            self.records.push(record);
        }
    }
}

impl Step {
    /// Forms an exact update request from the best source-declared candidate
    /// across one matching terminal and rejected-core population.
    ///
    /// The two populations are deliberately evaluated in their own immutable
    /// tree-chain contexts. Only their exact scores and stable IDs are compared;
    /// the winning compact cycle is decoded through the context that produced
    /// it. This combines maintained declarations without graph-cycle
    /// enumeration or a reference-backend fallback.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshots do not share one exact input, their
    /// candidate IDs overlap, their input coordinates differ from the caller,
    /// either source snapshot is stale, or the selected compact cycle cannot
    /// be decoded.
    pub fn from_maintained_candidates(
        ledger: &StableMinRatioLedger,
        terminal: &TerminalTree,
        spanner: &SpannerSnapshot,
        network: &CirculationNetwork,
        approximate_gradients: Vec<ExactRatio>,
        approximate_lengths: Vec<ExactRatio>,
        kappa: ExactRatio,
    ) -> Result<Option<Self>, Error> {
        if terminal.input() != spanner.input() {
            return Err(Error::MismatchedCandidateSnapshots);
        }
        validate_coordinates(
            terminal.input(),
            &approximate_gradients,
            &approximate_lengths,
        )?;
        terminal.verify(network)?;
        spanner.verify(network)?;

        let mut terminal_registry = terminal.registry(network)?;
        let mut spanner_registry = spanner.registry(network)?;
        validate_disjoint_ids(&terminal_registry, &spanner_registry)?;

        match select_population_choice(terminal_registry.best()?, spanner_registry.best()?)? {
            Some(PopulationChoice::Terminal(choice)) => Ok(Some(Self::from_compact_candidate(
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
            )?)),
            Some(PopulationChoice::Spanner(choice)) => Ok(Some(Self::from_compact_candidate(
                CompactCandidate {
                    ledger,
                    cycle: &choice.cycle,
                    graph: &spanner.materialization().graph,
                    chain: spanner.chain(),
                    shifts: spanner.shifts(),
                    bindings: &spanner.materialization().bindings,
                },
                network,
                approximate_gradients,
                approximate_lengths,
                kappa,
            )?)),
            None => Ok(None),
        }
    }

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
        validate_coordinates(
            terminal.input(),
            &approximate_gradients,
            &approximate_lengths,
        )?;
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

impl SourceSelected<'_> {
    /// Selects the best maintained candidate using this exact source projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the declared source input differs from either
    /// population, their candidate contexts reject, or no nonzero source
    /// candidate is available.
    fn step(self, network: &CirculationNetwork) -> Result<Step, Error> {
        if self.input != self.terminal.input() || self.input != self.spanner.input() {
            return Err(Error::MismatchedSelectedInput);
        }
        let gradients = self.input.arcs().iter().map(|arc| arc.gradient).collect();
        let lengths = self.input.arcs().iter().map(|arc| arc.length).collect();
        Step::from_maintained_candidates(
            self.ledger,
            self.terminal,
            self.spanner,
            network,
            gradients,
            lengths,
            self.kappa,
        )?
        .ok_or(Error::NoSourceCandidate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PopulationChoice {
    Terminal(Choice),
    Spanner(Choice),
}

fn validate_coordinates(
    input: &Input,
    gradients: &[ExactRatio],
    lengths: &[ExactRatio],
) -> Result<(), Error> {
    let expected_gradients = input.arcs().iter().map(|arc| arc.gradient);
    let expected_lengths = input.arcs().iter().map(|arc| arc.length);
    if !gradients.iter().copied().eq(expected_gradients)
        || !lengths.iter().copied().eq(expected_lengths)
    {
        return Err(Error::MismatchedCandidateCoordinates);
    }
    Ok(())
}

fn validate_disjoint_ids(terminal: &Registry, spanner: &Registry) -> Result<(), Error> {
    let terminal_ids = terminal
        .candidates()
        .into_iter()
        .map(|candidate| candidate.id)
        .collect::<BTreeSet<_>>();
    for candidate in spanner.candidates() {
        if terminal_ids.contains(&candidate.id) {
            return Err(Error::DuplicateCandidateId(candidate.id));
        }
    }
    Ok(())
}

fn select_population_choice(
    terminal: Option<Choice>,
    spanner: Option<Choice>,
) -> Result<Option<PopulationChoice>, Error> {
    let Some(terminal) = terminal else {
        return Ok(spanner.map(PopulationChoice::Spanner));
    };
    let Some(spanner) = spanner else {
        return Ok(Some(PopulationChoice::Terminal(terminal)));
    };
    if terminal.quality == spanner.quality {
        return Ok(Some(if terminal.id < spanner.id {
            PopulationChoice::Terminal(terminal)
        } else {
            PopulationChoice::Spanner(spanner)
        }));
    }
    if terminal.quality.at_least(spanner.quality)? {
        Ok(Some(PopulationChoice::Terminal(terminal)))
    } else {
        Ok(Some(PopulationChoice::Spanner(spanner)))
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

/// The selected source declaration and its accepted certified transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedOutcome {
    /// Exact direction decoded from the winning source candidate.
    pub step: Step,
    /// Certified IPM transition outcome.
    pub outcome: Outcome,
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

    /// Selects and applies one direction from matching source-maintained state.
    ///
    /// The supplied source projection is deliberately not reconstructed from
    /// IPM intervals. Its exact coordinates are selected from `Input`, then
    /// passed through [`Self::apply`], which certifies the approximation and
    /// records Detect accounting before committing the successor snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the request belongs to another certified snapshot,
    /// its source inputs disagree, no candidate is available, or the existing
    /// certified update transition rejects the selected direction.
    pub fn apply_source_selected(
        &mut self,
        network: &CirculationNetwork,
        selected: SourceSelected<'_>,
    ) -> Result<SelectedOutcome, Error> {
        if &self.snapshot != selected.snapshot {
            return Err(Error::StaleCertifiedSnapshot);
        }
        let step = selected.step(network)?;
        let outcome = self.apply(network, &step)?;
        Ok(SelectedOutcome { step, outcome })
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
    #[error("rejected-core source candidate failed: {0}")]
    Spanner(#[from] SpannerError),
    #[error("terminal candidate heap failed: {0}")]
    Candidate(#[from] CandidateError),
    #[error("terminal and rejected-core snapshots have different exact source inputs")]
    MismatchedCandidateSnapshots,
    #[error("selected source input does not match both maintained populations")]
    MismatchedSelectedInput,
    #[error("prepared source input does not match both maintained populations")]
    MismatchedPreparedInput,
    #[error("caller approximation coordinates differ from the source input")]
    MismatchedCandidateCoordinates,
    #[error("selected source state belongs to a stale certified IPM snapshot")]
    StaleCertifiedSnapshot,
    #[error("the maintained source populations contain no nonzero candidate")]
    NoSourceCandidate,
    #[error("terminal and rejected-core populations reuse candidate ID {0:?}")]
    DuplicateCandidateId(CandidateId),
    #[error("decoded compact cycle refers to an invalid circulation arc")]
    InvalidArc,
    #[error("source iteration reached its explicit limit of {maximum_iterations} updates")]
    IterationLimit { maximum_iterations: u64 },
    #[error("source iteration record count exceeds the supported exact domain")]
    IterationCountOverflow,
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::BTreeSet, rc::Rc};

    use super::{
        CompactCandidate, Driver, Error, FixedProjectionFactory, PopulationChoice, Projection,
        Session, SourceSelected, Step, select_population_choice,
    };
    use crate::{
        CertifiedIpmSnapshot, CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId,
        FractionalCirculation, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge, StableEdge,
        StableMinRatioLedger, StableWitness,
        source_min_ratio::{
            candidate::{CandidateId, Choice, Kind},
            chain::Chain,
            cycle::{ArcBindings, Cycle, Direction, Segment},
            input::Input,
            model::{Branch, BranchId, Level, LevelId, Tree},
            spanner::{Parameters, Snapshot as SpannerSnapshot},
            terminal::Tree as TerminalTree,
        },
        source_spanner::experiment::domain::ExhaustiveDomain,
    };

    fn ratio(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    fn ledger() -> StableMinRatioLedger {
        StableMinRatioLedger::new(
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
        .unwrap()
    }

    fn complete_network() -> CirculationNetwork {
        let mut network = CirculationNetwork::new(5);
        for first in 0..5 {
            for second in (first + 1)..5 {
                network
                    .add_arc(FlowNodeId(first), FlowNodeId(second), 1, 0)
                    .unwrap();
            }
        }
        network
    }

    fn selected_source_network() -> CirculationNetwork {
        let mut network = CirculationNetwork::new(5);
        let edges = [
            (0, 1, 4, 1),
            (1, 2, 2, 0),
            (2, 3, 2, 0),
            (3, 4, 2, 0),
            (4, 0, 4, 0),
            (0, 2, 2, 0),
            (2, 4, 2, 0),
            (1, 3, 2, 0),
            (3, 0, 2, 0),
        ];
        for (first, second, capacity, cost) in edges {
            network
                .add_arc(
                    FlowNodeId(first),
                    FlowNodeId(second),
                    capacity,
                    i128::from(cost),
                )
                .unwrap();
        }
        network
    }

    fn spanner_parameters() -> Parameters {
        Parameters {
            root: FlowNodeId(0),
            maximum_absolute_exponent: 4,
            phi: ExactRatio::new(1, 2).unwrap(),
            domain: ExhaustiveDomain { maximum_nodes: 8 },
            maximum_hops: 4,
            maximum_vertex_congestion: 100,
            maximum_rounds: 1,
        }
    }

    fn selected_iteration_fixture() -> (
        CirculationNetwork,
        CertifiedIpmSnapshot,
        Input,
        TerminalTree,
        SpannerSnapshot,
        StableMinRatioLedger,
    ) {
        let network = selected_source_network();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &FractionalCirculation {
                arc_flows: vec![
                    ratio(2),
                    ratio(1),
                    ratio(1),
                    ratio(1),
                    ratio(2),
                    ratio(1),
                    ratio(1),
                    ratio(1),
                    ratio(1),
                ],
                cost: ratio(2),
            },
            ratio(0),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        let mut gradients = vec![ratio(0); network.arc_count()];
        gradients[0] = ratio(90);
        let mut lengths = vec![ratio(2); network.arc_count()];
        lengths[0] = ratio(1);
        lengths[4] = ratio(1);
        let input = Input::new(&network, &gradients, &lengths, &lengths).unwrap();
        let terminal = TerminalTree::build(input.clone(), &network, FlowNodeId(0)).unwrap();
        let spanner =
            SpannerSnapshot::build(input.clone(), &network, spanner_parameters()).unwrap();
        (network, snapshot, input, terminal, spanner, ledger())
    }

    fn projection_for(
        snapshot: CertifiedIpmSnapshot,
        input: Input,
        network: &CirculationNetwork,
    ) -> Projection {
        let terminal = TerminalTree::build(input.clone(), network, FlowNodeId(0)).unwrap();
        let spanner = SpannerSnapshot::build(input.clone(), network, spanner_parameters()).unwrap();
        Projection::new(
            snapshot,
            input,
            ledger(),
            terminal,
            spanner,
            ExactRatio::new(1, 2).unwrap(),
            network,
        )
        .unwrap()
    }

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
            Err(Error::MismatchedCandidateCoordinates)
        );
    }

    #[test]
    fn selects_the_best_complete_population_without_a_reference_fallback() {
        let network = complete_network();
        let gradients = vec![ratio(-1); network.arc_count()];
        let lengths = vec![ratio(1); network.arc_count()];
        let input = Input::new(&network, &gradients, &lengths, &lengths).unwrap();
        let terminal = TerminalTree::build(input.clone(), &network, FlowNodeId(0)).unwrap();
        let spanner = SpannerSnapshot::build(input, &network, spanner_parameters()).unwrap();
        assert!(!terminal.candidates().is_empty());
        assert!(!spanner.candidates().is_empty());

        let mut terminal_registry = terminal.registry(&network).unwrap();
        let mut spanner_registry = spanner.registry(&network).unwrap();
        let terminal_choice = terminal_registry.best().unwrap().unwrap();
        let spanner_choice = spanner_registry.best().unwrap().unwrap();
        let expected_choice = if terminal_choice.quality == spanner_choice.quality {
            if terminal_choice.id < spanner_choice.id {
                PopulationChoice::Terminal(terminal_choice)
            } else {
                PopulationChoice::Spanner(spanner_choice)
            }
        } else if terminal_choice
            .quality
            .at_least(spanner_choice.quality)
            .unwrap()
        {
            PopulationChoice::Terminal(terminal_choice)
        } else {
            PopulationChoice::Spanner(spanner_choice)
        };
        let expected_ledger = ledger();
        let expected = match expected_choice {
            PopulationChoice::Terminal(choice) => Step::from_compact_candidate(
                CompactCandidate {
                    ledger: &expected_ledger,
                    cycle: &choice.cycle,
                    graph: &terminal.materialization().graph,
                    chain: terminal.chain(),
                    shifts: terminal.shifts(),
                    bindings: &terminal.materialization().bindings,
                },
                &network,
                gradients.clone(),
                lengths.clone(),
                ExactRatio::new(1, 2).unwrap(),
            )
            .unwrap(),
            PopulationChoice::Spanner(choice) => Step::from_compact_candidate(
                CompactCandidate {
                    ledger: &expected_ledger,
                    cycle: &choice.cycle,
                    graph: &spanner.materialization().graph,
                    chain: spanner.chain(),
                    shifts: spanner.shifts(),
                    bindings: &spanner.materialization().bindings,
                },
                &network,
                gradients.clone(),
                lengths.clone(),
                ExactRatio::new(1, 2).unwrap(),
            )
            .unwrap(),
        };
        let actual_ledger = ledger();

        assert_eq!(
            Step::from_maintained_candidates(
                &actual_ledger,
                &terminal,
                &spanner,
                &network,
                gradients,
                lengths,
                ExactRatio::new(1, 2).unwrap(),
            ),
            Ok(Some(expected))
        );
    }

    #[test]
    fn rejects_complete_populations_from_different_source_snapshots() {
        let network = complete_network();
        let gradients = vec![ratio(-1); network.arc_count()];
        let lengths = vec![ratio(1); network.arc_count()];
        let terminal_input = Input::new(&network, &gradients, &lengths, &lengths).unwrap();
        let mut changed_gradients = gradients.clone();
        changed_gradients[0] = ratio(-2);
        let spanner_input = Input::new(&network, &changed_gradients, &lengths, &lengths).unwrap();
        let terminal = TerminalTree::build(terminal_input, &network, FlowNodeId(0)).unwrap();
        let spanner =
            SpannerSnapshot::build(spanner_input, &network, spanner_parameters()).unwrap();
        let actual_ledger = ledger();

        assert_eq!(
            Step::from_maintained_candidates(
                &actual_ledger,
                &terminal,
                &spanner,
                &network,
                gradients,
                lengths,
                ExactRatio::new(1, 2).unwrap(),
            ),
            Err(Error::MismatchedCandidateSnapshots)
        );
    }

    #[test]
    fn selects_from_matching_terminal_and_core_successor_snapshots() {
        let network = complete_network();
        let initial_gradients = vec![ratio(-1); network.arc_count()];
        let next_gradients = vec![ratio(1); network.arc_count()];
        let lengths = vec![ratio(1); network.arc_count()];
        let initial = Input::new(&network, &initial_gradients, &lengths, &lengths).unwrap();
        let next = Input::new(&network, &next_gradients, &lengths, &lengths).unwrap();
        let terminal = TerminalTree::build(initial.clone(), &network, FlowNodeId(0)).unwrap();
        let spanner = SpannerSnapshot::build(initial, &network, spanner_parameters()).unwrap();
        let terminal_transition = terminal.transition(next.clone(), &network).unwrap();
        let spanner_transition = spanner.transition(next, &network).unwrap();
        let actual_ledger = ledger();

        assert_eq!(
            terminal_transition.refreshed.len(),
            terminal.candidates().len()
        );
        assert_eq!(
            spanner_transition.refreshed.len(),
            spanner.candidates().len()
        );
        assert!(
            Step::from_maintained_candidates(
                &actual_ledger,
                &terminal_transition.next,
                &spanner_transition.next,
                &network,
                next_gradients,
                lengths,
                ExactRatio::new(1, 2).unwrap(),
            )
            .unwrap()
            .is_some()
        );
    }

    #[test]
    fn resolves_equal_complete_population_quality_by_stable_candidate_id() {
        let choice = |id| Choice {
            id: CandidateId(id),
            kind: Kind::FundamentalTree {
                source: SourceEdgeId(id),
            },
            cycle: Cycle {
                segments: Vec::new(),
            },
            quality: ratio(1),
            gradient_dot: ratio(-1),
            length_norm: ratio(1),
        };
        assert!(matches!(
            select_population_choice(Some(choice(8)), Some(choice(7))).unwrap(),
            Some(PopulationChoice::Spanner(selected)) if selected.id == CandidateId(7)
        ));
    }

    #[test]
    fn applies_one_source_selected_certified_iteration() {
        let (network, snapshot, input, terminal, spanner, ledger) = selected_iteration_fixture();
        let mut session = Session::new(snapshot.clone()).unwrap();
        let selected = SourceSelected {
            snapshot: &snapshot,
            input: &input,
            ledger: &ledger,
            terminal: &terminal,
            spanner: &spanner,
            kappa: ExactRatio::new(1, 2).unwrap(),
        };

        let accepted = session.apply_source_selected(&network, selected).unwrap();

        assert_eq!(accepted.outcome.eta, ExactRatio::new(1, 18_000).unwrap());
        assert!(
            accepted
                .step
                .direction
                .iter()
                .any(|coordinate| !coordinate.is_zero())
        );
        assert_eq!(session.snapshot().update_metrics().iterations, 1);
        assert_eq!(session.detect_metrics().iterations, 0);
    }

    #[test]
    fn rejects_stale_or_mismatched_selected_source_without_state_change() {
        let (network, snapshot, input, terminal, spanner, ledger) = selected_iteration_fixture();
        let selected = SourceSelected {
            snapshot: &snapshot,
            input: &input,
            ledger: &ledger,
            terminal: &terminal,
            spanner: &spanner,
            kappa: ExactRatio::new(1, 2).unwrap(),
        };
        let mut session = Session::new(snapshot.clone()).unwrap();
        session.apply_source_selected(&network, selected).unwrap();
        let after_update = session.snapshot().clone();
        assert_eq!(
            session.apply_source_selected(&network, selected),
            Err(Error::StaleCertifiedSnapshot)
        );
        assert_eq!(session.snapshot(), &after_update);

        let mut mismatched_gradients = input
            .arcs()
            .iter()
            .map(|arc| arc.gradient)
            .collect::<Vec<_>>();
        mismatched_gradients[0] = ratio(39);
        let lengths = input
            .arcs()
            .iter()
            .map(|arc| arc.length)
            .collect::<Vec<_>>();
        let mismatched_input =
            Input::new(&network, &mismatched_gradients, &lengths, &lengths).unwrap();
        let mismatched = SourceSelected {
            snapshot: &snapshot,
            input: &mismatched_input,
            ledger: &ledger,
            terminal: &terminal,
            spanner: &spanner,
            kappa: ExactRatio::new(1, 2).unwrap(),
        };
        let mut fresh_session = Session::new(snapshot.clone()).unwrap();
        let before = fresh_session.snapshot().clone();
        assert_eq!(
            fresh_session.apply_source_selected(&network, mismatched),
            Err(Error::MismatchedSelectedInput)
        );
        assert_eq!(fresh_session.snapshot(), &before);
    }

    #[test]
    fn certifies_each_prepared_projection_before_selection() {
        let (network, snapshot, input, _, _, _) = selected_iteration_fixture();
        let projection = projection_for(snapshot.clone(), input.clone(), &network);

        assert_eq!(projection.snapshot(), &snapshot);
        assert_eq!(projection.input(), &input);
        assert_eq!(projection.approximation().edge_count, network.arc_count());
        assert_eq!(
            projection.approximation().factor_two_length_checks,
            u64::try_from(network.arc_count()).unwrap()
        );
        assert_eq!(
            projection.approximation().scaled_gradient_checks,
            u64::try_from(network.arc_count()).unwrap()
        );
    }

    #[test]
    fn drives_fresh_projections_until_the_explicit_limit() {
        let (network, snapshot, input, _, _, _) = selected_iteration_fixture();
        let preparations = Rc::new(Cell::new(0));
        let observed = Rc::clone(&preparations);
        let factory = move |current: &CertifiedIpmSnapshot, active: &CirculationNetwork| {
            observed.set(observed.get() + 1);
            Ok(projection_for(current.clone(), input.clone(), active))
        };
        let mut driver = Driver::new(Session::new(snapshot).unwrap(), factory, 2);

        assert_eq!(
            driver.run(&network),
            Err(Error::IterationLimit {
                maximum_iterations: 2
            })
        );
        assert_eq!(preparations.get(), 2);
        assert_eq!(driver.records().len(), 2);
        assert_eq!(driver.records()[0].sequence, 0);
        assert_eq!(driver.records()[1].sequence, 1);
        assert_eq!(driver.session().snapshot().update_metrics().iterations, 2);
        assert_eq!(driver.records()[0].input, driver.records()[1].input);
    }

    #[test]
    fn fixed_factory_rebuilds_and_recertifies_each_supported_snapshot() {
        let (network, snapshot, input, _, _, _) = selected_iteration_fixture();
        let factory = FixedProjectionFactory::new(
            input,
            ledger(),
            spanner_parameters(),
            ExactRatio::new(1, 2).unwrap(),
        );
        let mut driver = Driver::new(Session::new(snapshot).unwrap(), factory, 2);

        assert_eq!(
            driver.run(&network),
            Err(Error::IterationLimit {
                maximum_iterations: 2
            })
        );
        assert_eq!(driver.factory().preparation_count(), 2);
        assert_eq!(driver.records().len(), 2);
        assert_ne!(driver.records()[0].snapshot, driver.records()[1].snapshot);
        assert_eq!(
            driver.records()[0].approximation.edge_count,
            network.arc_count()
        );
        assert_eq!(
            driver.records()[1].approximation.edge_count,
            network.arc_count()
        );
    }

    #[test]
    fn rejects_a_reused_projection_after_the_first_update_without_mutation() {
        let (network, snapshot, input, _, _, _) = selected_iteration_fixture();
        let stale = projection_for(snapshot.clone(), input, &network);
        let factory = move |_: &CertifiedIpmSnapshot, _: &CirculationNetwork| Ok(stale.clone());
        let mut driver = Driver::new(Session::new(snapshot).unwrap(), factory, 2);

        assert_eq!(driver.run(&network), Err(Error::StaleCertifiedSnapshot));
        assert_eq!(driver.records().len(), 1);
        assert_eq!(driver.session().snapshot().update_metrics().iterations, 1);
    }

    #[test]
    fn stops_without_requesting_a_projection_at_additive_half_termination() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let quarter = ExactRatio::new(1, 4).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &FractionalCirculation {
                arc_flows: vec![quarter; 2],
                cost: quarter,
            },
            ExactRatio::new(0, 1).unwrap(),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        let factory = |_: &CertifiedIpmSnapshot, _: &CirculationNetwork| {
            Err::<Projection, Error>(Error::NoSourceCandidate)
        };
        let mut driver = Driver::new(Session::new(snapshot).unwrap(), factory, 0);

        let completion = driver.run(&network).unwrap();
        assert!(completion.records.is_empty());
        assert!(driver.records().is_empty());
        assert_eq!(driver.session().snapshot().update_metrics().iterations, 0);
    }
}
