//! Stage 7 bounded iteration. Device laws stay behind `NonlinearDevice`;
//! limiting and continuation never change the reported physical solution.
#[cfg(test)]
#[path = "newton_tests.rs"]
mod tests;
use crate::{
    DiagnosticCode, StableId,
    mna::{MnaSystem, NodeVariables},
    solver::SolverError,
};

pub(crate) const NEWTON_RELTOL: f64 = 1e-9;
pub(crate) const NEWTON_VNTOL: f64 = 1e-9;
pub(crate) const NEWTON_BRANCH_ABSTOL: f64 = 1e-12;
pub(crate) const NEWTON_ITOL: f64 = 1e-15;
pub(crate) const NEWTON_SPEED_ABSTOL: f64 = 1e-9;
pub(crate) const NEWTON_TORQUE_ABSTOL: f64 = 1e-15;
pub(crate) const NEWTON_MAX_ITERATIONS: usize = 100;
pub(crate) const THERMAL_VOLTAGE: f64 = 0.02586492578632875;

#[derive(Clone, Default)]
pub(crate) struct SolverMemory {
    pub nodes: std::collections::HashMap<usize, f64>,
    pub junctions: std::collections::HashMap<StableId, Vec<f64>>,
}

/// A junction voltage is positive minus negative, including the PNP polarity.
#[derive(Clone, Copy)]
pub(crate) struct Junction {
    pub positive: usize,
    pub negative: usize,
    pub thermal_voltage: f64,
    pub saturation_current: f64,
}

/// Terms are kept separate for the physical residual's rounding scale.
pub(crate) struct TerminalCurrent {
    pub node: usize,
    pub terms: Vec<f64>,
}

pub(crate) struct Evaluation {
    pub currents: Vec<TerminalCurrent>,
    /// (terminal row node, terminal column node, dI_row / dV_column).
    pub derivatives: Vec<(usize, usize, f64)>,
}

pub(crate) trait NonlinearDevice {
    fn id(&self) -> &StableId;
    fn junctions(&self) -> Vec<Junction>;
    /// All junction voltages supplied here may be limited during stamping, but
    /// residual evaluation always supplies the unlimited solution differences.
    fn evaluate(&self, junction_voltages: &[f64]) -> Evaluation;
}

/// exp(x)-1 and its derivative; tangent continuation is used only for iteration.
pub(crate) fn junction_exponential(x: f64) -> (f64, f64) {
    if x <= 80.0 {
        (x.exp_m1(), x.exp())
    } else {
        let e = 80.0_f64.exp();
        (e * (1.0 + x - 80.0) - 1.0, e)
    }
}

pub(crate) fn limit_junction(new: f64, old: f64, j: Junction) -> f64 {
    let t = j.thermal_voltage;
    let critical = t * (t / (std::f64::consts::SQRT_2 * j.saturation_current)).ln();
    if new > critical && (new - old).abs() > 2.0 * t {
        if old > 0.0 {
            let arg = 1.0 + (new - old) / t;
            if arg > 0.0 {
                old + t * arg.ln()
            } else {
                critical
            }
        } else {
            t * (new / t).ln()
        }
    } else {
        new
    }
}

#[derive(Debug)]
pub(crate) struct SolveFailure {
    pub code: DiagnosticCode,
    /// Empty means reference every component in the island, as for linear LU failure.
    pub components: Vec<StableId>,
}

fn failure(code: DiagnosticCode, devices: &[&dyn NonlinearDevice]) -> SolveFailure {
    let mut components = devices.iter().map(|d| d.id().clone()).collect::<Vec<_>>();
    components.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    components.dedup();
    SolveFailure { code, components }
}

#[derive(Clone, Copy)]
pub(crate) struct UnknownTolerance {
    pub update: f64,
    pub residual: f64,
}

