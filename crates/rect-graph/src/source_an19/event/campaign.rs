use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::model::{
    ChargeAnalysis, Context, Count, HierarchyMetrics, Problem, Ratio, Run, RuntimeStatus, Segment,
};
use crate::{
    ExactRatio, FlowNodeId, SourceDynamicGraph, SourceWeightedEdge,
    source_an19::{
        experiment,
        petal::{Error, ratio},
    },
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    ManyReducedCostsFewSourceLengths,
    RepeatedPortalSplitting,
    FullDepthPersistence,
    AllEqualReducedKeys,
    AllDistinctReducedKeys,
    AlternatingPartitionContraction,
    HighwayHalvingReorder,
    VirtualRealMixedSegments,
}

impl Family {
    pub const ALL: [Self; 8] = [
        Self::ManyReducedCostsFewSourceLengths,
        Self::RepeatedPortalSplitting,
        Self::FullDepthPersistence,
        Self::AllEqualReducedKeys,
        Self::AllDistinctReducedKeys,
        Self::AlternatingPartitionContraction,
        Self::HighwayHalvingReorder,
        Self::VirtualRealMixedSegments,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ManyReducedCostsFewSourceLengths => "many_reduced_costs_few_source_lengths",
            Self::RepeatedPortalSplitting => "repeated_portal_splitting",
            Self::FullDepthPersistence => "full_depth_persistence",
            Self::AllEqualReducedKeys => "all_equal_reduced_keys",
            Self::AllDistinctReducedKeys => "all_distinct_reduced_keys",
            Self::AlternatingPartitionContraction => "alternating_partition_contraction",
            Self::HighwayHalvingReorder => "highway_halving_reorder",
            Self::VirtualRealMixedSegments => "virtual_real_mixed_segments",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub input_family: Family,
    pub size_parameter: usize,
    pub logical_call_index: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub original_length_classes: u64,
    pub symbolic_source_label_classes: u64,
    pub symbolic_virtual_label_classes: u64,
    pub materialized_length_classes: u64,
    pub distinct_reduced_costs: u64,
    pub distinct_event_radii: u64,
    pub total_events: u64,
    pub events_per_source_depth_maximum: u64,
    pub events_per_lineage_maximum: u64,
    pub queue_insertions: u64,
    pub queue_pops: u64,
    pub exact_comparisons: u64,
    pub stale_events: u64,
    pub oracle_agreement: bool,
    pub selected_radius: Ratio,
    pub charge_analyses: Vec<ChargeAnalysis>,
    pub oracle_run: Run,
    pub reduced_run: Run,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub schema_version: u32,
    pub commit_sha: String,
    pub command_line: String,
    pub seed: Option<u64>,
    pub cases: Vec<Case>,
    pub aggregate: HierarchyMetrics,
    pub naive_reduced_class_conversion_survived: bool,
    pub runtime_status: RuntimeStatus,
}

impl Campaign {
    /// Runs deterministic fixed-snapshot campaigns. Growth measurements remain
    /// proof-discovery evidence; the local event-cardinality flag comes only
    /// from the structural certificate verified for every run.
    ///
    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, trace, or differential error.
    pub fn run(
        families: &[Family],
        sizes: &[usize],
        commit_sha: String,
        command_line: String,
    ) -> Result<Self, Error> {
        if families.is_empty() || sizes.is_empty() {
            return Err(Error::InvalidDomain);
        }
        let mut cases = Vec::new();
        for family in families {
            for size in sizes {
                for (logical_call_index, owned) in adversarial_problems(*family, *size)?
                    .into_iter()
                    .enumerate()
                {
                    let problem = owned.as_problem();
                    let (oracle, reduced) = experiment::event::Engine::run_differential(&problem)?;
                    if !oracle.semantically_agrees(&reduced) {
                        return Err(Error::InvalidEventTrace);
                    }
                    let source_depth_maximum =
                        maximum_count(&reduced.metrics.events_per_source_edge);
                    let lineage_maximum =
                        maximum_count(&reduced.metrics.events_per_segment_lineage);
                    cases.push(Case {
                        input_family: *family,
                        size_parameter: *size,
                        logical_call_index,
                        graph_nodes: owned.graph.node_count(),
                        graph_edges: owned.graph.edge_count(),
                        original_length_classes: reduced.metrics.original_length_class_count,
                        symbolic_source_label_classes: reduced
                            .metrics
                            .symbolic_source_label_class_count,
                        symbolic_virtual_label_classes: reduced
                            .metrics
                            .symbolic_virtual_label_class_count,
                        materialized_length_classes: reduced
                            .metrics
                            .materialized_exact_length_class_count,
                        distinct_reduced_costs: reduced.metrics.distinct_reduced_cost_count,
                        distinct_event_radii: reduced.metrics.distinct_event_radius_count,
                        total_events: u64::try_from(reduced.semantic_trace.len())
                            .map_err(|_| Error::Overflow)?,
                        events_per_source_depth_maximum: source_depth_maximum,
                        events_per_lineage_maximum: lineage_maximum,
                        queue_insertions: reduced.metrics.inserted_queue_item_count,
                        queue_pops: reduced.metrics.popped_queue_item_count,
                        exact_comparisons: reduced.metrics.exact_comparison_count,
                        stale_events: reduced.metrics.stale_queue_item_count,
                        oracle_agreement: true,
                        selected_radius: reduced.selected_radius,
                        charge_analyses: reduced.charge_analyses.clone(),
                        oracle_run: oracle,
                        reduced_run: reduced,
                    });
                }
            }
        }
        let aggregate = aggregate_campaign(&cases)?;
        set_growth_observations(&mut cases);
        let naive_reduced_class_conversion_survived = !cases.iter().any(|case| {
            case.input_family == Family::ManyReducedCostsFewSourceLengths
                && case.distinct_reduced_costs > case.original_length_classes.saturating_mul(4)
        });
        Ok(Self {
            schema_version: 1,
            commit_sha,
            command_line,
            seed: None,
            cases,
            aggregate,
            naive_reduced_class_conversion_survived,
            runtime_status: RuntimeStatus::exact_traced(true),
        })
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut output = vec![
            "# AN19 exact event adversarial campaign".to_owned(),
            String::new(),
            format!("- Commit: `{}`", self.commit_sha),
            format!("- Cases: {}", self.cases.len()),
            format!(
                "- Naive reduced-class conversion survived: {}",
                self.naive_reduced_class_conversion_survived
            ),
            "- Fixed-snapshot event-cardinality bound proved: true".to_owned(),
            "- Practical stable-binary-heap comparison bound certified: true".to_owned(),
            "- Priority-queue comparison bound proved: false".to_owned(),
            "- AN19 runtime verified: false".to_owned(),
            String::new(),
            "| family | size | call | nodes | edges | original classes | reduced costs | event radii | events | comparisons | practical bound | stale | Oracle |".to_owned(),
            "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |".to_owned(),
        ];
        for case in &self.cases {
            let practical_bound = case
                .reduced_run
                .practical_queue_bound
                .map_or(0, |certificate| certificate.total_comparison_bound);
            output.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                case.input_family.name(),
                case.size_parameter,
                case.logical_call_index,
                case.graph_nodes,
                case.graph_edges,
                case.original_length_classes,
                case.distinct_reduced_costs,
                case.distinct_event_radii,
                case.total_events,
                case.exact_comparisons,
                practical_bound,
                case.stale_events,
                case.oracle_agreement,
            ));
        }
        output.extend([
            String::new(),
            "Each run carries a verified fixed-snapshot certificate: semantic events are at most 3n + 4m + 2 and queue insertions/pops are at most n + 2m + 2. Each reduced-engine run separately certifies the practical stable binary heap bound 3 I ceil(log2(max(I, 1))) + 2m on its counted heap and relaxation-label comparisons; Oracle runs do not carry that implementation certificate. This is an O((n+m) log(n+m)) practical bound, not the source-equivalent O(m+n log log n) priority-queue proof, hierarchy-wide amortization, or AN19 runtime.".to_owned(),
        ]);
        output.join("\n") + "\n"
    }
}

