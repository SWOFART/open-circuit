//! Dense linear solvers used by the circuit engine.

/// Result of solving `A · x = b`.
#[derive(Clone, Debug, PartialEq)]
pub struct SolverSolution {
    pub values: Vec<f64>,
    /// Infinity norm of `A · x - b` before normalization.
    pub residual_inf: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverError {
    DimensionMismatch,
    NonFinite,
    Singular,
    ResidualTooLarge,
}

/// Interface kept small so dense LU can later be replaced without changing MNA assembly.
pub trait Solver {
    fn solve(&self, matrix: &[Vec<f64>], rhs: &[f64]) -> Result<SolverSolution, SolverError>;
}

/// Partial-pivoted dense LU for small Stage 1 systems.
///
/// The default relative pivot threshold is scaled by matrix dimension and row magnitude.
/// It detects numerically rank-deficient systems without comparing circuit coordinates or
/// physical parameters to an arbitrary absolute epsilon.
#[derive(Clone, Copy, Debug)]
pub struct DenseLuSolver {
    pub relative_pivot_tolerance: f64,
    pub relative_residual_tolerance: f64,
}

impl Default for DenseLuSolver {
    fn default() -> Self {
        Self {
            relative_pivot_tolerance: f64::EPSILON * 16.0,
            relative_residual_tolerance: 1e-10,
        }
    }
}

impl Solver for DenseLuSolver {
    fn solve(&self, matrix: &[Vec<f64>], rhs: &[f64]) -> Result<SolverSolution, SolverError> {
        let size = rhs.len();
        if matrix.len() != size || matrix.iter().any(|row| row.len() != size) {
            return Err(SolverError::DimensionMismatch);
        }
        if !self.relative_pivot_tolerance.is_finite()
            || self.relative_pivot_tolerance < 0.0
            || !self.relative_residual_tolerance.is_finite()
            || self.relative_residual_tolerance < 0.0
            || rhs.iter().any(|value| !value.is_finite())
            || matrix.iter().flatten().any(|value| !value.is_finite())
        {
            return Err(SolverError::NonFinite);
        }
        if size == 0 {
            return Ok(SolverSolution {
                values: Vec::new(),
                residual_inf: 0.0,
            });
        }

        let original_matrix = matrix;
        let original_rhs = rhs;
        let mut lu = matrix.to_vec();
        let mut result = rhs.to_vec();
        let mut row_scales = lu
            .iter()
            .map(|row| row.iter().map(|value| value.abs()).fold(0.0, f64::max))
            .collect::<Vec<_>>();
        if row_scales.contains(&0.0) {
            return Err(SolverError::Singular);
        }

        let pivot_limit = self.relative_pivot_tolerance * size as f64;
        for column in 0..size {
            let pivot_row = (column..size)
                .max_by(|left, right| {
                    let left_score = lu[*left][column].abs() / row_scales[*left];
                    let right_score = lu[*right][column].abs() / row_scales[*right];
                    left_score.total_cmp(&right_score)
                })
                .expect("a non-empty pivot range exists");
            let pivot_score = lu[pivot_row][column].abs() / row_scales[pivot_row];
            if !pivot_score.is_finite() {
                return Err(SolverError::NonFinite);
            }
            if pivot_score <= pivot_limit {
                return Err(SolverError::Singular);
            }

            if pivot_row != column {
                lu.swap(pivot_row, column);
                result.swap(pivot_row, column);
                row_scales.swap(pivot_row, column);
            }

            let pivot = lu[column][column];
            let pivot_values = lu[column].clone();
            for row in (column + 1)..size {
                let factor = lu[row][column] / pivot;
                if !factor.is_finite() {
                    return Err(SolverError::NonFinite);
                }
                lu[row][column] = 0.0;
                for (entry, pivot_entry) in lu[row].iter_mut().zip(&pivot_values).skip(column + 1) {
                    *entry -= factor * pivot_entry;
                    if !entry.is_finite() {
                        return Err(SolverError::NonFinite);
                    }
                }
                result[row] -= factor * result[column];
                if !result[row].is_finite() {
                    return Err(SolverError::NonFinite);
                }
            }
        }

        for row in (0..size).rev() {
            let mut value = result[row];
            for column in (row + 1)..size {
                value -= lu[row][column] * result[column];
            }
            result[row] = value / lu[row][row];
            if !result[row].is_finite() {
                return Err(SolverError::NonFinite);
            }
        }

        let mut residual_inf: f64 = 0.0;
        let mut matrix_solution_scale: f64 = 0.0;
        let mut rhs_scale: f64 = 0.0;
        for row in 0..size {
            let mut product = 0.0;
            let mut row_sum = 0.0;
            for (coefficient, value) in original_matrix[row].iter().zip(&result) {
                let term = coefficient * value;
                product += term;
                row_sum += term.abs();
            }
            if !product.is_finite() || !row_sum.is_finite() {
                return Err(SolverError::NonFinite);
            }
            residual_inf = residual_inf.max((product - original_rhs[row]).abs());
            matrix_solution_scale = matrix_solution_scale.max(row_sum);
            rhs_scale = rhs_scale.max(original_rhs[row].abs());
        }

        let residual_scale = matrix_solution_scale.max(rhs_scale).max(f64::MIN_POSITIVE);
        if residual_inf / residual_scale > self.relative_residual_tolerance {
            return Err(SolverError::ResidualTooLarge);
        }

        Ok(SolverSolution {
            values: result,
            residual_inf,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_lu_solves_a_system_with_a_row_pivot() {
        let solved = DenseLuSolver::default()
            .solve(&[vec![0.0, 2.0], vec![1.0, 1.0]], &[4.0, 3.0])
            .unwrap();
        assert!((solved.values[0] - 1.0).abs() < 1e-14);
        assert!((solved.values[1] - 2.0).abs() < 1e-14);
        assert!(solved.residual_inf < 1e-14);
    }

    #[test]
    fn dense_lu_rejects_singular_and_inconsistent_systems() {
        let solver = DenseLuSolver::default();
        assert_eq!(
            solver.solve(&[vec![1.0, 2.0], vec![2.0, 4.0]], &[1.0, 2.0]),
            Err(SolverError::Singular)
        );
        assert_eq!(
            solver.solve(&[vec![1.0, 1.0], vec![1.0, 1.0]], &[1.0, 2.0]),
            Err(SolverError::Singular)
        );
    }

    #[test]
    fn dense_lu_rejects_malformed_and_non_finite_systems() {
        let solver = DenseLuSolver::default();
        assert_eq!(
            solver.solve(&[vec![1.0, 0.0]], &[1.0, 2.0]),
            Err(SolverError::DimensionMismatch)
        );
        assert_eq!(
            solver.solve(&[vec![f64::NAN]], &[1.0]),
            Err(SolverError::NonFinite)
        );
    }
}