pub(crate) fn solve(
    base: &MnaSystem,
    variables: &NodeVariables,
    devices: &[&dyn NonlinearDevice],
    tolerance: &[UnknownTolerance],
    initial: Vec<f64>,
    initial_junctions: Vec<Vec<f64>>,
) -> Result<(Vec<f64>, Vec<Vec<f64>>), SolveFailure> {
    if devices.is_empty() {
        return base
            .solve()
            .map(|s| (s.values, Vec::new()))
            .map_err(|_| SolveFailure {
                code: DiagnosticCode::SingularSystem,
                components: Vec::new(),
            });
    }
    let initial_valid = initial.len() == base.size()
        && tolerance.len() == base.size()
        && initial_junctions.len() == devices.len()
        && initial.iter().all(|x| x.is_finite())
        && tolerance.iter().all(|t| {
            t.update.is_finite() && t.update > 0.0 && t.residual.is_finite() && t.residual > 0.0
        })
        && devices.iter().zip(&initial_junctions).all(|(d, v)| {
            let js = d.junctions();
            js.len() == v.len()
                && v.iter().all(|x| x.is_finite())
                && js.iter().all(|j| {
                    j.thermal_voltage.is_finite()
                        && j.thermal_voltage >= THERMAL_VOLTAGE
                        && j.thermal_voltage <= 2.0 * THERMAL_VOLTAGE
                        && j.saturation_current.is_finite()
                        && j.saturation_current > 0.0
                })
        });
    if !initial_valid {
        return Err(failure(DiagnosticCode::NonFiniteState, devices));
    }
    if let Some(solution) = stage(
        base,
        variables,
        devices,
        tolerance,
        initial.clone(),
        initial_junctions.clone(),
        0.0,
    )? {
        return check_range(solution, devices);
    }
    let (mut x, mut j) = (initial, initial_junctions);
    for g in [
        1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12, 0.0,
    ] {
        let Some((next, junctions)) = stage(base, variables, devices, tolerance, x, j, g)? else {
            return Err(failure(DiagnosticCode::NonConvergence, devices));
        };
        x = next;
        j = junctions;
    }
    check_range((x, j), devices)
}

fn lu_failure(error: SolverError) -> SolveFailure {
    SolveFailure {
        code: if matches!(error, SolverError::NonFinite) {
            DiagnosticCode::NonFiniteState
        } else {
            DiagnosticCode::SingularSystem
        },
        components: Vec::new(),
    }
}

fn voltage(x: &[f64], variables: &NodeVariables, node: usize) -> f64 {
    variables.voltage_variable(node).map_or(0.0, |i| x[i])
}

fn junction_values(d: &dyn NonlinearDevice, x: &[f64], variables: &NodeVariables) -> Vec<f64> {
    d.junctions()
        .iter()
        .map(|j| voltage(x, variables, j.positive) - voltage(x, variables, j.negative))
        .collect()
}

fn check_range(
    solution: (Vec<f64>, Vec<Vec<f64>>),
    devices: &[&dyn NonlinearDevice],
) -> Result<(Vec<f64>, Vec<Vec<f64>>), SolveFailure> {
    for (d, voltages) in devices.iter().zip(&solution.1) {
        for (j, v) in d.junctions().iter().zip(voltages) {
            let (value, derivative) = junction_exponential(v / j.thermal_voltage);
            if !value.is_finite() || !derivative.is_finite() {
                return Err(failure(DiagnosticCode::NonFiniteState, devices));
            }
        }
    }
    let bad = devices
        .iter()
        .zip(&solution.1)
        .filter(|(d, v)| {
            d.junctions()
                .iter()
                .zip(*v)
                .any(|(j, v)| v / j.thermal_voltage > 80.0)
        })
        .map(|(d, _)| *d)
        .collect::<Vec<_>>();
    if bad.is_empty() {
        Ok(solution)
    } else {
        Err(failure(DiagnosticCode::ModelRangeExceeded, &bad))
    }
}

type StageSolution = Option<(Vec<f64>, Vec<Vec<f64>>)>;