fn maximum_count(counts: &[Count]) -> u64 {
    counts.iter().map(|entry| entry.count).max().unwrap_or(0)
}

fn set_growth_observations(cases: &mut [Case]) {
    for family in Family::ALL {
        let family_cases = cases
            .iter()
            .filter(|case| case.input_family == family)
            .collect::<Vec<_>>();
        let grows = family_cases.windows(2).any(|pair| {
            pair[1].size_parameter > pair[0].size_parameter
                && pair[1].events_per_source_depth_maximum > pair[0].events_per_source_depth_maximum
        });
        for case in cases.iter_mut().filter(|case| case.input_family == family) {
            for analysis in &mut case.charge_analyses {
                analysis.observed_growth_with_input_size = Some(grows);
            }
            case.reduced_run.charge_analyses = case.charge_analyses.clone();
        }
    }
}

fn aggregate_campaign(cases: &[Case]) -> Result<HierarchyMetrics, Error> {
    let mut by_depth = BTreeMap::<String, u64>::new();
    let mut by_source = BTreeMap::<String, u64>::new();
    let mut source_depth = BTreeMap::<(String, String), u64>::new();
    let mut lineage = BTreeMap::<String, u64>::new();
    let mut total_events = 0_u64;
    let mut total_reduced = 0_u64;
    let mut total_comparisons = 0_u64;
    let mut total_stale = 0_u64;
    let mut maximum_reduced = 0_u64;
    for case in cases {
        total_events = total_events
            .checked_add(case.total_events)
            .ok_or(Error::Overflow)?;
        total_reduced = total_reduced
            .checked_add(case.distinct_reduced_costs)
            .ok_or(Error::Overflow)?;
        total_comparisons = total_comparisons
            .checked_add(case.exact_comparisons)
            .ok_or(Error::Overflow)?;
        total_stale = total_stale
            .checked_add(case.stale_events)
            .ok_or(Error::Overflow)?;
        maximum_reduced = maximum_reduced.max(case.distinct_reduced_costs);
        for event in case
            .reduced_run
            .semantic_trace
            .iter()
            .filter(|event| !event.stale)
        {
            increment_string_count(&mut by_depth, event.logical_partition_depth.to_string())?;
            if let Some(source) = event.source_edge_id {
                let source = source.to_string();
                increment_string_count(&mut by_source, source.clone())?;
                let key = (source, event.logical_partition_depth.to_string());
                let count = source_depth.entry(key).or_default();
                *count = count.checked_add(1).ok_or(Error::Overflow)?;
            }
            if let Some(lineage_id) = event.segment_lineage_root_id {
                increment_string_count(&mut lineage, lineage_id.to_string())?;
            }
        }
    }
    Ok(HierarchyMetrics {
        total_events_across_logical_calls: total_events,
        maximum_events_for_one_source_edge_at_one_depth: source_depth
            .values()
            .copied()
            .max()
            .unwrap_or(0),
        maximum_events_for_one_source_edge_across_all_depths: by_source
            .values()
            .copied()
            .max()
            .unwrap_or(0),
        maximum_events_for_one_segment_lineage: lineage.values().copied().max().unwrap_or(0),
        maximum_reduced_classes_in_one_snapshot: maximum_reduced,
        total_reduced_classes_across_snapshots: total_reduced,
        total_exact_comparisons: total_comparisons,
        total_stale_events: total_stale,
        total_event_work_grouped_by_logical_depth: count_map_to_vec(by_depth),
        total_event_work_grouped_by_top_level_source_edge: count_map_to_vec(by_source),
    })
}

