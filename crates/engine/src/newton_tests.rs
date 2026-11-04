use super::*;
use std::cell::{Cell, RefCell};
thread_local! {
    pub(super) static FORCE_CONTINUATION: Cell<bool> = const { Cell::new(false) };
    pub(super) static STAGES: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    pub(crate) static ITERATIONS: Cell<usize> = const { Cell::new(0) };
}

struct Law {
    id: StableId,
    kind: u8,
    calls: Cell<usize>,
}
impl Law {
    fn diode() -> Self {
        Self {
            id: StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
            kind: 0,
            calls: Cell::new(0),
        }
    }
}
impl NonlinearDevice for Law {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        vec![Junction {
            positive: 0,
            negative: 1,
            thermal_voltage: THERMAL_VOLTAGE,
            saturation_current: 1e-14,
        }]
    }
    fn evaluate(&self, v: &[f64]) -> Evaluation {
        self.calls.set(self.calls.get() + 1);
        let (e, de) = junction_exponential(v[0] / THERMAL_VOLTAGE);
        let (i, g) = match self.kind {
            1 => (1.0, 1.0),
            2 => (1.0, 0.0),
            3 => (f64::NAN, 1.0),
            _ => (1e-14 * e, 1e-14 / THERMAL_VOLTAGE * de + 1e-12),
        };
        let leakage = if self.kind == 0 { 1e-12 * v[0] } else { 0.0 };
        Evaluation {
            currents: vec![
                TerminalCurrent {
                    node: 0,
                    terms: vec![i, leakage],
                },
                TerminalCurrent {
                    node: 1,
                    terms: vec![-i, -leakage],
                },
            ],
            derivatives: vec![(0, 0, g), (0, 1, -g), (1, 0, -g), (1, 1, g)],
        }
    }
}
fn variables() -> NodeVariables {
    NodeVariables::for_island(2, &[0, 1], 1, &Default::default())
}
fn tolerance(n: usize) -> Vec<UnknownTolerance> {
    let mut t = vec![
        UnknownTolerance {
            update: NEWTON_BRANCH_ABSTOL,
            residual: NEWTON_VNTOL
        };
        n
    ];
    t[0] = UnknownTolerance {
        update: NEWTON_VNTOL,
        residual: NEWTON_ITOL,
    };
    t
}
fn run(base: &MnaSystem, d: &Law) -> Result<(Vec<f64>, Vec<Vec<f64>>), SolveFailure> {
    solve(
        base,
        &variables(),
        &[d],
        &tolerance(base.size()),
        vec![0.0; base.size()],
        vec![vec![0.0]],
    )
}

#[test]
fn diode_forward_reverse_and_ideal_source_range_use_physical_residuals() {
    for (source, expected) in [(5.0, 0.692887832378056), (-5.0, -4.99999999499)] {
        let mut base = MnaSystem::new(1);
        let vars = variables();
        assert!(base.stamp_conductance(&vars, 0, 1, 0.001));
        assert!(base.stamp_current_injection(&vars, 0, 1, source / 1000.0));
        let d = Law::diode();
        ITERATIONS.set(0);
        let (x, _) = run(&base, &d).unwrap();
        eprintln!("source={source} V: {} Newton iterations", ITERATIONS.get());
        assert!((x[0] - expected).abs() < 1e-9, "{x:?}");
        let current = d.evaluate(&[x[0]]).currents[0].terms.iter().sum::<f64>();
        assert!(((source - x[0]) / 1000.0 - current).abs() < 1e-12);
    }
    let mut base = MnaSystem::new(2);
    assert!(base.stamp_ideal_voltage_source(&variables(), 0, 1, 1, 5.0));
    assert_eq!(
        run(&base, &Law::diode()).unwrap_err().code,
        DiagnosticCode::ModelRangeExceeded
    );
}

#[test]
fn continuation_runs_every_stage_and_removes_all_auxiliary_conductance() {
    let mut base = MnaSystem::new(1);
    let vars = variables();
    base.stamp_conductance(&vars, 0, 1, 0.001);
    base.stamp_current_injection(&vars, 0, 1, 0.005);
    STAGES.with(|s| s.borrow_mut().clear());
    FORCE_CONTINUATION.set(true);
    ITERATIONS.set(0);
    let (x, _) = run(&base, &Law::diode()).unwrap();
    eprintln!(
        "forced continuation: {} Newton iterations",
        ITERATIONS.get()
    );
    assert!((x[0] - 0.692887832378056).abs() < 1e-9);
    STAGES.with(|s| {
        assert_eq!(
            *s.borrow(),
            vec![
                0.0, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12, 0.0
            ]
        )
    });
}

