# Physics and component models

This directory records the electrical equations, assumptions, and model families used by
Open Circuit. It is a research and implementation reference, not a promise to simulate every
physical effect or every real-world part.

## Source of truth

- [`ARCHITECTURE.md`](../../ARCHITECTURE.md) contains project-level physics and solver
  decisions.
- The [Stage 1 contract](../contracts/stage-1.md) is normative for the current schematic,
  parameter domains, terminal signs, and Engine behavior.
- The [Stage 1 resistive solver notes](../solver/stage-1-resistive.md) record the implemented
  matrix stamps, numerical thresholds, and golden-fixture tolerances.
- [`models.md`](models.md) gives the common model-record format and the staged component
  inventory.

If this reference conflicts with the architecture or an approved contract, the architecture
or contract takes precedence. Resolve the conflict in a reviewed change; do not silently
change the Engine behavior to match explanatory text.

## Modeling approach

The Engine combines network conservation equations with each device's constitutive model.
The topology and model equations must use the same declared pin orientation and current sign.
Numerical methods such as modified nodal analysis (MNA) are ways to assemble and solve those
equations; they are not additional physical laws.

The current design uses lumped electrical components. KCL and KVL are applied within that
model, with relevant distributed, electromagnetic, thermal, and manufacturing effects
represented only when an explicit component model is added for them.

## Research principles

- State each model's assumptions and validity limits.
- Keep ideal models distinct from non-ideal extensions such as tolerance, temperature
  dependence, parasitics, leakage, saturation, or loss.
- Use SI values inside the Engine; UI prefixes are presentation and input concerns.
- Accept a model only with analytical or otherwise authoritative known-answer tests, as
  required by the architecture.
- Cite the source and version for model equations or parameter conventions. A simulator's
  device model is an implementation choice, not a universal description of a component.

## References

- Ho, Ruehli, and Brennan, [“The Modified Nodal Approach to Network Analysis”](https://doi.org/10.1109/TCS.1975.1084079), *IEEE Transactions on Circuits and Systems*, 1975.
- MIT OpenCourseWare, [Kirchhoff's Laws](https://ocw.mit.edu/courses/6-071j-introduction-to-electronics-signals-and-measurement-spring-2006/resources/03-kirchhoff1/), 6.071J, Spring 2006.
- Bureau International des Poids et Mesures, [The International System of Units (SI Brochure)](https://www.bipm.org/en/web/guest/publications/si-brochure), 9th edition, updated 2026.
- Modelica Association, [Modelica Standard Library 4.0.0: Electrical.Analog.Basic](https://doc.modelica.org/Modelica%204.0.0/Resources/helpDymola/Modelica_Electrical_Analog_Basic.html).
- ngspice project, [official documentation and manuals](https://ngspice.sourceforge.io/docs.html), including User's Manual 47 and the maintained online manual.
