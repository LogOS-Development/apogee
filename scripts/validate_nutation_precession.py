#!/usr/bin/env python3
"""Validate Apogee nutation/precession model against ERFA/SOFA.

Usage:
    scripts/validate_nutation_precession.py [out_dir]

Runs:
    cargo run --example nutation_precession_samples -p apogee-core

to produce a CSV of Apogee model outputs at yearly samples 2000-2030,
then computes the corresponding ERFA reference values and renders
comparison plots using scripts/validation_framework.py.

Output:
    plots/nutation_precession/
        dpsi_comparison.png
        deps_comparison.png
        obliquity_comparison.png
        bpn_angular_diff.png
        summary.json
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# Ensure the shared framework is importable.
SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from validation_framework import ComparisonPlot, Source, SummaryWriter

try:
    import erfa
except ImportError as e:  # pragma: no cover
    raise ImportError("pyerfa is required; run `uv pip install pyerfa`") from e


ROOT = SCRIPT_DIR.parent
EXAMPLE = "nutation_precession_samples"
DEFAULT_OUT_DIR = ROOT / "plots" / "nutation_precession"


def run_apogee_example() -> str:
    cmd = [
        "cargo",
        "run",
        "--example",
        EXAMPLE,
        "-p",
        "apogee-core",
        "--release",
        "--",
    ]
    result = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, check=True)
    return result.stdout


def load_apogee_csv(text: str) -> pd.DataFrame:
    lines = text.strip().splitlines()
    from io import StringIO

    return pd.read_csv(StringIO(text))


def reference_values(df: pd.DataFrame) -> dict[str, np.ndarray]:
    """Compute ERFA reference values for the same epochs as the Apogee CSV."""
    jd1 = 2451545.0
    # df.year is calendar year; convert to JD offset from J2000.0
    # J2000.0 = 2000-01-01 12:00:00 TDB = JD 2451545.0
    # Each year is approximately 365.25 days, but we use the Apogee-provided
    # tdb_seconds column for exact alignment if present.
    if "tdb_seconds" in df.columns:
        jd2 = (df["tdb_seconds"] - 0.0).to_numpy() / 86400.0
    else:
        years = df["year"].to_numpy()
        jd2 = (years - 2000.0) * 365.25

    dpsi = np.empty(len(jd2))
    deps = np.empty(len(jd2))
    obl = np.empty(len(jd2))
    bpn_diff = np.empty(len(jd2))

    for i, offset in enumerate(jd2):
        psi_i, eps_i = erfa.nut00b(jd1, offset)
        obl_i = erfa.obl06(jd1, offset)

        dpsi[i] = psi_i
        deps[i] = eps_i
        obl[i] = obl_i

        # BPN angular difference requires reconstructing matrices.
        # Note: erfa.bp06 returns (rb, rp, rbp) where rbp = P @ B; it does NOT
        # return N. Use erfa.num00b for the nutation matrix.
        B, P, _rbp = erfa.bp06(jd1, offset)
        N = erfa.num00b(jd1, offset)
        bpn_ref = N @ P @ B
        # Apogee matrix columns in CSV
        cols = [f"bpn_{r}{c}" for r in "123" for c in "123"]
        bpn_apogee = df[cols].iloc[i].to_numpy().reshape((3, 3))
        residual = bpn_apogee.T @ bpn_ref
        trace = np.clip((np.trace(residual) - 1.0) / 2.0, -1.0, 1.0)
        bpn_diff[i] = np.arccos(trace)

    ARCSEC = 4.84813681109536e-6
    return {
        "dpsi_arcsec": dpsi / ARCSEC,
        "deps_arcsec": deps / ARCSEC,
        "obliquity_deg": np.degrees(obl),
        "bpn_angular_diff_arcsec": bpn_diff / ARCSEC,
    }


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_OUT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)

    csv_text = run_apogee_example()
    df = load_apogee_csv(csv_text)
    years = df["year"].to_numpy()

    ref = reference_values(df)

    apogee_dpsi = Source.from_arrays("Apogee", years, df["dpsi_arcsec"].to_numpy(), unit="arcsec")
    ref_dpsi = Source.from_arrays("ERFA", years, ref["dpsi_arcsec"], unit="arcsec")
    apogee_deps = Source.from_arrays("Apogee", years, df["deps_arcsec"].to_numpy(), unit="arcsec")
    ref_deps = Source.from_arrays("ERFA", years, ref["deps_arcsec"], unit="arcsec")
    apogee_obl = Source.from_arrays("Apogee", years, df["obliquity_deg"].to_numpy(), unit="deg")
    ref_obl = Source.from_arrays("ERFA", years, ref["obliquity_deg"], unit="deg")

    # Nutation angles
    plot = ComparisonPlot(title="Nutation angles: Apogee vs ERFA (IAU 2000B)")
    plot.add_series(
        ref_dpsi,
        apogee_dpsi,
        name="Nutation in longitude Δψ",
        ylabel="Δψ",
        residual_unit="arcsec",
        tolerance=1.0,
    )
    plot.add_series(
        ref_deps,
        apogee_deps,
        name="Nutation in obliquity Δε",
        ylabel="Δε",
        residual_unit="arcsec",
        tolerance=1.0,
    )
    plot.save(out_dir / "nutation_angles.png")

    # Mean obliquity
    plot2 = ComparisonPlot(title="Mean obliquity: Apogee vs ERFA (IAU 2006)")
    plot2.add_series(
        ref_obl,
        apogee_obl,
        name="Mean obliquity ε_A",
        ylabel="ε_A",
        residual_unit="deg",
        tolerance=1.0 / 3600.0,
    )
    plot2.save(out_dir / "obliquity.png")

    # BPN angular difference
    bpn_source = Source.from_arrays(
        "Apogee", years, ref["bpn_angular_diff_arcsec"], unit="arcsec"
    )
    fig, ax = plt.subplots(figsize=(10, 4))
    ax.plot(bpn_source.x, bpn_source.y, label="BPN angular difference")
    ax.axhline(1.0, color="red", linestyle=":", label="1 arcsec tolerance")
    ax.set_xlabel("year")
    ax.set_ylabel("angular difference (arcsec)")
    ax.set_title("GCRF-to-TOD matrix angular difference vs ERFA")
    ax.legend()
    ax.grid(True, linestyle="--", alpha=0.5)
    fig.tight_layout()
    fig.savefig(out_dir / "bpn_angular_diff.png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    max_residual = max(
        np.max(np.abs(apogee_dpsi.y - ref_dpsi.y)),
        np.max(np.abs(apogee_deps.y - ref_deps.y)),
        np.max(np.abs(apogee_obl.y - ref_obl.y)) * 3600.0,  # deg -> arcsec
    )
    max_bpn = float(np.max(ref["bpn_angular_diff_arcsec"]))

    SummaryWriter(out_dir / "summary.json").write(
        name="nutation_precession",
        passed=max_residual <= 1.0 and max_bpn <= 1.0,
        max_residual=float(max_residual),
        notes=[
            f"samples: {len(years)} ({years[0]:.0f}–{years[-1]:.0f})",
            f"max nutation/obliquity residual: {max_residual:.3f} arcsec",
            f"max BPN angular diff: {max_bpn:.3f} arcsec",
        ],
    )

    print(f"Plots saved to {out_dir}")
    print(f"  max residual: {max_residual:.3f} arcsec")
    print(f"  max BPN diff: {max_bpn:.3f} arcsec")


if __name__ == "__main__":
    main()