#[allow(clippy::too_many_arguments)]
fn stage(
    base: &MnaSystem,
    variables: &NodeVariables,
    devices: &[&dyn NonlinearDevice],
    tolerance: &[UnknownTolerance],
    mut old: Vec<f64>,
    mut limited: Vec<Vec<f64>>,
    conductance: f64,
) -> Result<StageSolution, SolveFailure> {
    #[cfg(test)]
    {
        tests::STAGES.with(|s| s.borrow_mut().push(conductance));
        if tests::FORCE_CONTINUATION.replace(false) {
            return Ok(None);
        }
    }
    let mut continuation_nodes = std::collections::BTreeSet::new();
    for d in devices {
        for j in d.junctions() {
            continuation_nodes.insert(j.positive);
            continuation_nodes.insert(j.negative);
        }
    }
    for _ in 0..NEWTON_MAX_ITERATIONS {
        #[cfg(test)]
        tests::ITERATIONS.set(tests::ITERATIONS.get() + 1);
        let mut system = base.clone();
        for node in &continuation_nodes {
            if let Some(i) = variables.voltage_variable(*node) {
                system.add_matrix(i, i, conductance);
            }
        }
        for (d, js) in devices.iter().zip(&limited) {
            let evaluation = d.evaluate(js);
            let mut intercepts = evaluation
                .currents
                .iter()
                .map(|c| (c.node, c.terms.iter().sum::<f64>()))
                .collect::<Vec<_>>();
            if !evaluation
                .currents
                .iter()
                .flat_map(|c| &c.terms)
                .all(|v| v.is_finite())
                || !evaluation.derivatives.iter().all(|(_, _, g)| g.is_finite())
            {
                return Err(failure(DiagnosticCode::NonFiniteState, devices));
            }
            // Recover terminal potentials consistent with the limited junctions.
            // Devices provide derivatives, so the intercept can instead be formed
            // directly by their junction chain rule: G V* = sum (dI/dv_j) v_j.
            // Recover that product using junction incidence (at most two junctions).
            let junctions = d.junctions();
            let mut potentials = std::collections::BTreeMap::new();
            for _ in 0..=junctions.len() {
                for (j, v) in junctions.iter().zip(js) {
                    if !potentials.contains_key(&j.positive)
                        && !potentials.contains_key(&j.negative)
                    {
                        potentials.insert(j.negative, 0.0);
                    }
                    if let Some(p) = potentials.get(&j.negative).copied() {
                        potentials.insert(j.positive, p + v);
                    } else if let Some(p) = potentials.get(&j.positive).copied() {
                        potentials.insert(j.negative, p - v);
                    }
                }
            }
            for (row, col, g) in evaluation.derivatives {
                for (node, intercept) in &mut intercepts {
                    if *node == row {
                        *intercept -= g * potentials.get(&col).copied().unwrap_or(0.0);
                    }
                }
                if let (Some(r), Some(c)) = (
                    variables.voltage_variable(row),
                    variables.voltage_variable(col),
                ) {
                    system.add_matrix(r, c, g);
                }
            }
            for (node, current) in intercepts {
                if let Some(i) = variables.voltage_variable(node) {
                    system.add_rhs(i, -current);
                }
            }
        }
        let next = system.solve_equilibrated().map_err(lu_failure)?.values;
        if !next.iter().all(|v| v.is_finite()) {
            return Err(failure(DiagnosticCode::NonFiniteState, devices));
        }
        let unlimited = devices
            .iter()
            .map(|d| junction_values(*d, &next, variables))
            .collect::<Vec<_>>();
        let mut was_limited = false;
        let next_limited = devices
            .iter()
            .zip(&unlimited)
            .zip(&limited)
            .map(|((d, new), old)| {
                d.junctions()
                    .iter()
                    .zip(new)
                    .zip(old)
                    .map(|((j, new), old)| {
                        let v = limit_junction(*new, *old, *j);
                        was_limited |= v != *new;
                        v
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let update =
            next.iter().zip(&old).zip(tolerance).all(|((a, b), t)| {
                (a - b).abs() <= NEWTON_RELTOL * a.abs().max(b.abs()) + t.update
            });
        if !was_limited
            && update
            && residual_ok(
                base,
                variables,
                devices,
                tolerance,
                &next,
                &unlimited,
                &continuation_nodes,
                conductance,
            )?
        {
            return Ok(Some((next, unlimited)));
        }
        old = next;
        limited = next_limited;
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn residual_ok(
    base: &MnaSystem,
    variables: &NodeVariables,
    devices: &[&dyn NonlinearDevice],
    tolerance: &[UnknownTolerance],
    x: &[f64],
    junctions: &[Vec<f64>],
    nodes: &std::collections::BTreeSet<usize>,
    g: f64,
) -> Result<bool, SolveFailure> {
    let mut residual = vec![0.0; x.len()];
    let mut scale = vec![0.0; x.len()];
    let mut roundoff = vec![0.0; x.len()];
    let mut additions = vec![0_usize; x.len()];
    for (i, terms) in base.terms.iter().enumerate() {
        let mut currents = Vec::with_capacity(terms.len());
        for (coefficients, rhs) in terms {
            let magnitude = coefficients
                .iter()
                .map(|(j, a)| (a * x[*j]).abs())
                .sum::<f64>()
                + rhs.abs();
            // n products, n-1 additions, RHS subtraction, and allowance for
            // rounded stored operands: conservatively use 2n+2 operations.
            roundoff[i] += gamma(2 * coefficients.len() + 2) * magnitude;
            additions[i] += 1;
            let value = if let [(a, g), (b, h)] = coefficients.as_slice()
                && *g == -*h
                && *rhs == 0.0
            {
                g * (x[*a] - x[*b])
            } else {
                coefficients.iter().map(|(j, a)| a * x[*j]).sum::<f64>() - rhs
            };
            residual[i] += value;
            currents.push(value);
            if i >= variables.voltage_variable_count() {
                scale[i] += coefficients
                    .iter()
                    .map(|(j, a)| (a * x[*j]).abs())
                    .sum::<f64>()
                    + rhs.abs();
            }
        }
        if i < variables.voltage_variable_count() {
            let mut start = 0;
            for group in &base.current_groups[i] {
                scale[i] += currents[start..group.start]
                    .iter()
                    .map(|v| v.abs())
                    .sum::<f64>();
                scale[i] += currents[group.clone()].iter().sum::<f64>().abs();
                start = group.end;
            }
            scale[i] += currents[start..].iter().map(|v| v.abs()).sum::<f64>();
        }
    }
    for (row, a, b, e) in &base.voltage_constraints {
        let (a, b) = (a.map_or(0.0, |i| x[i]), b.map_or(0.0, |i| x[i]));
        residual[*row] = a - b - e;
        scale[*row] = e.abs().max(a.abs()).max(b.abs());
        roundoff[*row] = gamma(4) * (a.abs() + b.abs() + e.abs());
        additions[*row] = 1;
    }
    for (d, js) in devices.iter().zip(junctions) {
        for c in d.evaluate(js).currents {
            if let Some(i) = variables.voltage_variable(c.node) {
                for term in c.terms {
                    residual[i] += term;
                    scale[i] += term.abs();
                    additions[i] += 1;
                }
            }
        }
    }
    for node in nodes {
        if let Some(i) = variables.voltage_variable(*node) {
            let current = g * x[i];
            residual[i] += current;
            scale[i] += current.abs();
            roundoff[i] += gamma(2) * current.abs();
            additions[i] += 1;
        }
    }
    for i in 0..x.len() {
        roundoff[i] += gamma(additions[i]) * scale[i];
    }
    if !residual
        .iter()
        .chain(&scale)
        .chain(&roundoff)
        .all(|v| v.is_finite())
    {
        return Err(failure(DiagnosticCode::NonFiniteState, devices));
    }
    Ok(residual
        .iter()
        .zip(&scale)
        .zip(tolerance)
        .zip(roundoff)
        .all(|(((r, s), t), rounding)| r.abs() <= NEWTON_RELTOL * s + t.residual + rounding))
}

/// Conservative floating-point evaluation allowance approved in #118.
/// EPSILON (rather than half EPSILON) also covers rounded stored operands.
fn gamma(operations: usize) -> f64 {
    let m = operations as f64 * f64::EPSILON;
    if m < 1.0 {
        m / (1.0 - m)
    } else {
        f64::INFINITY
    }
}