#[test]
fn exhaustion_is_bounded_and_linear_failure_does_not_try_continuation() {
    let mut d = Law::diode();
    d.kind = 1;
    let mut base = MnaSystem::new(1);
    base.stamp_conductance(&variables(), 0, 1, 0.001);
    assert_eq!(
        run(&base, &d).unwrap_err().code,
        DiagnosticCode::NonConvergence
    );
    assert!(d.calls.get() <= 1300 * 2);
    d.kind = 2;
    STAGES.with(|s| s.borrow_mut().clear());
    assert_eq!(
        run(&MnaSystem::new(1), &d).unwrap_err().code,
        DiagnosticCode::SingularSystem
    );
    STAGES.with(|s| assert_eq!(*s.borrow(), vec![0.0]));
    d.kind = 3;
    assert_eq!(
        run(&base, &d).unwrap_err().code,
        DiagnosticCode::NonFiniteState
    );
}

#[test]
fn tiny_updates_do_not_hide_a_large_original_equation_residual() {
    let base = MnaSystem::new(1);
    let vars = variables();
    let mut d = Law::diode();
    d.kind = 1;
    assert!(
        !residual_ok(
            &base,
            &vars,
            &[&d],
            &tolerance(1),
            &[0.0],
            &[vec![0.0]],
            &Default::default(),
            0.0
        )
        .unwrap()
    );
}

#[test]
fn limiting_and_exponential_extension_follow_the_contract() {
    let j = Law::diode().junctions()[0];
    let limited = limit_junction(5.0, 0.0, j);
    assert_eq!(limited, THERMAL_VOLTAGE * (5.0 / THERMAL_VOLTAGE).ln());
    assert_eq!(limit_junction(-5.0, 0.0, j), -5.0);
    assert_eq!(junction_exponential(81.0).1, 80.0_f64.exp());
    assert_eq!(junction_exponential(81.0).0, 2.0 * 80.0_f64.exp() - 1.0);
}

#[test]
fn motor_voltage_and_torque_rows_and_speed_update_use_distinct_units() {
    let vars = variables();
    let mut base = MnaSystem::new(3);
    base.stamp_ideal_voltage_source(&vars, 0, 1, 1, 1.0);
    base.stamp_linear_equation(2, &[(2, 2.0), (1, -0.1)], 3.0);
    let t = vec![
        UnknownTolerance {
            update: NEWTON_VNTOL,
            residual: NEWTON_ITOL,
        },
        UnknownTolerance {
            update: NEWTON_BRANCH_ABSTOL,
            residual: NEWTON_VNTOL,
        },
        UnknownTolerance {
            update: NEWTON_SPEED_ABSTOL,
            residual: NEWTON_TORQUE_ABSTOL,
        },
    ];
    let d = Law::diode();
    let (e, _) = junction_exponential(1.0 / THERMAL_VOLTAGE);
    let current = 1e-14 * e + 1e-12;
    let x = vec![1.0, -current, (3.0 - 0.1 * current) / 2.0];
    assert!(
        residual_ok(
            &base,
            &vars,
            &[&d],
            &t,
            &x,
            &[vec![1.0]],
            &Default::default(),
            0.0
        )
        .unwrap()
    );
    let mut bad = x.clone();
    bad[2] += 1e-6;
    assert!(
        !residual_ok(
            &base,
            &vars,
            &[&d],
            &t,
            &bad,
            &[vec![1.0]],
            &Default::default(),
            0.0
        )
        .unwrap()
    );
}

#[test]
fn cancellation_allowance_accepts_roundoff_but_rejects_a_physically_wrong_current() {
    struct Current {
        id: StableId,
    }
    impl NonlinearDevice for Current {
        fn id(&self) -> &StableId {
            &self.id
        }
        fn junctions(&self) -> Vec<Junction> {
            vec![Junction {
                positive: 1,
                negative: 2,
                thermal_voltage: THERMAL_VOLTAGE,
                saturation_current: 1e-14,
            }]
        }
        fn evaluate(&self, _: &[f64]) -> Evaluation {
            Evaluation {
                currents: vec![
                    TerminalCurrent {
                        node: 1,
                        terms: vec![1.9456910007138435e-5],
                    },
                    TerminalCurrent {
                        node: 2,
                        terms: vec![-1.9456910007138435e-5],
                    },
                ],
                derivatives: vec![],
            }
        }
    }
    let variables = NodeVariables::for_island(3, &[0, 1, 2], 2, &Default::default());
    let mut base = MnaSystem::new(2);
    base.stamp_conductance(&variables, 0, 1, 100.0);
    base.stamp_current_injection(&variables, 0, 2, 1.9456910007138435e-5);
    let d = Current {
        id: Law::diode().id,
    };
    let x = [5.0, 4.9999998054309];
    let tolerance = vec![
        UnknownTolerance {
            update: NEWTON_VNTOL,
            residual: NEWTON_ITOL
        };
        2
    ];
    assert!(
        residual_ok(
            &base,
            &variables,
            &[&d],
            &tolerance,
            &x,
            &[vec![x[1]]],
            &Default::default(),
            0.0
        )
        .unwrap()
    );
    let wrong = [5.0, x[1] + 1e-8];
    assert!(
        !residual_ok(
            &base,
            &variables,
            &[&d],
            &tolerance,
            &wrong,
            &[vec![wrong[1]]],
            &Default::default(),
            0.0
        )
        .unwrap()
    );
    let mut overflow = MnaSystem::new(2);
    overflow.stamp_linear_equation(0, &[(0, 1e308), (1, -1e308)], 0.0);
    assert_eq!(
        residual_ok(
            &overflow,
            &variables,
            &[&d],
            &tolerance,
            &[5.0, 5.0],
            &[vec![5.0]],
            &Default::default(),
            0.0
        )
        .unwrap_err()
        .code,
        DiagnosticCode::NonFiniteState
    );
}

