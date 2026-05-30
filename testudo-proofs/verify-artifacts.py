#!/usr/bin/env python3
"""Cross-reference Lean proofs ↔ TOML artifacts for testudo-proofs.

Run from testudo-proofs/ directory:
    python3 verify-artifacts.py

Checks:
  1. Every .lean file has a matching .toml artifact.
  2. Every .toml has a matching .lean file.
  3. Artifact theorem.name appears in the .lean file.
  4. Constraint values are within reasonable bounds.
"""

# @anchor infra:proofs:verify-artifacts
# @tags infra
import re
import sys
import tomllib

PROOF_DIR = "Proofs"


def lean_theorem_names(path: str) -> set[str]:
    """Extract theorem names from a .lean file."""
    with open(path) as f:
        content = f.read()
    return set(re.findall(r"^theorem\s+(\w+)", content, re.MULTILINE))


def main() -> int:
    errors = 0
    warnings = 0

    lean_files = sorted(
        f for f in os.listdir(PROOF_DIR) if f.endswith(".lean")
    )
    toml_files = sorted(
        f for f in os.listdir(PROOF_DIR) if f.endswith(".toml")
    )

    # ── Check every .lean has a matching .toml ──
    for lean_file in lean_files:
        base = lean_file.replace(".lean", "")
        toml_path = os.path.join(PROOF_DIR, f"{base}.toml")

        if not os.path.exists(toml_path):
            print(f"ERROR: {lean_file} has no matching {base}.toml artifact")
            errors += 1
            continue

        # ── Parse artifact ──
        with open(toml_path, "rb") as f:
            artifact = tomllib.load(f)

        # Check required sections
        for section in ("meta", "theorem", "constraints", "prompt"):
            if section not in artifact:
                print(f"ERROR: {base}.toml missing [{section}] section")
                errors += 1

        # Check theorem name matches a theorem in the .lean file
        if "theorem" in artifact:
            theorem_name = artifact["theorem"].get("name", "")
            lean_names = lean_theorem_names(os.path.join(PROOF_DIR, lean_file))

            if theorem_name and theorem_name not in lean_names:
                # Theorem name might be a compound (e.g., "thm1 + thm2").
                # Split and check individually.
                parts = [p.strip() for p in theorem_name.replace("+", " ").split()]
                found_all = all(p in lean_names for p in parts if p)
                if not found_all:
                    print(
                        f"WARN: {base}.toml theorem.name '{theorem_name}' "
                        f"not found in {lean_file} "
                        f"(available: {sorted(lean_names)})"
                    )
                    warnings += 1

        # ── Spot-check constraint values ──
        constraints = artifact.get("constraints", {})
        if "max_leverage" in constraints and constraints["max_leverage"] > 10:
            print(
                f"WARN: {base}.toml max_leverage={constraints['max_leverage']} "
                f"exceeds 10"
            )
            warnings += 1
        if "max_drawdown_pct" in constraints and constraints["max_drawdown_pct"] > 50:
            print(
                f"WARN: {base}.toml max_drawdown_pct={constraints['max_drawdown_pct']} "
                f"exceeds 50"
            )
            warnings += 1
        if "max_account_risk_pct" in constraints and constraints["max_account_risk_pct"] > 10:
            print(
                f"WARN: {base}.toml max_account_risk_pct={constraints['max_account_risk_pct']} "
                f"exceeds 10"
            )
            warnings += 1

    # ── Check every .toml has a matching .lean ──
    for toml_file in toml_files:
        base = toml_file.replace(".toml", "")
        lean_path = os.path.join(PROOF_DIR, f"{base}.lean")

        if not os.path.exists(lean_path):
            print(f"ERROR: {toml_file} has no matching {base}.lean")
            errors += 1

    # ── Summary ──
    total_lean = len(lean_files)
    total_toml = len(toml_files)
    print(f"\nLean files: {total_lean}, TOML artifacts: {total_toml}")
    if warnings:
        print(f"{warnings} warning(s)")
    if errors:
        print(f"{errors} error(s)")
        return 1
    print("All artifacts valid ✓")
    return 0


if __name__ == "__main__":
    sys.exit(main())