fn increment_string_count(counts: &mut BTreeMap<String, u64>, key: String) -> Result<(), Error> {
    let count = counts.entry(key).or_default();
    *count = count.checked_add(1).ok_or(Error::Overflow)?;
    Ok(())
}

fn count_map_to_vec(counts: BTreeMap<String, u64>) -> Vec<Count> {
    counts
        .into_iter()
        .map(|(key, count)| Count { key, count })
        .collect()
}

pub(in crate::source_an19) struct OwnedEventProblem {
    pub(in crate::source_an19) graph: SourceDynamicGraph,
    cluster: BTreeSet<FlowNodeId>,
    remaining: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
    context: Context,
    segments: Vec<Segment>,
}

impl OwnedEventProblem {
    pub(in crate::source_an19) fn as_problem(&self) -> Problem<'_> {
        Problem {
            graph: &self.graph,
            cluster: &self.cluster,
            remaining: &self.remaining,
            center: self.center,
            target: self.target,
            budget: self.budget,
            context: self.context,
            segments: &self.segments,
        }
    }
}

pub(in crate::source_an19) fn adversarial_problems(
    family: Family,
    requested_size: usize,
) -> Result<Vec<OwnedEventProblem>, Error> {
    let size = requested_size.max(10);
    match family {
        Family::ManyReducedCostsFewSourceLengths | Family::AllDistinctReducedKeys => {
            Ok(vec![power_of_two_chord_problem(
                size.next_power_of_two().max(16),
                family,
            )?])
        }
        Family::FullDepthPersistence => {
            let depth = usize::try_from(size.ilog2()).map_err(|_| Error::Overflow)?;
            (0..=depth)
                .map(|logical_depth| {
                    path_problem(
                        size,
                        family,
                        u64::try_from(logical_depth).map_err(|_| Error::Overflow)?,
                    )
                })
                .collect()
        }
        Family::AlternatingPartitionContraction => (0..=2)
            .map(|generation| {
                let mut problem = path_problem(
                    size,
                    family,
                    u64::try_from(generation / 2).map_err(|_| Error::Overflow)?,
                )?;
                problem.context.contraction_generation =
                    u64::try_from(generation).map_err(|_| Error::Overflow)?;
                for segment in &mut problem.segments {
                    segment.contraction_generation = problem.context.contraction_generation;
                }
                Ok(problem)
            })
            .collect(),
        Family::HighwayHalvingReorder => (0..=1)
            .map(|projection_generation| path_problem(size, family, projection_generation))
            .collect(),
        Family::RepeatedPortalSplitting
        | Family::AllEqualReducedKeys
        | Family::VirtualRealMixedSegments => Ok(vec![path_problem(size, family, 0)?]),
    }
}

