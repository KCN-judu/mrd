# P09.5 - Source-Shaped Flow Integration Gap

The certified IPM exposes public snapshots, update metrics, additive-half
termination, and recovery APIs. Inspection establishes that the existing
`CertifiedIpmSnapshot::recover_additive_half` invokes the permanent exact
rounding Oracle. The residual refinement experiment likewise calls an
enumerating residual-cycle Oracle.

Neither operation may be called by `source_flow::Backend`. The new backend
boundary therefore rejects execution explicitly and reports
`an19_runtime_verified: false`; it does not silently choose Dinic,
Push--Relabel, or an Oracle. Completing P9.5 requires a separately certified,
non-Oracle iterative/recovery path before any end-to-end exact-flow claim.