#[test]
fn finite_source_and_companion_cancellation_cannot_inflate_physical_current_scale() {
    use crate::devices::{
        CapacitorDevice, CapacitorMode, DcVoltageSourceDevice, Device, InductorDevice,
        InductorMode, IntegrationMethod, Terminals,
    };
    let d = Law::diode();
    let terminals = Terminals { pin1: 0, pin2: 1 };
    let devices: Vec<Box<dyn Device>> = vec![
        Box::new(DcVoltageSourceDevice::new(
            d.id.clone(),
            terminals,
            -5.0,
            0.1,
            None,
        )),
        Box::new(CapacitorDevice::new(
            d.id.clone(),
            terminals,
            0.005,
            CapacitorMode::Transient {
                step_seconds: 0.001,
                start_voltage_volts: -5.0,
                start_current_amps: 0.0,
                integration_method: IntegrationMethod::Trapezoidal,
            },
        )),
        Box::new(InductorDevice::new(
            d.id.clone(),
            terminals,
            0.00005,
            InductorMode::Transient {
                step_seconds: 0.001,
                start_current_amps: 50.0,
                start_voltage_volts: 0.0,
                integration_method: IntegrationMethod::Trapezoidal,
            },
        )),
    ];
    for device in devices {
        let vars = variables();
        let mut base = MnaSystem::new(1);
        device.stamp(&mut base, &vars).unwrap();
        let wrong = [-4.999999999];
        let physical = device
            .result(&[wrong[0], 0.0], &wrong)
            .unwrap()
            .current_amps;
        assert!(physical.abs() < 2e-8);
        assert!(
            !residual_ok(
                &base,
                &vars,
                &[&d],
                &tolerance(1),
                &wrong,
                &[vec![wrong[0]]],
                &Default::default(),
                0.0
            )
            .unwrap(),
            "a ~1e-8 A KCL error must not inherit the ~100 A Norton-stamp scale"
        );
        let (solution, junctions) = run(&base, &d).unwrap();
        assert!(
            residual_ok(
                &base,
                &vars,
                &[&d],
                &tolerance(1),
                &solution,
                &junctions,
                &Default::default(),
                0.0
            )
            .unwrap()
        );
    }
}

#[test]
fn two_junction_three_terminal_law_uses_a_consistent_linearization_reference() {
    struct TwoJunction(StableId);
    impl NonlinearDevice for TwoJunction {
        fn id(&self) -> &StableId {
            &self.0
        }
        fn junctions(&self) -> Vec<Junction> {
            [1, 2]
                .map(|negative| Junction {
                    positive: 0,
                    negative,
                    thermal_voltage: THERMAL_VOLTAGE,
                    saturation_current: 1e-14,
                })
                .to_vec()
        }
        fn evaluate(&self, v: &[f64]) -> Evaluation {
            let (a, b) = (0.01 * v[0].powi(3), 0.02 * v[1].powi(3));
            let (ga, gb) = (0.03 * v[0].powi(2), 0.06 * v[1].powi(2));
            Evaluation {
                currents: vec![
                    TerminalCurrent {
                        node: 0,
                        terms: vec![a, b],
                    },
                    TerminalCurrent {
                        node: 1,
                        terms: vec![-a],
                    },
                    TerminalCurrent {
                        node: 2,
                        terms: vec![-b],
                    },
                ],
                derivatives: vec![
                    (0, 0, ga + gb),
                    (0, 1, -ga),
                    (0, 2, -gb),
                    (1, 0, -ga),
                    (1, 1, ga),
                    (2, 0, -gb),
                    (2, 2, gb),
                ],
            }
        }
    }
    let vars = NodeVariables::for_island(4, &[0, 1, 2, 3], 3, &Default::default());
    let mut base = MnaSystem::new(3);
    // Analytical answer V=[0.5,0.3,0.2], v_j=[0.2,0.3], currents [80,540] uA.
    for (node, current) in [0.00112, 0.00022, -0.00034].into_iter().enumerate() {
        base.stamp_conductance(&vars, node, 3, 0.001);
        base.stamp_current_injection(&vars, node, 3, current);
    }
    let t = vec![
        UnknownTolerance {
            update: NEWTON_VNTOL,
            residual: NEWTON_ITOL
        };
        3
    ];
    let d = TwoJunction(Law::diode().id);
    let (x, j) = solve(&base, &vars, &[&d], &t, vec![0.0; 3], vec![vec![0.0; 2]]).unwrap();
    for (actual, expected) in x.iter().zip([0.5, 0.3, 0.2]) {
        assert!((actual - expected).abs() < 1e-9, "{x:?}");
    }
    assert!(residual_ok(&base, &vars, &[&d], &t, &x, &j, &Default::default(), 0.0).unwrap());
}