pub(in crate::source_an19) fn path_problem(
    nodes: usize,
    family: Family,
    logical_depth: u64,
) -> Result<OwnedEventProblem, Error> {
    let mut edges = Vec::new();
    let mut total_length = 0_i128;
    for index in 0..nodes - 1 {
        let length = if family == Family::HighwayHalvingReorder {
            if index % 2 == 0 {
                if logical_depth == 0 { 4 } else { 2 }
            } else {
                3
            }
        } else {
            1
        };
        total_length = total_length.checked_add(length).ok_or(Error::Overflow)?;
        edges.push(SourceWeightedEdge {
            first: FlowNodeId(index),
            second: FlowNodeId(index + 1),
            length: ExactRatio::new(length, 1).map_err(|_| Error::Overflow)?,
            weight: ratio(1, 1)?,
        });
    }
    let maximum_coordinate =
        i128::try_from(nodes.saturating_mul(64)).map_err(|_| Error::Overflow)?;
    let graph = SourceDynamicGraph::new(nodes, edges, maximum_coordinate)
        .map_err(|_| Error::InvalidDomain)?;
    let mut segments = Segment::from_graph(&graph)?;
    let portal_generation = if family == Family::RepeatedPortalSplitting {
        u64::try_from(nodes - 1).map_err(|_| Error::Overflow)?
    } else {
        0
    };
    for (index, segment) in segments.iter_mut().enumerate() {
        if family == Family::RepeatedPortalSplitting {
            segment.source_edge_id = Some(0);
            segment.segment_lineage_root_id = 0;
            segment.portal_split_generation = u64::try_from(index).map_err(|_| Error::Overflow)?;
        }
        if family == Family::VirtualRealMixedSegments && index % 3 == 1 {
            segment.source_edge_id = None;
        }
        if family == Family::HighwayHalvingReorder && logical_depth > 0 && index % 2 == 0 {
            segment.highway_halved = true;
            segment.symbolic_unsplit_rounded_length = ratio(4, 1)?.into();
        }
        segment.projection_generation = logical_depth;
    }
    let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
    Ok(OwnedEventProblem {
        graph,
        cluster: cluster.clone(),
        remaining: cluster,
        center: FlowNodeId(0),
        target: FlowNodeId(nodes - 1),
        budget: ExactRatio::new(total_length.max(3) / 3, 1).map_err(|_| Error::Overflow)?,
        context: Context {
            cluster_id: logical_depth,
            projection_snapshot_id: logical_depth,
            logical_partition_depth: logical_depth,
            recursion_parent_id: logical_depth.checked_sub(1),
            portal_split_generation: portal_generation,
            contraction_generation: 0,
            projection_generation: logical_depth,
        },
        segments,
    })
}

fn power_of_two_chord_problem(nodes: usize, _family: Family) -> Result<OwnedEventProblem, Error> {
    let mut edges = (0..nodes - 1)
        .map(|index| SourceWeightedEdge {
            first: FlowNodeId(index),
            second: FlowNodeId(index + 1),
            length: ratio(1, 1).expect("constant ratio"),
            weight: ratio(1, 1).expect("constant ratio"),
        })
        .collect::<Vec<_>>();
    for index in 0..nodes - 2 {
        let distance = nodes - 1 - index;
        edges.push(SourceWeightedEdge {
            first: FlowNodeId(index),
            second: FlowNodeId(nodes - 1),
            length: ExactRatio::new(
                i128::try_from(distance.next_power_of_two()).map_err(|_| Error::Overflow)?,
                1,
            )
            .map_err(|_| Error::Overflow)?,
            weight: ratio(1, 1)?,
        });
    }
    let maximum_coordinate =
        i128::try_from(nodes.saturating_mul(128)).map_err(|_| Error::Overflow)?;
    let graph = SourceDynamicGraph::new(nodes, edges, maximum_coordinate)
        .map_err(|_| Error::InvalidDomain)?;
    let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
    let segments = Segment::from_graph(&graph)?;
    Ok(OwnedEventProblem {
        graph,
        cluster: cluster.clone(),
        remaining: cluster,
        center: FlowNodeId(0),
        target: FlowNodeId(nodes - 1),
        budget: ExactRatio::new(i128::try_from(nodes / 4).map_err(|_| Error::Overflow)?, 1)
            .map_err(|_| Error::Overflow)?,
        context: Context {
            cluster_id: u64::try_from(nodes).map_err(|_| Error::Overflow)?,
            projection_snapshot_id: u64::try_from(nodes).map_err(|_| Error::Overflow)?,
            logical_partition_depth: 0,
            recursion_parent_id: None,
            portal_split_generation: 0,
            contraction_generation: 0,
            projection_generation: 0,
        },
        segments,
    })
}
