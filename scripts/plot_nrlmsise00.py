#!/usr/bin/env python3
"""Plot NRLMSISE-00 output produced by the apogee-core example.

Usage:
    scripts/plot_nrlmsise00.py [alt_min_km] [alt_max_km] [step_km]

The script runs:
    cargo run --example plot_nrlmsise00 -p apogee-core -- <args>
and produces a two-panel matplotlib figure:
  - top: total mass density vs altitude
  - bottom: exospheric and local temperature vs altitude
"""

import os
import subprocess
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np


def run_example(alt_min_km: float, alt_max_km: float, step_km: float) -> str:
    root = Path(__file__).resolve().parents[1]
    cmd = [
        "cargo",
        "run",
        "--example",
        "plot_nrlmsise00",
        "-p",
        "apogee-core",
        "--",
        str(alt_min_km),
        str(alt_max_km),
        str(step_km),
    ]
    result = subprocess.run(
        cmd,
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )
    return result.stdout


def parse_csv(text: str) -> dict:
    lines = text.strip().splitlines()
    header = lines[0].split(",")
    data = np.genfromtxt(lines[1:], delimiter=",", names=header)
    return {name: data[name] for name in header}


def plot(data: dict, output_path: Path) -> None:
    fig, axes = plt.subplots(2, 1, figsize=(8, 8), sharex=True)

    alt = data["altitude_km"]

    ax = axes[0]
    ax.semilogy(alt, data["density_kg_m3"], label="total density")
    ax.set_ylabel("density (kg/m³)")
    ax.set_title("NRLMSISE-00: density vs altitude")
    ax.grid(True, which="both", ls="--", alpha=0.5)
    ax.legend()

    ax = axes[1]
    ax.plot(alt, data["temperature_exo_k"], label="exospheric T")
    ax.plot(alt, data["temperature_alt_k"], label="local T")
    ax.set_xlabel("altitude (km)")
    ax.set_ylabel("temperature (K)")
    ax.set_title("NRLMSISE-00: temperature vs altitude")
    ax.grid(True, ls="--", alpha=0.5)
    ax.legend()

    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    print(f"saved plot to {output_path}")


def main() -> None:
    alt_min_km = float(sys.argv[1]) if len(sys.argv) > 1 else 100.0
    alt_max_km = float(sys.argv[2]) if len(sys.argv) > 2 else 600.0
    step_km = float(sys.argv[3]) if len(sys.argv) > 3 else 10.0

    csv_text = run_example(alt_min_km, alt_max_km, step_km)
    data = parse_csv(csv_text)

    out_dir = Path(__file__).resolve().parents[1] / "plots"
    out_dir.mkdir(exist_ok=True)
    output_path = out_dir / "nrlmsise00_density_temperature.png"
    plot(data, output_path)


if __name__ == "__main__":
    main()
